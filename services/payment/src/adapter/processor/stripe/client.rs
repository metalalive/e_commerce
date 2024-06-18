use std::marker::{Send, Sync};
use std::result::Result;
use std::sync::Arc;

use http_body_util::{BodyExt, Full};
use hyper::body::Bytes;
use hyper::header::{HeaderName, HeaderValue, AUTHORIZATION};
use hyper::Method;
use serde::de::DeserializeOwned;
use serde::ser::Serialize;

use crate::adapter::processor::base_client::BaseClientErrorReason;
use ecommerce_common::logging::AppLogContext;

use super::super::base_client::{BaseClient, BaseClientError};

pub(super) struct AppStripeClient<B> {
    logctx: Arc<AppLogContext>,
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
        host: String,
        port: u16,
        secret_key: String,
    ) -> Result<Self, BaseClientError> {
        let _base_client = BaseClient::<B>::try_build(logctx.clone(), host, port).await?;
        Ok(Self {
            logctx,
            secret_key,
            _base_client,
        })
    }
}

impl AppStripeClient<Full<Bytes>> {
    pub(super) async fn execute_form<D, S>(
        &mut self,
        path: &str,
        method: Method,
        body_obj: &S,
        mut headers: Vec<(HeaderName, HeaderValue)>,
    ) -> Result<D, BaseClientError>
    where
        D: DeserializeOwned + Send + 'static,
        S: Serialize + Send + 'static,
    {
        let value = format!("Bearer {}", self.secret_key.as_str());
        let pair = (
            AUTHORIZATION,
            HeaderValue::from_str(value.as_str()).map_err(|_e| BaseClientError {
                reason: BaseClientErrorReason::HttpRequest("auth-header-parse-fail".to_string()),
            })?,
        );
        headers.push(pair);
        self._base_client
            .execute_form(path, method, body_obj, headers)
            .await
    }
} // end of impl AppStripeClient
