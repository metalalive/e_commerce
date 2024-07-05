mod currency_exchange;

use std::boxed::Box;
use std::result::Result;
use std::sync::Arc;
use std::vec::Vec;

use ecommerce_common::confidentiality::AbstractConfidentiality;
use ecommerce_common::config::App3rdPartyCfg;
use ecommerce_common::error::AppErrorCode;
use ecommerce_common::logging::AppLogContext;

use crate::error::AppError;

pub use currency_exchange::AbstractCurrencyExchange;
use currency_exchange::AppCurrencyExchange;

pub(crate) fn app_currency_context(
    cfgs: &Option<Vec<Arc<App3rdPartyCfg>>>,
    cfdntl: Arc<Box<dyn AbstractConfidentiality>>,
    logctx: Arc<AppLogContext>,
) -> Result<Box<dyn AbstractCurrencyExchange>, AppError> {
    let _cfgs = cfgs.as_ref().cloned().ok_or(AppError {
        code: AppErrorCode::MissingConfig,
        detail: Some("3rd-parties".to_string()),
    })?;
    let obj = AppCurrencyExchange::try_build(_cfgs, cfdntl, logctx)?;
    Ok(Box::new(obj))
}
