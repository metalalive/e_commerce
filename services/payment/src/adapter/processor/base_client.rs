use std::boxed::Box;
use std::io::{Error as IoError, ErrorKind};
use std::marker::{Send, Sync};
use std::result::Result;
use std::sync::Arc;

use http_body_util::BodyExt;
use http_body_util::Full;
use hyper::body::Bytes;
use hyper::{Error as HyperError, Method, Request};
// TODO, switch to http2
use hyper::client::conn::http1::{handshake, SendRequest};
use hyper::header::{HeaderName, HeaderValue, HOST};
use hyper_util::rt::TokioIo;
use serde::de::DeserializeOwned;
use serde::ser::Serialize;
use tokio::net::TcpStream;
use tokio_native_tls::{native_tls, TlsConnector};

use ecommerce_common::logging::{app_log_event, AppLogContext, AppLogLevel};

#[derive(Debug)]
pub enum BaseClientErrorReason {
    TcpNet(ErrorKind, String),
    SysIo(ErrorKind, String),
    Http {
        sender_closed: bool,
        parse_error: bool,
        req_cancelled: bool,
        messasge_corrupted: bool,
        timeout: bool,
        detail: String,
    },
    HttpRequest(String),
    Tls(String),
    SerialiseFailure(String),
    DeserialiseFailure(String, u16),
}

impl From<IoError> for BaseClientErrorReason {
    fn from(value: IoError) -> Self {
        let ekind = value.kind();
        match &ekind {
            ErrorKind::TimedOut
            | ErrorKind::AddrInUse
            | ErrorKind::NotConnected
            | ErrorKind::ConnectionReset
            | ErrorKind::ConnectionRefused
            | ErrorKind::ConnectionAborted => Self::TcpNet(ekind, value.to_string()),
            _others => Self::SysIo(ekind, value.to_string()),
        }
    }
}
impl From<HyperError> for BaseClientErrorReason {
    fn from(value: HyperError) -> Self {
        Self::Http {
            sender_closed: value.is_closed(),
            parse_error: value.is_parse_status() | value.is_parse(),
            timeout: value.is_timeout(),
            messasge_corrupted: value.is_incomplete_message() | value.is_body_write_aborted(),
            req_cancelled: value.is_canceled(),
            detail: value.to_string(),
        }
    }
}

impl From<native_tls::Error> for BaseClientErrorReason {
    fn from(value: native_tls::Error) -> Self {
        Self::Tls(value.to_string())
    }
}

#[derive(Debug)]
pub struct BaseClientError {
    pub reason: BaseClientErrorReason,
}

pub(super) struct BaseClient<B> {
    req_sender: SendRequest<B>,
    _connector: TlsConnector,
    logctx: Arc<AppLogContext>,
    host: String,
    port: u16,
    conn_closed: bool,
}

impl<B> BaseClient<B>
where
    B: BodyExt + Send + 'static,
    B::Data: Send,
    B::Error: std::error::Error + Send + Sync,
{
    pub(super) async fn try_build(
        logctx: Arc<AppLogContext>,
        secure_connector: &TlsConnector,
        host: String,
        port: u16,
    ) -> Result<Self, BaseClientError> {
        let logctx_cpy = logctx.clone();
        let tcp_stream = TcpStream::connect((host.as_str(), port))
            .await
            .map_err(|e| {
                app_log_event!(
                    logctx_cpy,
                    AppLogLevel::ERROR,
                    "tcp-conn-err, {host}:{port}, {:?}",
                    &e
                );
                BaseClientError { reason: e.into() }
            })?;
        let tls_stream = secure_connector
            .connect(host.as_str(), tcp_stream)
            .await
            .map_err(|e| BaseClientError { reason: e.into() })?;
        let io_adapter = TokioIo::new(tls_stream);
        let (req_sender, connector) = handshake(io_adapter)
            .await
            .map_err(|e| BaseClientError { reason: e.into() })?;
        let host_cpy = host.clone();
        let fut = Box::pin(async move {
            if let Err(e) = connector.await {
                app_log_event!(
                    logctx_cpy,
                    AppLogLevel::WARNING,
                    "remote server: {host_cpy}:{port}, {:?}",
                    e
                );
            }
            app_log_event!(logctx_cpy, AppLogLevel::DEBUG, "connector-end");
        });
        let _handle = tokio::spawn(fut);
        Ok(Self {
            // TODO, keep `io-adapter` instead of app-level request sender
            req_sender,
            _connector: secure_connector.clone(),
            logctx,
            host,
            port,
            conn_closed: false,
        })
    } // end of fn try-build

    async fn _execute<T: DeserializeOwned + Send + 'static>(
        &mut self,
        req: Request<B>,
    ) -> Result<T, BaseClientError> {
        let logctx_p = &self.logctx;
        let uri_log = req.uri().to_string();

        // TODO, initiate http handshake at here
        let mut resp = self.req_sender.send_request(req).await.map_err(|e| {
            self.conn_closed = e.is_closed() | e.is_timeout();
            app_log_event!(logctx_p, AppLogLevel::WARNING, "{:?}", e);
            BaseClientError { reason: e.into() }
        })?; // TODO, return raw response body
        let mut raw_collected = Vec::<u8>::new();
        while let Some(nxt) = resp.frame().await {
            let frm = nxt.map_err(|e| BaseClientError { reason: e.into() })?;
            let newchunk = frm.into_data().map_err(|failed_frame| {
                app_log_event!(
                    logctx_p,
                    AppLogLevel::ERROR,
                    "data:{}, trailers:{}",
                    failed_frame.is_data(),
                    failed_frame.is_trailers()
                );
                BaseClientError {
                    reason: BaseClientErrorReason::Http {
                        sender_closed: false,
                        parse_error: true,
                        req_cancelled: false,
                        messasge_corrupted: false,
                        timeout: false,
                        detail: "frame-corrupted".to_string(),
                    },
                }
            })?;
            raw_collected.extend(newchunk.to_vec());
        } // end of loop
        let status_code = resp.status();
        if status_code.is_client_error() {
            app_log_event!(
                logctx_p,
                AppLogLevel::INFO,
                "server:{}:{}, uri:{}",
                self.host.as_str(),
                self.port,
                uri_log
            );
        } else if status_code.is_server_error() {
            app_log_event!(
                logctx_p,
                AppLogLevel::WARNING,
                "server:{}:{}, uri:{}",
                self.host.as_str(),
                self.port,
                uri_log
            );
        }
        let out = serde_json::from_slice::<T>(raw_collected.as_slice()).map_err(|_e| {
            let reason = match String::from_utf8(raw_collected) {
                Ok(v) => BaseClientErrorReason::DeserialiseFailure(v, status_code.as_u16()),
                Err(_e) => BaseClientErrorReason::Http {
                    sender_closed: false,
                    parse_error: true,
                    req_cancelled: false,
                    timeout: false,
                    messasge_corrupted: true,
                    detail: "resp-body-complete-corrupt".to_string(),
                },
            };
            BaseClientError { reason }
        })?; //TODO, deserialise in specific 3rd-party client, different processors
             // applies different deserialisation format
        Ok(out)
    } // end of fn execute
} // end of impl BaseClient

impl BaseClient<Full<Bytes>> {
    pub(super) async fn execute_form<D, S>(
        &mut self,
        path: &str,
        method: Method,
        body_obj: &S,
        headers: Vec<(HeaderName, HeaderValue)>,
    ) -> Result<D, BaseClientError>
    where
        D: DeserializeOwned + Send + 'static,
        S: Serialize + Send + 'static,
    {
        let body = serde_qs::to_string(body_obj)
            .map(|v| Bytes::copy_from_slice(v.as_bytes()))
            .map(Full::new)
            .map_err(|e| BaseClientError {
                reason: BaseClientErrorReason::SerialiseFailure(e.to_string()),
            })?;
        let mut req = Request::builder()
            .method(method)
            .uri(path)
            .body(body)
            .map_err(|e| BaseClientError {
                reason: BaseClientErrorReason::HttpRequest(e.to_string()),
            })?; // hyper error is vague, TODO improve the detail
        let hdr_map = req.headers_mut();
        headers
            .into_iter()
            .map(|(k, v)| {
                let _old = hdr_map.insert(k, v);
            })
            .count();
        let _discarded = hdr_map.insert(HOST, HeaderValue::from_str(self.host.as_str()).unwrap()); // required in case the 3rd-party remote server sits behind reverse proxy
                                                                                                   // server (e.g. CDN)
        let resp = self._execute::<D>(req).await?;
        Ok(resp)
    }
} // end of impl BaseClient
