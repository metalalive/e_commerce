mod client;
mod resources;

use std::boxed::Box;
use std::result::Result;
use std::sync::Arc;

use async_trait::async_trait;
use chrono::{DateTime, Duration, Utc};
use http_body_util::Full;
use hyper::body::Bytes;
use hyper::header::{HeaderName, HeaderValue};
use hyper::Method;
use tokio_native_tls::{native_tls, TlsConnector as TlsConnectorWrapper};

use ecommerce_common::confidentiality::AbstractConfidentiality;
use ecommerce_common::logging::{app_log_event, AppLogContext, AppLogLevel};

use self::client::AppStripeClient;
use self::resources::{
    CheckoutSession, CheckoutSessionMode, CreateCheckoutSession, CreateCheckoutSessionLineItem,
    CreateCheckoutSessionPaymentIntentData,
};
use super::{AppProcessorError, AppProcessorErrorReason, AppProcessorPayInResult, BaseClientError};
use crate::api::web::dto::{
    PaymentMethodRespDto, StripeCheckoutSessionReqDto, StripeCheckoutSessionRespDto,
    StripeCheckoutUImodeDto,
};
use crate::model::{
    BuyerPayInState, ChargeBuyerModel, ChargeMethodModel, ChargeMethodStripeModel,
    StripeCheckoutPaymentStatusModel, StripeSessionStatusModel,
};

const HEADER_NAME_IDEMPOTENCY: &str = "Idempotency-Key";
const CHECKOUT_SESSION_MIN_SECONDS: i64 = 1800;

#[async_trait]
pub(super) trait AbstStripeContext: Send + Sync {
    async fn pay_in_start(
        &self,
        req: &StripeCheckoutSessionReqDto,
        meta: &ChargeBuyerModel,
    ) -> Result<(AppProcessorPayInResult, ChargeMethodModel), AppProcessorError>;
}

pub(super) struct AppProcessorStripeCtx {
    host: String,
    port: u16,
    secure_connector: TlsConnectorWrapper,
    api_key: String,
    logctx: Arc<AppLogContext>,
}

impl AppProcessorStripeCtx {
    pub(super) fn try_build(
        host: &str,
        port: u16,
        confidential_path: &str,
        cfdntl: Arc<Box<dyn AbstractConfidentiality>>,
        logctx: Arc<AppLogContext>,
    ) -> Result<Box<dyn AbstStripeContext>, AppProcessorError> {
        let serial = cfdntl
            .try_get_payload(confidential_path)
            .map_err(|_e| AppProcessorError {
                reason: AppProcessorErrorReason::MissingCredential,
            })?;
        let api_key =
            serde_json::from_str::<String>(serial.as_str()).map_err(|_e| AppProcessorError {
                reason: AppProcessorErrorReason::CredentialCorrupted,
            })?;
        let secure_connector = {
            let mut builder = native_tls::TlsConnector::builder();
            builder.min_protocol_version(Some(native_tls::Protocol::Tlsv12));
            let c = builder
                .build()
                .map_err(|e| BaseClientError { reason: e.into() })
                .map_err(|e| AppProcessorError { reason: e.into() })?;
            c.into()
        };
        let m = Self {
            host: host.to_string(),
            port,
            secure_connector,
            api_key,
            logctx,
        };
        Ok(Box::new(m))
    } // end of fn try-build
} // end of impl AppProcessorStripeCtx

#[async_trait]
impl AbstStripeContext for AppProcessorStripeCtx {
    async fn pay_in_start(
        &self,
        req: &StripeCheckoutSessionReqDto,
        charge_buyer: &ChargeBuyerModel,
    ) -> Result<(AppProcessorPayInResult, ChargeMethodModel), AppProcessorError> {
        let _logctx = &self.logctx;
        let is_embed_ui = matches!(req.ui_mode, StripeCheckoutUImodeDto::EmbeddedJs);
        let is_redirect_pg = matches!(req.ui_mode, StripeCheckoutUImodeDto::RedirectPage);
        if is_embed_ui {
            if req.success_url.is_some() {
                return Err(AppProcessorError {
                    reason: AppProcessorErrorReason::InvalidMethod(
                        "embed-ui-success-url".to_string(),
                    ),
                });
            }
            if req.return_url.is_none() {
                return Err(AppProcessorError {
                    reason: AppProcessorErrorReason::InvalidMethod(
                        "embed-ui-missing-return-url".to_string(),
                    ),
                });
            }
        }
        if is_redirect_pg && req.return_url.is_some() {
            return Err(AppProcessorError {
                reason: AppProcessorErrorReason::InvalidMethod(
                    "redirect-page-with-return-url".to_string(),
                ),
            });
        }
        let buyer_currency = charge_buyer.get_buyer_currency().ok_or(AppProcessorError {
            reason: AppProcessorErrorReason::MissingCurrency(charge_buyer.meta.owner),
        })?;

        let charge_token_serial =
            charge_buyer
                .meta
                .token()
                .0
                .iter()
                .fold(String::new(), |mut dst, num| {
                    let hex = format!("{:02x}", num);
                    dst += hex.as_str();
                    dst
                });

        app_log_event!(
            _logctx,
            AppLogLevel::DEBUG,
            "charge-token: {}",
            &charge_token_serial
        );

        let body_obj = CreateCheckoutSession {
            client_reference_id: format!("{}-{}", charge_buyer.meta.owner, charge_buyer.meta.oid),
            currency: buyer_currency.label.clone(),
            customer: req.customer_id.clone(),
            expires_at: charge_buyer.meta.create_time.timestamp() + CHECKOUT_SESSION_MIN_SECONDS,
            cancel_url: req.cancel_url.clone(),
            success_url: req.success_url.clone(),
            return_url: req.return_url.clone(),
            line_items: charge_buyer
                .lines
                .iter()
                .map(|v| CreateCheckoutSessionLineItem::from((buyer_currency.label.clone(), v)))
                .collect(),
            payment_intent_data: CreateCheckoutSessionPaymentIntentData {
                transfer_group: Some(charge_token_serial.clone()),
            },
            mode: CheckoutSessionMode::Payment,
            ui_mode: (&req.ui_mode).into(),
        };
        let mut _client = AppStripeClient::<Full<Bytes>>::try_build(
            self.logctx.clone(),
            &self.secure_connector,
            self.host.clone(),
            self.port,
            self.api_key.clone(),
        )
        .await
        .map_err(|e| AppProcessorError { reason: e.into() })?;

        let hdrs = vec![(
            // header-name from-static does not allow uppercase word
            HeaderName::from_bytes(HEADER_NAME_IDEMPOTENCY.as_bytes()).unwrap(),
            HeaderValue::from_str(charge_token_serial.as_str()).unwrap(),
        )];
        let resp = _client
            .execute_form::<CheckoutSession, CreateCheckoutSession>(
                "/checkout/sessions",
                Method::POST,
                &body_obj,
                hdrs,
            )
            .await
            .map_err(|e| AppProcessorError { reason: e.into() })?;
        let time_now = Utc::now();
        let result = AppProcessorPayInResult {
            charge_id: charge_buyer.meta.token().0.to_vec(),
            method: PaymentMethodRespDto::Stripe(StripeCheckoutSessionRespDto {
                id: resp.id.clone(),
                redirect_url: resp.url,
                client_session: resp.client_secret,
            }),
            state: BuyerPayInState::ProcessorAccepted(time_now),
            completed: false,
        };
        let mthd_m = ChargeMethodModel::Stripe(ChargeMethodStripeModel {
            checkout_session_id: resp.id,
            session_state: resp.status,
            payment_state: resp.payment_status,
            payment_intent_id: resp.payment_intent,
            expiry: DateTime::from_timestamp(resp.expires_at, 0)
                .unwrap_or(time_now + Duration::seconds(CHECKOUT_SESSION_MIN_SECONDS)),
        });
        Ok((result, mthd_m))
    } // end of fn create_checkout_session
} // end of impl AppProcessorStripeCtx

pub(super) struct MockProcessorStripeCtx;

impl MockProcessorStripeCtx {
    pub(super) fn build() -> Box<dyn AbstStripeContext> {
        Box::new(Self)
    }
}

#[async_trait]
impl AbstStripeContext for MockProcessorStripeCtx {
    async fn pay_in_start(
        &self,
        req: &StripeCheckoutSessionReqDto,
        charge_buyer: &ChargeBuyerModel,
    ) -> Result<(AppProcessorPayInResult, ChargeMethodModel), AppProcessorError> {
        let (redirect_url, client_session) = match req.ui_mode {
            StripeCheckoutUImodeDto::RedirectPage => (Some("https://abc.new.au".to_string()), None),
            StripeCheckoutUImodeDto::EmbeddedJs => {
                (None, Some("mock-client-session-seq".to_string()))
            }
        };
        let checkout_session_id = "mock-stripe-checkout-session-id".to_string();
        let mthd_detail = StripeCheckoutSessionRespDto {
            id: checkout_session_id.clone(),
            redirect_url,
            client_session,
        };
        let result = AppProcessorPayInResult {
            charge_id: charge_buyer.meta.token().0.to_vec(),
            method: PaymentMethodRespDto::Stripe(mthd_detail),
            state: BuyerPayInState::ProcessorAccepted(charge_buyer.meta.create_time),
            completed: false,
        };
        let stripe_m = ChargeMethodStripeModel {
            checkout_session_id,
            payment_intent_id: "mock-stripe-payment-intent-id".to_string(),
            session_state: StripeSessionStatusModel::open,
            payment_state: StripeCheckoutPaymentStatusModel::unpaid,
            expiry: charge_buyer.meta.create_time + Duration::seconds(35),
        };
        let mthd_m = ChargeMethodModel::Stripe(stripe_m);
        Ok((result, mthd_m))
    }
} // end of impl MockProcessorStripeCtx
