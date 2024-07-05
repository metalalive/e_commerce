use std::result::Result;
use std::sync::Arc;

use hyper::body::{Body as HyperBody, HttpBody};
use hyper::client::conn::{handshake, SendRequest};
use hyper::header::{HeaderName, HeaderValue, HOST};
use hyper::{Method, Request, StatusCode};
use tokio::net::TcpStream;
use tokio_native_tls::TlsConnector;

use ecommerce_common::error::AppErrorCode;
use ecommerce_common::logging::{app_log_event, AppLogContext, AppLogLevel};

use crate::error::AppError;

pub(super) struct BaseClient {
    req_sender: SendRequest<HyperBody>,
    host: String,
    port: u16,
    logctx: Arc<AppLogContext>,
    conn_closed: bool,
}

impl BaseClient {
    pub(super) async fn try_build(
        host: String,
        port: u16,
        secure_connector: &TlsConnector,
        logctx: Arc<AppLogContext>,
    ) -> Result<Self, AppError> {
        let tcp_stream = TcpStream::connect((host.as_str(), port))
            .await
            .map_err(|e| AppError {
                code: AppErrorCode::IOerror(e.kind()),
                detail: Some(e.to_string()),
            })?;
        let tls_stream = secure_connector
            .connect(host.as_str(), tcp_stream)
            .await
            .map_err(|e| AppError {
                code: AppErrorCode::CryptoFailure,
                detail: Some(e.to_string()),
            })?;
        let (req_sender, connector) = handshake(tls_stream).await.map_err(|e| AppError {
            code: AppErrorCode::HttpHandshakeFailure,
            detail: Some(e.to_string()),
        })?;
        let logctx_p = logctx.clone();
        let _handle = tokio::task::spawn(async move {
            if let Err(e) = connector.await {
                app_log_event!(
                    logctx_p,
                    AppLogLevel::ERROR,
                    "failed to start http connection: {:?}",
                    e
                );
            }
        });
        Ok(Self {
            req_sender,
            host,
            port,
            logctx,
            conn_closed: false,
        })
    } // end of fn try-build

    async fn _execute(
        &mut self,
        req: Request<HyperBody>,
    ) -> Result<(Vec<u8>, StatusCode), AppError> {
        let logctx_p = self.logctx.clone();
        let mut resp = self.req_sender.send_request(req).await.map_err(|e| {
            self.conn_closed = e.is_closed() || !e.is_connect();
            let detail = e.to_string();
            app_log_event!(
                logctx_p,
                AppLogLevel::WARNING,
                "host: {}, port:{}, detail:{}",
                &self.host,
                self.port,
                &detail
            );
            AppError {
                code: AppErrorCode::HttpHandshakeFailure,
                detail: Some(detail),
            }
        })?;
        let body = resp.body_mut();
        let mut raw_collected = Vec::new();
        while let Some(r) = body.data().await {
            let rawbytes = r.map_err(|e| AppError {
                code: AppErrorCode::DataCorruption,
                detail: Some(e.to_string()),
            })?;
            raw_collected.extend(rawbytes.to_vec());
        } // end of loop
        let status = resp.status();
        app_log_event!(
            logctx_p,
            AppLogLevel::DEBUG,
            "host: {}, port:{}, resp-status:{}",
            &self.host,
            self.port,
            status.as_u16()
        );
        Ok((raw_collected, status))
    } // end of fn _execute

    pub(super) async fn execute(
        &mut self,
        resource_path: &str,
        method: Method,
        headers: Vec<(HeaderName, HeaderValue)>,
        rawbody: Option<Vec<u8>>,
    ) -> Result<(Vec<u8>, StatusCode), AppError> {
        let body = if let Some(v) = rawbody {
            HyperBody::from(v)
        } else {
            HyperBody::empty()
        };
        let mut req = Request::builder()
            .uri(resource_path)
            .method(method)
            .body(body)
            .map_err(|e| AppError {
                code: AppErrorCode::HttpHandshakeFailure,
                detail: Some(e.to_string()),
            })?;
        let hdrs = req.headers_mut();
        headers
            .into_iter()
            .map(|(k, v)| {
                let _discard = hdrs.insert(k, v);
            })
            .count();
        let _discard = hdrs.insert(HOST, HeaderValue::from_str(self.host.as_str()).unwrap());
        self._execute(req).await
    } // end of fn execute
} // end of impl BaseClient
