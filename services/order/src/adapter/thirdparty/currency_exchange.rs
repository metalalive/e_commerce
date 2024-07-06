use std::collections::HashMap;
use std::marker::{Send, Sync};
use std::result::Result;
use std::str::FromStr;
use std::sync::Arc;
use std::vec::Vec;

use async_trait::async_trait;
use hyper::header::{HeaderValue, AUTHORIZATION};
use hyper::Method;
use rust_decimal::Decimal;
use serde::Deserialize;
use serde_json::Number as JsnNum;
use tokio_native_tls::{native_tls, TlsConnector};

use ecommerce_common::api::dto::CurrencyDto;
use ecommerce_common::confidentiality::AbstractConfidentiality;
use ecommerce_common::config::App3rdPartyCfg;
use ecommerce_common::error::AppErrorCode;
use ecommerce_common::logging::{app_log_event, AppLogContext, AppLogLevel};

use super::base_client::BaseClient;
use crate::error::AppError;
use crate::model::{CurrencyModel, CurrencyModelSet};

#[async_trait]
pub trait AbstractCurrencyExchange: Send + Sync {
    // refresh exchange rates of currencies specified in implementation
    // the crate `async-trait` is still required since this method is invoked
    // through size-unknown trait object (not concrete type)
    async fn refresh(&self, chosen: Vec<CurrencyDto>) -> Result<CurrencyModelSet, AppError>;
}

pub(super) struct AppCurrencyExchange {
    _host: String,
    _port: u16,
    _app_id: String,
    _logctx: Arc<AppLogContext>,
    _secure_connector: TlsConnector,
}

#[derive(Deserialize)]
struct ExRateIntermediate {
    base: CurrencyDto,
    rates: HashMap<CurrencyDto, JsnNum>,
}

impl TryFrom<ExRateIntermediate> for CurrencyModelSet {
    type Error = AppError;
    fn try_from(value: ExRateIntermediate) -> Result<Self, Self::Error> {
        let ExRateIntermediate { base, rates } = value;
        let mut errors = vec![];
        let exchange_rates = rates
            .into_iter()
            .filter_map(|(name, v)| {
                Decimal::from_str(v.to_string().as_str())
                    .map(|v| (name, v))
                    .map_err(|e| {
                        errors.push(AppError {
                            code: AppErrorCode::DataCorruption,
                            detail: Some(e.to_string()),
                        });
                        0
                    })
                    .ok()
            })
            .map(|(name, rate)| CurrencyModel { rate, name })
            .collect::<Vec<_>>();
        if errors.is_empty() {
            Ok(CurrencyModelSet {
                base,
                exchange_rates,
            })
        } else {
            let e = errors.remove(0);
            Err(e)
        }
    } // end of fn try-from
} // end of impl CurrencyModelSet

#[async_trait]
impl AbstractCurrencyExchange for AppCurrencyExchange {
    async fn refresh(&self, chosen: Vec<CurrencyDto>) -> Result<CurrencyModelSet, AppError> {
        let symbols = chosen
            .into_iter()
            .map(|c| c.to_string())
            .collect::<Vec<_>>()
            .join(",");
        let auth_tok = {
            let s = format!("Token {}", &self._app_id);
            HeaderValue::from_str(s.as_str()).map_err(|e| AppError {
                code: AppErrorCode::InvalidInput,
                detail: Some(e.to_string()),
            })?
        };
        let mut _client = BaseClient::try_build(
            self._host.clone(),
            self._port,
            &self._secure_connector,
            self._logctx.clone(),
        )
        .await?;
        let headers = vec![(AUTHORIZATION, auth_tok)];
        let resource_path =
            format!("/api/latest.json?symbols={symbols}&prettyprint=false&show_alternative=false");
        let (rawbody, status) = _client
            .execute(resource_path.as_str(), Method::GET, headers, None)
            .await?;
        if status.is_success() {
            self._try_into_modelset(rawbody)
        } else {
            Err(AppError {
                code: AppErrorCode::HttpHandshakeFailure,
                detail: Some(format!(
                    "host: {}:{}, status:{}",
                    self._host.as_str(),
                    self._port,
                    status.as_u16()
                )),
            })
        }
    } // end of fn refresh
} // end of impl AppCurrencyExchange

impl AppCurrencyExchange {
    pub(super) fn try_build(
        cfgs: Vec<Arc<App3rdPartyCfg>>,
        cfdntl: Arc<Box<dyn AbstractConfidentiality>>,
        _logctx: Arc<AppLogContext>,
    ) -> Result<Self, AppError> {
        let cfg_found = cfgs
            .into_iter()
            .find(|c| c.name.to_lowercase().as_str() == "openexchangerates")
            .ok_or(AppError {
                code: AppErrorCode::MissingConfig,
                detail: Some("currency-exchange".to_string()),
            })?;
        let credential_path = cfg_found.confidentiality_path.as_str();
        let serial = cfdntl
            .try_get_payload(credential_path)
            .map_err(|e| AppError {
                code: e.code,
                detail: Some(e.detail),
            })?;
        let _app_id = serde_json::from_str::<String>(serial.as_str()).map_err(|_e| AppError {
            code: AppErrorCode::DataCorruption,
            detail: Some("credential-parse-failure".to_string()),
        })?;
        let _secure_connector = {
            let mut builder = native_tls::TlsConnector::builder();
            builder.min_protocol_version(Some(native_tls::Protocol::Tlsv12));
            let sc = builder.build().map_err(|e| AppError {
                code: AppErrorCode::InvalidInput,
                detail: Some(e.to_string()),
            })?;
            sc.into()
        };
        Ok(Self {
            _host: cfg_found.host.clone(),
            _port: cfg_found.port,
            _secure_connector,
            _app_id,
            _logctx,
        })
    } // end of fn try-build

    fn _try_into_modelset(&self, rawbody: Vec<u8>) -> Result<CurrencyModelSet, AppError> {
        let logctx_p = &self._logctx;
        let intermediate = serde_json::from_slice::<ExRateIntermediate>(&rawbody).map_err(|e| {
            let detail = e.to_string();
            app_log_event!(logctx_p, AppLogLevel::ERROR, "{}", &detail);
            AppError {
                code: AppErrorCode::DataCorruption,
                detail: Some(detail),
            }
        })?;
        let obj = CurrencyModelSet::try_from(intermediate).map_err(|e| {
            app_log_event!(logctx_p, AppLogLevel::ERROR, "{:?}", e);
            e
        })?;
        Ok(obj)
    }
} // end of impl AppCurrencyExchange
