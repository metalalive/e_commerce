mod base_client;
mod stripe;

use std::boxed::Box;
use std::marker::{Send, Sync};
use std::result::Result;
use std::sync::Arc;

use async_trait::async_trait;
use chrono::Local;
use ecommerce_common::confidentiality::AbstractConfidentiality;
use ecommerce_common::config::App3rdPartyCfg;
use ecommerce_common::logging::AppLogContext;

pub use self::base_client::{BaseClientError, BaseClientErrorReason};
use self::stripe::{AbstStripeContext, AppProcessorStripeCtx, MockProcessorStripeCtx};
use crate::api::web::dto::{
    ChargeCreateRespDto, PaymentMethodErrorReason, PaymentMethodReqDto, PaymentMethodRespDto,
};
use crate::model::{BuyerPayInState, Charge3partyModel, ChargeBuyerMetaModel, ChargeBuyerModel};

#[async_trait]
pub trait AbstractPaymentProcessor: Send + Sync {
    async fn pay_in_start(
        &self,
        charge_m: &ChargeBuyerModel,
        req_mthd: PaymentMethodReqDto,
    ) -> Result<(AppProcessorPayInResult, Charge3partyModel), AppProcessorError>;

    async fn pay_in_progress(
        &self,
        meta: &ChargeBuyerMetaModel,
    ) -> Result<Charge3partyModel, AppProcessorError>;
}

struct AppProcessorContext {
    _stripe: Box<dyn AbstStripeContext>,
    _logctx: Arc<AppLogContext>,
}

#[derive(Debug)]
pub enum AppProcessorErrorReason {
    InvalidConfig,
    MissingCredential,
    MissingCurrency(u32), // keep user ID that misses the currency snapshot
    CredentialCorrupted,
    NotSupport,
    NotImplemented,
    LowLvlNet(BaseClientError),
    InvalidMethod(String),
}

#[derive(Debug)]
pub struct AppProcessorError {
    pub reason: AppProcessorErrorReason,
}

pub struct AppProcessorPayInResult {
    pub charge_id: Vec<u8>,
    pub method: PaymentMethodRespDto,
    pub state: BuyerPayInState,
    pub completed: bool,
}

impl From<AppProcessorPayInResult> for ChargeCreateRespDto {
    fn from(value: AppProcessorPayInResult) -> Self {
        let id = value
            .charge_id
            .iter()
            .map(|b| format!("{:02x}", b))
            .collect::<Vec<_>>()
            .join("");
        let ctime = value
            .state
            .create_time()
            .unwrap_or(Local::now().fixed_offset());
        Self {
            id,
            method: value.method,
            create_time: ctime,
        }
    }
}

impl From<BaseClientError> for AppProcessorErrorReason {
    fn from(value: BaseClientError) -> Self {
        Self::LowLvlNet(value)
    }
}
impl From<AppProcessorErrorReason> for PaymentMethodErrorReason {
    fn from(value: AppProcessorErrorReason) -> Self {
        match value {
            AppProcessorErrorReason::NotSupport | AppProcessorErrorReason::NotImplemented => {
                Self::OperationRefuse
            }
            _others => Self::ProcessorFailure,
        }
    } // TODO, finish implementation
}

impl AppProcessorContext {
    fn new(
        cfgs3pt: Vec<Arc<App3rdPartyCfg>>,
        cfdntl: Arc<Box<dyn AbstractConfidentiality>>,
        _logctx: Arc<AppLogContext>,
    ) -> Result<Self, AppProcessorError> {
        let mut errors = Vec::new();
        let mut result_stripe = None;
        cfgs3pt
            .into_iter()
            .map(|c| match c.as_ref() {
                App3rdPartyCfg::dev {
                    name,
                    host,
                    port,
                    confidentiality_path,
                } => {
                    if result_stripe.is_none() && name.as_str().to_lowercase() == "stripe" {
                        result_stripe = AppProcessorStripeCtx::try_build(
                            host.as_str(),
                            *port,
                            confidentiality_path.as_str(),
                            cfdntl.clone(),
                            _logctx.clone(),
                        )
                        .map_err(|e| errors.push(e))
                        .ok();
                    }
                }
                App3rdPartyCfg::test { name, data_src: _ } => {
                    if result_stripe.is_none() && name.as_str().to_lowercase() == "stripe" {
                        result_stripe = Some(MockProcessorStripeCtx::build());
                    }
                }
            })
            .count();
        if errors.is_empty() {
            if let Some(_stripe) = result_stripe {
                Ok(Self { _logctx, _stripe })
            } else {
                Err(AppProcessorError {
                    reason: AppProcessorErrorReason::InvalidConfig,
                })
            }
        } else {
            Err(errors.remove(0))
        }
    } // end of fn new
} // end of impl AppProcessorContext

#[async_trait]
impl AbstractPaymentProcessor for AppProcessorContext {
    async fn pay_in_start(
        &self,
        charge_m: &ChargeBuyerModel,
        req_mthd: PaymentMethodReqDto,
    ) -> Result<(AppProcessorPayInResult, Charge3partyModel), AppProcessorError> {
        let out = match req_mthd {
            PaymentMethodReqDto::Stripe(c) => self._stripe.pay_in_start(&c, charge_m).await?,
        };
        Ok(out)
    }

    async fn pay_in_progress(
        &self,
        _meta: &ChargeBuyerMetaModel,
    ) -> Result<Charge3partyModel, AppProcessorError> {
        Err(AppProcessorError {
            reason: AppProcessorErrorReason::NotImplemented,
        })
    }
}

pub(crate) fn app_processor_context(
    cfg_3pt: &Option<Vec<Arc<App3rdPartyCfg>>>,
    cfdntl: Arc<Box<dyn AbstractConfidentiality>>,
    logctx: Arc<AppLogContext>,
) -> Result<Box<dyn AbstractPaymentProcessor>, AppProcessorError> {
    let _cfg_3pt = cfg_3pt.as_ref().cloned().ok_or(AppProcessorError {
        reason: AppProcessorErrorReason::InvalidConfig,
    })?;
    let proc = AppProcessorContext::new(_cfg_3pt, cfdntl, logctx)?;
    Ok(Box::new(proc))
}
