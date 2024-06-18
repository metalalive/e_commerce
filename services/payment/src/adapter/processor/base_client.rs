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
use hyper::header::{HeaderName, HeaderValue};
use hyper_util::rt::TokioIo;
use serde::de::DeserializeOwned;
use serde::ser::Serialize;
use tokio::net::TcpStream;

use ecommerce_common::logging::{app_log_event, AppLogContext, AppLogLevel};

pub enum BaseClientErrorReason {
    TcpNet(ErrorKind),
    SysIo(ErrorKind),
    Http {
        sender_closed: bool,
        parse_error: bool,
        req_cancelled: bool,
        messasge_corrupted: bool,
        timeout: bool,
        detail: String,
    },
    HttpRequest(String),
    DeserialiseFailure,
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
            | ErrorKind::ConnectionAborted => Self::TcpNet(ekind),
            _others => Self::SysIo(ekind),
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

pub struct BaseClientError {
    pub reason: BaseClientErrorReason,
}

pub(super) struct BaseClient<B> {
    req_sender: SendRequest<B>,
    logctx: Arc<AppLogContext>,
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
        host: String,
        port: u16,
    ) -> Result<Self, BaseClientError> {
        let stream = TcpStream::connect((host.as_str(), port))
            .await
            .map_err(|e| BaseClientError { reason: e.into() })?;
        let io_adapter = TokioIo::new(stream);
        let (req_sender, connector) = handshake(io_adapter)
            .await
            .map_err(|e| BaseClientError { reason: e.into() })?;
        let logctx_cpy = logctx.clone();
        let fut = Box::pin(async move {
            if let Err(e) = connector.await {
                app_log_event!(
                    logctx_cpy,
                    AppLogLevel::WARNING,
                    "host:{host}, port:{port}, {:?}",
                    e
                )
            }
        });
        let _handle = tokio::spawn(fut);
        Ok(Self {
            req_sender,
            logctx,
            conn_closed: false,
        })
    }

    async fn _execute<T: DeserializeOwned + Send + 'static>(
        &mut self,
        req: Request<B>,
    ) -> Result<T, BaseClientError> {
        let logctx_p = &self.logctx;
        let mut resp = self.req_sender.send_request(req).await.map_err(|e| {
            self.conn_closed = e.is_closed() | e.is_timeout();
            app_log_event!(logctx_p, AppLogLevel::WARNING, "{:?}", e);
            BaseClientError { reason: e.into() }
        })?;
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
        let out = serde_json::from_slice::<T>(raw_collected.as_slice()).map_err(|_e| {
            BaseClientError {
                reason: BaseClientErrorReason::DeserialiseFailure,
            }
        })?;
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
        let body = serde_json::to_vec(body_obj)
            .map(|v| Bytes::copy_from_slice(&v))
            .map(|b| Full::new(b))
            .map_err(|e| BaseClientError {
                reason: BaseClientErrorReason::HttpRequest(e.to_string()),
            })?;
        let mut req = Request::builder()
            .method(method)
            .uri(path)
            .body(body)
            .map_err(|e| BaseClientError {
                reason: BaseClientErrorReason::HttpRequest(e.to_string()),
            })?;
        let hdr_map = req.headers_mut();
        headers
            .into_iter()
            .map(|(k, v)| {
                let _old = hdr_map.insert(k, v);
            })
            .count();
        let resp = self._execute::<D>(req).await?;
        Ok(resp)
    }
} // end of impl BaseClient
