use std::marker::{Send, Sync};
use std::result::Result;
use std::sync::Arc;

use async_trait::async_trait;

use ecommerce_common::logging::AppLogContext;

use crate::api::web::dto::PaymentMethodErrorReason;
use crate::model::ChargeLineModelSet;

#[async_trait]
pub trait AbstractPaymentProcessor: Send + Sync {
    async fn pay_in_start(
        &self,
        cline_set: &ChargeLineModelSet,
    ) -> Result<AppProcessorPayInResult, AppProcessorError>;
}

struct AppProcessorContext;

pub struct AppProcessorError {
    pub reason: PaymentMethodErrorReason,
}

pub struct AppProcessorPayInResult {
    pub completed: bool,
}

impl AppProcessorContext {
    pub fn new(_logctx: Arc<AppLogContext>) -> Result<Self, AppProcessorError> {
        Ok(Self)
    }
}

#[async_trait]
impl AbstractPaymentProcessor for AppProcessorContext {
    async fn pay_in_start(
        &self,
        _cline_set: &ChargeLineModelSet,
    ) -> Result<AppProcessorPayInResult, AppProcessorError> {
        let reason = PaymentMethodErrorReason::ProcessorFailure;
        Err(AppProcessorError { reason })
    }
}

pub(crate) fn app_processor_context(
    logctx: Arc<AppLogContext>,
) -> Result<Box<dyn AbstractPaymentProcessor>, AppProcessorError> {
    let proc = AppProcessorContext::new(logctx)?;
    Ok(Box::new(proc))
}
