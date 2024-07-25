mod client;
mod resources;

use std::boxed::Box;
use std::result::Result;
use std::sync::Arc;

use async_trait::async_trait;
use chrono::Utc;
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
use crate::model::{BuyerPayInState, ChargeBuyerModel};

const HEADER_NAME_IDEMPOTENCY: &str = "Idempotency-Key";
const CHECKOUT_SESSION_MIN_SECONDS: i64 = 1800;

#[async_trait]
pub(super) trait AbstStripeContext: Send + Sync {
    async fn pay_in_start(
        &self,
        req: &StripeCheckoutSessionReqDto,
        meta: &ChargeBuyerModel,
    ) -> Result<AppProcessorPayInResult, AppProcessorError>;
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
        meta: &ChargeBuyerModel,
    ) -> Result<AppProcessorPayInResult, AppProcessorError> {
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
        let buyer_currency = meta.get_buyer_currency().ok_or(AppProcessorError {
            reason: AppProcessorErrorReason::MissingCurrency(meta.owner),
        })?;

        let charge_token_serial = meta.token.0.iter().fold(String::new(), |mut dst, num| {
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
            client_reference_id: format!("{}-{}", meta.owner, meta.oid),
            currency: buyer_currency.label.clone(),
            customer: req.customer_id.clone(),
            expires_at: meta.create_time.timestamp() + CHECKOUT_SESSION_MIN_SECONDS,
            cancel_url: req.cancel_url.clone(),
            success_url: req.success_url.clone(),
            return_url: req.return_url.clone(),
            line_items: meta
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
        let out = AppProcessorPayInResult {
            charge_id: meta.token.0.to_vec(),
            method: PaymentMethodRespDto::Stripe(StripeCheckoutSessionRespDto {
                id: resp.id,
                redirect_url: resp.url,
                client_session: resp.client_secret,
            }),
            state: BuyerPayInState::ProcessorAccepted(Utc::now()),
            completed: false,
        };
        Ok(out)
    } // end of fn create_checkout_session
} // end of impl AppProcessorStripeCtx
