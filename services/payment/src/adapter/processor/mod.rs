use std::marker::{Send, Sync};
use std::result::Result;
use std::sync::Arc;

use async_trait::async_trait;

use chrono::Local;
use ecommerce_common::logging::{app_log_event, AppLogContext, AppLogLevel};

use crate::api::web::dto::{ChargeRespDto, PaymentMethodErrorReason, PaymentMethodRespDto};
use crate::model::{BuyerPayInState, ChargeBuyerModel};

#[async_trait]
pub trait AbstractPaymentProcessor: Send + Sync {
    async fn pay_in_start(
        &self,
        cline_set: &ChargeBuyerModel,
    ) -> Result<AppProcessorPayInResult, AppProcessorError>;
}

struct AppProcessorContext {
    _logctx: Arc<AppLogContext>,
}

pub struct AppProcessorError {
    pub reason: PaymentMethodErrorReason,
}

pub struct AppProcessorPayInResult {
    pub charge_id: Vec<u8>,
    pub method: PaymentMethodRespDto,
    pub state: BuyerPayInState,
    pub completed: bool,
}

impl From<AppProcessorPayInResult> for ChargeRespDto {
    fn from(value: AppProcessorPayInResult) -> Self {
        let ctime = value
            .state
            .create_time()
            .unwrap_or(Local::now().fixed_offset());
        Self {
            id: value.charge_id,
            method: value.method,
            create_time: ctime,
        }
    }
}

impl AppProcessorContext {
    pub fn new(_logctx: Arc<AppLogContext>) -> Result<Self, AppProcessorError> {
        Ok(Self { _logctx })
    }
}

#[async_trait]
impl AbstractPaymentProcessor for AppProcessorContext {
    async fn pay_in_start(
        &self,
        _cline_set: &ChargeBuyerModel,
    ) -> Result<AppProcessorPayInResult, AppProcessorError> {
        let logctx_p = &self._logctx;
        app_log_event!(logctx_p, AppLogLevel::ERROR, "not-implemented-yet");
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
