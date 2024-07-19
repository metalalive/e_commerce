mod base_client;
mod currency_exchange;

use std::boxed::Box;
use std::result::Result;
use std::sync::Arc;
use std::vec::Vec;

use ecommerce_common::confidentiality::AbstractConfidentiality;
use ecommerce_common::config::{App3rdPartyCfg, AppBasepathCfg};
use ecommerce_common::error::AppErrorCode;
use ecommerce_common::logging::AppLogContext;

use crate::error::AppError;

pub use currency_exchange::AbstractCurrencyExchange;
use currency_exchange::{AppCurrencyExchange, MockCurrencyExchange};

pub(crate) fn app_currency_context(
    cfg_basepath: &AppBasepathCfg,
    cfgs3pt: &Option<Vec<Arc<App3rdPartyCfg>>>,
    cfdntl: Arc<Box<dyn AbstractConfidentiality>>,
    logctx: Arc<AppLogContext>,
) -> Result<Box<dyn AbstractCurrencyExchange>, AppError> {
    const THIRD_PARTY_LABEL: &str = "openexchangerates";
    let _cfgs = cfgs3pt.as_ref().cloned().ok_or(AppError {
        code: AppErrorCode::MissingConfig,
        detail: Some("3rd-parties".to_string()),
    })?;
    let result = _cfgs.into_iter().find_map(|c| match &*c {
        App3rdPartyCfg::dev {
            name,
            host,
            port,
            confidentiality_path,
        } => {
            if name.to_lowercase().as_str() == THIRD_PARTY_LABEL {
                let r = AppCurrencyExchange::try_build(
                    host.clone(),
                    *port,
                    confidentiality_path.clone(),
                    cfdntl.clone(),
                    logctx.clone(),
                )
                .map(|v| {
                    let o: Box<dyn AbstractCurrencyExchange> = Box::new(v);
                    o
                });
                Some(r)
            } else {
                None
            }
        }
        App3rdPartyCfg::test { name, data_src } => {
            if name.to_lowercase().as_str() == THIRD_PARTY_LABEL {
                let r = MockCurrencyExchange::try_build(cfg_basepath, data_src.clone()).map(|v| {
                    let o: Box<dyn AbstractCurrencyExchange> = Box::new(v);
                    o
                });
                Some(r)
            } else {
                None
            }
        }
    }); // end of find-map
    let ctx = result.ok_or(AppError {
        code: AppErrorCode::MissingConfig,
        detail: Some("currency-exchange".to_string()),
    })??;
    Ok(ctx)
} // end of  fn app_currency_context
