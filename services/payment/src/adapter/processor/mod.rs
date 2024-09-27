mod base_client;
mod stripe;

use std::boxed::Box;
use std::marker::{Send, Sync};
use std::result::Result;
use std::sync::Arc;

use async_trait::async_trait;
use chrono::Local;
use ecommerce_common::api::rpc::dto::StoreProfileReplicaDto;
use ecommerce_common::confidentiality::AbstractConfidentiality;
use ecommerce_common::config::App3rdPartyCfg;
use ecommerce_common::logging::AppLogContext;

pub use self::base_client::{BaseClientError, BaseClientErrorReason};
use self::stripe::{AbstStripeContext, AppProcessorStripeCtx, MockProcessorStripeCtx};
use crate::api::web::dto::{
    CapturePay3partyRespDto, CapturePayRespDto, ChargeCreateRespDto, PaymentMethodErrorReason,
    PaymentMethodReqDto, PaymentMethodRespDto, StoreOnboardReqDto, StoreOnboardRespDto,
};
use crate::model::{
    BuyerPayInState, Charge3partyModel, ChargeBuyerMetaModel, ChargeBuyerModel,
    Merchant3partyModel, Payout3partyModel, PayoutModel,
};

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

    async fn onboard_merchant(
        &self,
        store_profile: StoreProfileReplicaDto,
        req_3pt: StoreOnboardReqDto,
    ) -> Result<AppProcessorMerchantResult, AppProcessorError>;

    async fn refresh_onboard_status(
        &self,
        m3pty: Merchant3partyModel,
        req_3pt: StoreOnboardReqDto,
    ) -> Result<AppProcessorMerchantResult, AppProcessorError>;

    async fn pay_out(
        &self,
        payout_m: PayoutModel,
    ) -> Result<AppProcessorPayoutResult, AppProcessorError>;
} // end of trait AbstractPaymentProcessor

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
    InvalidStoreProfileDto(Vec<String>),
    CorruptedTimeStamp(String, i64), // label and given incorrect timestamp
    AmountOverflow(String),
    ThirdParty(String),
}

#[derive(Debug)]
pub enum AppProcessorFnLabel {
    TryBuild,
    PayInStart,
    PayInProgress,
    PayOut,
    OnboardMerchant,
    RefreshOnboardStatus,
}

#[derive(Debug)]
pub struct AppProcessorError {
    pub reason: AppProcessorErrorReason,
    pub fn_label: AppProcessorFnLabel,
} //TODO, new field which specifies which 3rd party reports this error

pub struct AppProcessorPayInResult {
    pub charge_id: Vec<u8>,
    pub method: PaymentMethodRespDto,
    pub state: BuyerPayInState,
    pub completed: bool,
}

pub struct AppProcessorMerchantResult {
    dto: StoreOnboardRespDto,
    model: Merchant3partyModel,
}

pub struct AppProcessorPayoutResult {
    dto: CapturePayRespDto,
    model: PayoutModel,
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

impl AppProcessorMerchantResult {
    pub fn into_parts(self) -> (StoreOnboardRespDto, Merchant3partyModel) {
        let Self { dto, model } = self;
        (dto, model)
    }
}
impl AppProcessorPayoutResult {
    pub fn into_parts(self) -> (CapturePayRespDto, PayoutModel) {
        let Self { dto, model } = self;
        (dto, model)
    }
    pub fn new(dto: CapturePayRespDto, model: PayoutModel) -> Self {
        Self { dto, model }
    }
}

// TODO, conditional compilation only for testing purpose
// #[cfg(test)] // <- complier still fails to include this code even in test mode, FIXME
impl Default for AppProcessorMerchantResult {
    fn default() -> Self {
        let d = StoreOnboardRespDto::Unknown;
        let m = Merchant3partyModel::Unknown;
        Self { dto: d, model: m }
    }
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
                    fn_label: AppProcessorFnLabel::TryBuild,
                })
            }
        } else {
            Err(AppProcessorError {
                reason: errors.remove(0),
                fn_label: AppProcessorFnLabel::TryBuild,
            })
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
        let result = match req_mthd {
            PaymentMethodReqDto::Stripe(c) => self._stripe.pay_in_start(&c, charge_m).await,
        };
        result.map_err(|reason| AppProcessorError {
            reason,
            fn_label: AppProcessorFnLabel::PayInStart,
        })
    }

    async fn pay_in_progress(
        &self,
        meta: &ChargeBuyerMetaModel,
    ) -> Result<Charge3partyModel, AppProcessorError> {
        let result = match meta.method_3party() {
            Charge3partyModel::Stripe(c) => self
                ._stripe
                .pay_in_progress(c)
                .await
                .map(Charge3partyModel::Stripe),
            Charge3partyModel::Unknown => Err(AppProcessorErrorReason::InvalidMethod(
                "unknown".to_string(),
            )),
        };
        result.map_err(|reason| AppProcessorError {
            reason,
            fn_label: AppProcessorFnLabel::PayInProgress,
        })
    }

    async fn onboard_merchant(
        &self,
        profile: StoreProfileReplicaDto,
        req_3pt: StoreOnboardReqDto,
    ) -> Result<AppProcessorMerchantResult, AppProcessorError> {
        let result = match req_3pt {
            StoreOnboardReqDto::Stripe(req) => self._stripe.onboard_merchant(profile, req).await,
        };
        result.map_err(|reason| AppProcessorError {
            reason,
            fn_label: AppProcessorFnLabel::OnboardMerchant,
        })
    }

    async fn refresh_onboard_status(
        &self,
        m3pty: Merchant3partyModel,
        req_3pt: StoreOnboardReqDto,
    ) -> Result<AppProcessorMerchantResult, AppProcessorError> {
        let result = match (m3pty, req_3pt) {
            (Merchant3partyModel::Stripe(ms), StoreOnboardReqDto::Stripe(ds)) => {
                self._stripe.refresh_onboard_status(ms, ds).await
            }
            _others => {
                let msg = "given-model-dto-mismatch".to_string();
                Err(AppProcessorErrorReason::InvalidMethod(msg))
            }
        };
        result.map_err(|reason| AppProcessorError {
            reason,
            fn_label: AppProcessorFnLabel::RefreshOnboardStatus,
        })
    }

    async fn pay_out(
        &self,
        payout_m: PayoutModel,
    ) -> Result<AppProcessorPayoutResult, AppProcessorError> {
        let (p_inner, p3pt) = payout_m.into_parts();
        let result = match p3pt {
            Payout3partyModel::Stripe(s) => self
                ._stripe
                .pay_out(&p_inner, s)
                .await
                .map(Payout3partyModel::Stripe),
        };
        result
            .map_err(|reason| AppProcessorError {
                reason,
                fn_label: AppProcessorFnLabel::PayOut,
            })
            .map(|p3pt| {
                let expect = p_inner.amount_merchant();
                let dto = CapturePayRespDto {
                    store_id: p_inner.merchant_id(),
                    processor: CapturePay3partyRespDto::from(&p3pt),
                    amount: expect.0.to_string(),
                    currency: expect.2.label.clone(),
                };
                let payout_m = PayoutModel::from_parts(p_inner, p3pt);
                AppProcessorPayoutResult::new(dto, payout_m)
            })
    } // end of fn pay_out
} // end of impl AppProcessorContext

pub(crate) fn app_processor_context(
    cfg_3pt: &Option<Vec<Arc<App3rdPartyCfg>>>,
    cfdntl: Arc<Box<dyn AbstractConfidentiality>>,
    logctx: Arc<AppLogContext>,
) -> Result<Box<dyn AbstractPaymentProcessor>, AppProcessorError> {
    let _cfg_3pt = cfg_3pt.as_ref().cloned().ok_or(AppProcessorError {
        reason: AppProcessorErrorReason::InvalidConfig,
        fn_label: AppProcessorFnLabel::TryBuild,
    })?;
    let proc = AppProcessorContext::new(_cfg_3pt, cfdntl, logctx)?;
    Ok(Box::new(proc))
}
