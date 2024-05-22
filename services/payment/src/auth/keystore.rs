use std::borrow::BorrowMut;
use std::collections::hash_map::RandomState;
use std::collections::HashSet;
use std::io::Error as IoError;
use std::result::Result;

use actix_http::uri::{InvalidUri, Uri};
use actix_web::web::Bytes;
use async_trait::async_trait;
use chrono::{DateTime, Duration, FixedOffset, Local};
use http_body_util::{BodyExt, Empty};
use hyper::body::Frame;
use jsonwebtoken::jwk::{Jwk, JwkSet};
use tokio::sync::RwLock;

use ecommerce_common::config::AppAuthCfg;

use super::AppAuthError;

// Note, the macro `async_trait(?Send)` is applied ONLY if you are pretty sure
// the interface is accessed ONLY in single-thread runtime, so you can ignore
// `Send` check in almost all types declared in this application.

#[async_trait]
pub trait AbstractAuthKeystore: Sync + Send {
    type Error;

    fn update_period(&self) -> Duration;

    async fn refresh(&self) -> Result<AppKeystoreRefreshResult, Self::Error>;

    async fn find(&self, kid: &str) -> Result<Jwk, Self::Error>;
}

pub struct AppAuthKeystore {
    update_period: Duration,
    url: Uri,
    inner: RwLock<InnerKeystoreContext>,
}
struct InnerKeystoreContext {
    keyset: JwkSet,
    last_update: DateTime<FixedOffset>,
}
pub struct AppKeystoreRefreshResult {
    // number of minutes to next refresh operation
    pub period_next_op: Duration,
    pub num_discarded: usize,
    pub num_added: usize,
}

impl From<InvalidUri> for AppAuthError {
    fn from(value: InvalidUri) -> Self {
        Self::KeyStoreUri(value.to_string())
    }
}
impl From<IoError> for AppAuthError {
    fn from(value: IoError) -> Self {
        Self::NetworkIO(value)
    }
}
impl From<hyper::Error> for AppAuthError {
    fn from(value: hyper::Error) -> Self {
        let detail = value.to_string();
        if value.is_user() {
            Self::HttpInvalidSetup(detail)
        } else if value.is_parse() || value.is_parse_status() {
            Self::HttpParse(detail)
        } else if value.is_timeout() {
            Self::HttpTimeout(detail)
        } else if value.is_canceled() || value.is_body_write_aborted() {
            Self::HttpAbort(detail)
        } else if value.is_incomplete_message() {
            Self::HttpDataCorruption(detail)
        } else {
            Self::HttpOther(value)
        }
    }
}
impl From<hyper::http::Error> for AppAuthError {
    fn from(value: hyper::http::Error) -> Self {
        let detail = value.to_string();
        Self::HttpInvalidSetup(detail)
    }
}
impl From<Frame<Bytes>> for AppAuthError {
    fn from(value: Frame<Bytes>) -> Self {
        let detail = format!("{:?}", value);
        Self::HttpDataCorruption(detail)
    }
}
impl From<serde_json::Error> for AppAuthError {
    fn from(value: serde_json::Error) -> Self {
        let detail = value.to_string();
        Self::AppParse(detail)
    }
}

#[async_trait]
impl AbstractAuthKeystore for AppAuthKeystore {
    type Error = AppAuthError;

    fn update_period(&self) -> Duration {
        self.update_period
    }

    async fn refresh(&self) -> Result<AppKeystoreRefreshResult, Self::Error> {
        let mut guard = self.inner.write().await;
        let ctx = guard.borrow_mut();
        let next_time = ctx.last_update + self.update_period;
        let t0 = Local::now().fixed_offset();
        let (nd, na) = if t0 > next_time {
            let newkeys = self.request_new_keys().await?;
            ctx.last_update = t0;
            Self::merge(&mut ctx.keyset, newkeys)
        } else {
            (0, 0)
        };
        Ok(AppKeystoreRefreshResult {
            period_next_op: self.update_period,
            num_discarded: nd,
            num_added: na,
        })
    }

    async fn find(&self, _kid: &str) -> Result<Jwk, Self::Error> {
        Err(AppAuthError::NotSupport)
    }
} // end of impl AppAuthKeystore

impl AppAuthKeystore {
    pub(crate) fn try_create(cfg: &AppAuthCfg) -> Result<Self, AppAuthError> {
        let update_period = Duration::minutes(cfg.update_interval_minutes as i64);
        let last_update = Local::now().fixed_offset() - update_period - Duration::seconds(5);
        let url = cfg.keystore_url.parse::<Uri>()?;
        if url.host().is_none() || url.port_u16().is_none() {
            let msg = format!("host-or-port-missing, {}", cfg.keystore_url);
            return Err(AppAuthError::KeyStoreUri(msg));
        }
        let inner = {
            let jwks = InnerKeystoreContext {
                keyset: JwkSet { keys: Vec::new() },
                last_update,
            };
            RwLock::new(jwks)
        };
        Ok(Self {
            update_period,
            url,
            inner,
        })
    }

    async fn request_new_keys(&self) -> Result<JwkSet, AppAuthError> {
        let addr = (self.url.host().unwrap(), self.url.port_u16().unwrap());
        let stream = actix_web::rt::net::TcpStream::connect(addr).await?;
        let io_adapter = hyper_util::rt::TokioIo::new(stream);
        let (mut sender, connector) = hyper::client::conn::http1::handshake(io_adapter).await?;
        let _handle = actix_web::rt::spawn(connector);
        let body = Empty::<Bytes>::default();
        let req = hyper::Request::get(self.url.path())
            .header(hyper::header::ACCEPT, "application/json")
            .body(body)?;
        let mut resp = sender.send_request(req).await?;
        if resp.status() != hyper::StatusCode::OK {
            let code = resp.status().as_u16();
            return Err(AppAuthError::KeyStoreServer(code));
        }
        let mut raw_collected = Vec::<u8>::new();
        while let Some(nxt) = resp.frame().await {
            let frm = nxt?;
            let newchunk = frm.into_data()?;
            raw_collected.extend(newchunk.to_vec());
        } // end of loop
        let out = serde_json::from_slice::<JwkSet>(raw_collected.as_slice())?;
        Ok(out)
    } // end of fn request_new_keys

    pub fn merge(target: &mut JwkSet, new: JwkSet) -> (usize, usize) {
        let clone_kid = |item: &Jwk| -> Option<String> { item.common.key_id.clone() }; // filter out the items which don't have key ID
        let kids_iter_1 = target.keys.iter().filter_map(clone_kid);
        let kids_iter_2 = new.keys.iter().filter_map(clone_kid);
        let kidset1: HashSet<String, RandomState> = HashSet::from_iter(kids_iter_1);
        let kidset2 = HashSet::from_iter(kids_iter_2);
        let added = kidset2.difference(&kidset1).collect::<Vec<_>>();
        let discarding = kidset1.difference(&kidset2).collect::<Vec<_>>();
        let out = (discarding.len(), added.len());
        let _discarded = discarding
            .into_iter()
            .filter_map(|del_kid| {
                target
                    .keys
                    .iter()
                    .position(|item| {
                        item.common
                            .key_id
                            .as_ref()
                            .map_or(false, |t_kid| del_kid.as_str() == t_kid.as_str())
                    })
                    .map(|idx| target.keys.remove(idx))
            })
            .collect::<Vec<_>>();
        let new_iter = new.keys.into_iter().filter(|item| {
            item.common
                .key_id
                .as_ref()
                .map_or(false, |id| added.contains(&id))
        });
        target.keys.extend(new_iter);
        out
    } // end of fn merge
} // end of impl AppAuthKeystore
