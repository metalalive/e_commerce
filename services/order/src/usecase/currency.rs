use std::boxed::Box;
use std::result::Result;
use std::sync::Arc;

use ecommerce_common::api::dto::CurrencyDto;
use ecommerce_common::logging::{app_log_event, AppLogContext, AppLogLevel};

use crate::adapter::thirdparty::AbstractCurrencyExchange;
use crate::error::AppError;
use crate::repository::AbsCurrencyRepo;

pub struct CurrencyRateRefreshUseCase;

impl CurrencyRateRefreshUseCase {
    pub async fn execute(
        repo: Box<dyn AbsCurrencyRepo>,
        exrate_ctx: Arc<Box<dyn AbstractCurrencyExchange>>,
        logctx: Arc<AppLogContext>,
    ) -> Result<(), AppError> {
        let chosen = vec![
            CurrencyDto::USD,
            CurrencyDto::IDR,
            CurrencyDto::INR,
            CurrencyDto::TWD,
            CurrencyDto::THB,
        ];
        let ms = exrate_ctx.refresh(chosen).await.map_err(|e| {
            app_log_event!(logctx, AppLogLevel::ERROR, "{:?}", e);
            e
        })?;
        repo.save(ms).await.map_err(|e| {
            app_log_event!(logctx, AppLogLevel::ERROR, "{:?}", e);
            e
        })
    }
} // end of impl CurrencyRateRefreshUseCase
