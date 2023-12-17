use std::borrow::BorrowMut;
use std::collections::HashSet;
use std::collections::hash_map::RandomState;
use std::io::ErrorKind;
use std::result::Result as DefaultResult;

use chrono::{DateTime, FixedOffset, Local as LocalTime, Duration};
use http_body::Body as HttpBody;
use tokio::task;
use tokio::net::TcpStream;
use tokio::sync::RwLock;
use hyper::{Uri, Request, header, Body as HyperBody, Response, StatusCode};
use hyper::client::conn as ClientConn;
use jsonwebtoken::jwk::{JwkSet, Jwk};

use crate::AppAuthCfg;
use crate::error::{AppError, AppErrorCode};
use crate::constant::HTTP_CONTENT_TYPE_JSON;

const MAX_NBYTES_LOADED_RESPONSE_KEYSTORE: usize = 102400;

pub struct AppAuthKeystore {
    pub update_period: Duration,
    inner: RwLock<InnerKeystoreContext>
}
struct InnerKeystoreContext {
    keyset: JwkSet,
    keystore_url: Uri,
    last_update: DateTime<FixedOffset>
}
pub struct AppKeystoreRefreshResult {
    // number of minutes to next refresh operation
    pub period_next_op: Duration,
    pub num_discarded: usize,
    pub num_added: usize,
}

impl AppAuthKeystore {
    pub fn new(cfg:&AppAuthCfg) -> Self {
        let update_period = Duration::minutes(cfg.update_interval_minutes as i64);
        // caller can start refresh operation immediately after initialization
        let last_update = LocalTime::now().fixed_offset() - update_period - Duration::seconds(5);
        let keystore_url = cfg.keystore_url.parse::<Uri>().unwrap();
        let inner = InnerKeystoreContext { keyset: JwkSet{keys:vec![]},
            keystore_url, last_update };
        Self { inner: RwLock::new(inner), update_period }
    }
    pub async fn refresh(&self) -> DefaultResult<AppKeystoreRefreshResult, AppError>
    {
        let mut guard = self.inner.write().await;
        let ctx = guard.borrow_mut();
        let expect_time = ctx.last_update + self.update_period;
        let t0 = LocalTime::now().fixed_offset();
        // this ensures there's only one task refreshing the key store
        // in multithreaded application
        if t0 > expect_time {
            let keys = self.request_new_keys(&ctx.keystore_url).await ?;
            let (num_discarded, num_added) = Self::merge(& mut ctx.keyset, keys);
            ctx.last_update = t0;
            Ok(AppKeystoreRefreshResult { num_discarded, num_added,
                period_next_op: self.update_period.clone() })
        } else {
            let period_next_op = expect_time - t0;
            assert!(period_next_op.num_seconds() >= 0);
            Ok(AppKeystoreRefreshResult { period_next_op, num_discarded:0, num_added:0 })
        }
    }
    
    pub fn merge(target:&mut JwkSet, new:JwkSet) -> (usize, usize)
    {
        let get_kid = |item:&Jwk| -> Option<String> {
            let result = item.common.key_id.as_ref();
            if let Some(id) = result {
                Some(id.to_string())
            } else { None }
        };  // in this application, key ID must be present
        let kids_iter_1 = target.keys.iter().filter_map(get_kid);
        let kids_iter_2 = new.keys.iter().filter_map(get_kid);
        let kidset1: HashSet<String, RandomState> = HashSet::from_iter(kids_iter_1);
        let kidset2 = HashSet::from_iter(kids_iter_2);
        let added     = kidset2.difference(&kidset1).collect::<Vec<_>>();
        let discarded = kidset1.difference(&kidset2).collect::<Vec<_>>();
        discarded.iter().map(|d_kid| {
            let result = target.keys.iter().position(|item| {
                let t_kid = item.common.key_id.as_ref().unwrap().as_str();
                d_kid.as_str() == t_kid
            });
            if let Some(idx) = result {
                let _item = target.keys.remove(idx);
            }
        }).count();
        let new_iter = new.keys.into_iter().filter(|item| {
            if let Some(id) = item.common.key_id.as_ref() {
                added.contains(&id)
            } else { false }
        });
        target.keys.extend(new_iter); 
        (discarded.len(), added.len())
    } // end of fn merge

    async fn request_new_keys(&self, url:&Uri) -> DefaultResult<JwkSet, AppError>
    { // TODO, config parameter for http version
        let (sender, connector) = self.setup_tcp_keyserver(url).await ?;
        // make the low-level connection process inbound / outbound messages
        // in a spawned task, optionally return error
        let _handle = task::spawn(async move {connector.await});
        let resp = self._request_to_key_server(url, sender).await ?;
        let keys = self.resp_body_to_keys(resp).await ?;
        Ok(keys)
    } // end of request_new_keys
    
    async fn setup_tcp_keyserver(&self, url:&Uri)
        -> DefaultResult<(ClientConn::SendRequest<HyperBody>,
                          ClientConn::Connection<TcpStream, HyperBody>), AppError>
    {
        let host = url.host().unwrap();
        let port = url.port().unwrap().as_u16();
        let addr = format!("{host}:{port}");
        match TcpStream::connect(addr).await {
            Ok(stream) => match ClientConn::handshake(stream).await {
                Ok(m) => Ok(m),
                Err(net_e) => Err( AppError { detail: Some(net_e.to_string()),
                    code: AppErrorCode::from(&net_e) })
            },
            Err(net_e) => Err( AppError { detail: Some(net_e.to_string()),
                    code: AppErrorCode::IOerror(net_e.kind()) })
        }
    }
    
    async fn _request_to_key_server(&self, url:&Uri, mut sender:ClientConn::SendRequest<HyperBody>)
        -> DefaultResult<Response<HyperBody>, AppError>
    {
        let result = Request::builder().uri(url.path()).method(hyper::Method::GET)
            .header(header::ACCEPT, HTTP_CONTENT_TYPE_JSON)
            .body(HyperBody::empty());
        match result {
            Ok(req) => match sender.send_request(req).await {
                Ok(resp) => if resp.status() == StatusCode::OK {
                    Ok(resp) // TODO, improve status check
                } else {
                    Err(AppError {
                        detail: Some(format!("remote-key-server-response-status:{}", resp.status())),
                        code: AppErrorCode::IOerror(ErrorKind::ConnectionRefused) })
                },
                Err(net_e) => Err( AppError { detail: Some(net_e.to_string()),
                                   code: AppErrorCode::from(&net_e) })
            },
            Err(net_e) => Err( AppError { detail: Some(net_e.to_string()),
                    code: AppErrorCode::InvalidInput })
        }
    }

    async fn resp_body_to_keys(&self, mut resp: Response<HyperBody>)
        -> DefaultResult<JwkSet, AppError>
    { // TODO, generalize using macro, generic type parameter cause
        let body = resp.body_mut();
        let mut raw_collected : Vec<u8> = Vec::new();
        while let Some(data) = body.data().await {
            let result = match data {
                Ok(raw) => {
                    raw_collected.extend(raw.to_vec());
                    let result = serde_json::from_slice::<JwkSet>(raw_collected.as_slice());
                    if let Ok(out) = result {
                        Some(Ok(out))
                    } else if raw_collected.len() > MAX_NBYTES_LOADED_RESPONSE_KEYSTORE {
                        Some(Err( AppError { detail: Some("auth-keys-resp-body".to_string()),
                            code: AppErrorCode::ExceedingMaxLimit }))
                    } else {None}
                },
                Err(net_e) => Some(Err( AppError { detail: Some(net_e.to_string()),
                        code: AppErrorCode::from(&net_e) } ))
            };
            if let Some(v) = result {
                return v;
            }
        }
        Err( AppError { detail: Some("resp-body-recv-complete".to_string()),
                        code: AppErrorCode::DataCorruption })
    } // end of resp_body_to_keys
} // end of fn AppAuthKeystore

impl From<&hyper::Error> for AppErrorCode {
    fn from(value: &hyper::Error) -> Self {
        if value.is_connect() {
            Self::IOerror(ErrorKind::NotConnected)
        } else if value.is_parse() || value.is_incomplete_message() {
            Self::DataCorruption
        } else if value.is_parse_too_large() {
            Self::ExceedingMaxLimit
        } else if value.is_user() {
            Self::IOerror(ErrorKind::InvalidInput)
        } else if value.is_timeout() {
            Self::IOerror(ErrorKind::TimedOut)
        } else if value.is_canceled() {
            Self::IOerror(ErrorKind::Interrupted)
        } else { Self::IOerror(ErrorKind::Other) }
    }
}

