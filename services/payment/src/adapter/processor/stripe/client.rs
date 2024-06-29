use std::marker::{Send, Sync};
use std::result::Result;
use std::sync::Arc;

use http_body_util::{BodyExt, Full};
use hyper::body::Bytes;
use hyper::header::{HeaderName, HeaderValue, ACCEPT, AUTHORIZATION, CONTENT_TYPE};
use hyper::Method;
use serde::de::DeserializeOwned;
use serde::ser::Serialize;
use tokio_native_tls::TlsConnector;

use crate::adapter::processor::base_client::BaseClientErrorReason;
use ecommerce_common::logging::AppLogContext;

use super::super::base_client::{BaseClient, BaseClientError};

const API_VERSION: &str = "v1";

pub(super) struct AppStripeClient<B> {
    secret_key: String,
    _base_client: BaseClient<B>,
}

impl<B> AppStripeClient<B>
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
        secret_key: String,
    ) -> Result<Self, BaseClientError> {
        let _base_client = BaseClient::<B>::try_build(logctx, secure_connector, host, port).await?;
        Ok(Self {
            secret_key,
            _base_client,
        })
    }
}

impl AppStripeClient<Full<Bytes>> {
    pub(super) async fn execute_form<D, S>(
        &mut self,
        resource_path: &str,
        method: Method,
        body_obj: &S,
        mut headers: Vec<(HeaderName, HeaderValue)>,
    ) -> Result<D, BaseClientError>
    where
        D: DeserializeOwned + Send + 'static,
        S: Serialize + Send + 'static,
    {
        let value = format!("Bearer {}", self.secret_key.as_str());
        let pairs = [
            (
                AUTHORIZATION,
                HeaderValue::from_str(value.as_str()).map_err(|_e| BaseClientError {
                    reason: BaseClientErrorReason::HttpRequest(
                        "auth-header-parse-fail".to_string(),
                    ),
                })?,
            ),
            (ACCEPT, HeaderValue::from_str("application/json").unwrap()),
            (
                CONTENT_TYPE,
                HeaderValue::from_str("application/x-www-form-urlencoded").unwrap(),
            ),
        ];
        headers.extend(pairs.into_iter());
        let uri = "/".to_string() + API_VERSION + resource_path;
        let body = serde_qs::to_string(body_obj)
            .map(|v| Bytes::copy_from_slice(v.as_bytes()))
            .map(Full::new)
            .map_err(|e| BaseClientError {
                reason: BaseClientErrorReason::SerialiseFailure(e.to_string()),
            })?;
        let (raw_collected, status_code) = self
            ._base_client
            .execute_form(uri.as_str(), method, body, headers)
            .await?;
        let out = serde_json::from_slice::<D>(raw_collected.as_slice()).map_err(|_e| {
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
    } // end of fn execute_form
} // end of impl AppStripeClient
