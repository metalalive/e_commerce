use std::future::Future;
use std::marker::{Send, Sync};
use std::result::Result;
use std::sync::Arc;
use std::vec::Vec;

use crate::error::AppError;
use ecommerce_common::api::dto::CurrencyDto;
use ecommerce_common::confidentiality::AbstractConfidentiality;
use ecommerce_common::config::App3rdPartyCfg;
use ecommerce_common::error::AppErrorCode;
use ecommerce_common::logging::AppLogContext;

pub trait AbstractCurrencyExchange: Send + Sync {
    // refresh exchange rates of currencies specified in implementation
    fn refresh(&self) -> impl Future<Output = Result<(), AppError>>
    where
        Self: Sized;
    // retrieve rates with chosen currencies on creating each new order
    fn convert(
        &self,
        src: CurrencyDto,
        dst: CurrencyDto,
    ) -> impl Future<Output = Result<f64, AppError>>
    where
        Self: Sized;
}

pub(super) struct AppCurrencyExchange {
    _host: String,
    _port: u16,
    _app_id: String,
    _logctx: Arc<AppLogContext>,
}

impl AbstractCurrencyExchange for AppCurrencyExchange {
    async fn refresh(&self) -> Result<(), AppError> {
        let e = AppError {
            code: AppErrorCode::NotImplemented,
            detail: None,
        };
        Err(e)
    }
    async fn convert(&self, _src: CurrencyDto, _dst: CurrencyDto) -> Result<f64, AppError> {
        let e = AppError {
            code: AppErrorCode::NotImplemented,
            detail: None,
        };
        Err(e)
    }
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
        Ok(Self {
            _host: cfg_found.host.clone(),
            _port: cfg_found.port,
            _app_id,
            _logctx,
        })
    } // end of fn try-build
} // end of impl AppCurrencyExchange
