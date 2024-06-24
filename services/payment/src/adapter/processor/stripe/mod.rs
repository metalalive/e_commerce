mod client;
mod resources;

use std::boxed::Box;
use std::result::Result;
use std::sync::Arc;

use chrono::Utc;
use http_body_util::Full;
use hyper::body::Bytes;
use hyper::header::{HeaderName, HeaderValue};
use hyper::Method;
use tokio_native_tls::{native_tls, TlsConnector as TlsConnectorWrapper};

use ecommerce_common::confidentiality::AbstractConfidentiality;
use ecommerce_common::config::App3rdPartyCfg;
use ecommerce_common::logging::{app_log_event, AppLogContext, AppLogLevel};

use self::client::AppStripeClient;
use self::resources::{
    CheckoutSession, CheckoutSessionMode, CreateCheckoutSession,
    CreateCheckoutSessionPaymentIntentData,
};
use super::{AppProcessorError, AppProcessorErrorReason, AppProcessorPayInResult, BaseClientError};
use crate::api::web::dto::{
    PaymentCurrencyDto, PaymentMethodRespDto, StripeCheckoutSessionReqDto,
    StripeCheckoutSessionRespDto, StripeCheckoutUImodeDto,
};
use crate::model::{BuyerPayInState, ChargeBuyerModel};

const HEADER_NAME_IDEMPOTENCY: &str = "Idempotency-Key";

pub(super) struct AppProcessorStripeCtx {
    cfg: Arc<App3rdPartyCfg>,
    secure_connector: TlsConnectorWrapper,
    api_key: String,
    app_fee_amount: i64,
    logctx: Arc<AppLogContext>,
}

impl AppProcessorStripeCtx {
    pub(super) fn try_build(
        cfg: Arc<App3rdPartyCfg>,
        cfdntl: Arc<Box<dyn AbstractConfidentiality>>,
        logctx: Arc<AppLogContext>,
    ) -> Result<Self, AppProcessorError> {
        let confidential_path = cfg.confidentiality_path.as_str();
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
        Ok(Self {
            cfg,
            secure_connector,
            api_key,
            logctx,
            app_fee_amount: 12, // TODO, parameterize
        })
    } // end of fn try-build

    pub(super) async fn pay_in_start(
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
            currency: PaymentCurrencyDto::TWD, // TODO, finish implementation
            customer: req.customer_id.clone(),
            expires_at: meta.create_time.timestamp(),
            cancel_url: req.cancel_url.clone(),
            success_url: req.success_url.clone(),
            return_url: req.return_url.clone(),
            line_items: vec![
                //CreateCheckoutSessionLineItems {
                //    price: "price_1PVbLrK1DDCwdgSi36RFYb1C".to_string(),
                //    quantity: 3
                //}
            ], // TODO, finish implementation
            payment_intent_data: CreateCheckoutSessionPaymentIntentData {
                application_fee_amount: self.app_fee_amount,
                transfer_group: Some(charge_token_serial.clone()),
            },
            mode: CheckoutSessionMode::Payment,
            ui_mode: (&req.ui_mode).into(),
        };
        let mut _client = AppStripeClient::<Full<Bytes>>::try_build(
            self.logctx.clone(),
            &self.secure_connector,
            self.cfg.host.clone(),
            self.cfg.port,
            self.api_key.clone(),
        )
        .await
        .map_err(|e| AppProcessorError { reason: e.into() })?;

        let hdrs = vec![(
            // header-name from-static does not allow uppercase word
            HeaderName::from_bytes(HEADER_NAME_IDEMPOTENCY.as_bytes()).unwrap(),
            HeaderValue::from_str(charge_token_serial.as_str()).unwrap(),
        )];
        let _resp = _client
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
            method: PaymentMethodRespDto::Stripe(
                StripeCheckoutSessionRespDto {
                    redirect_url: None,
                    client_session: None,
                }, // TODO, finish implementation
            ),
            state: BuyerPayInState::ProcessorAccepted(Utc::now()),
            completed: false,
        };
        Ok(out)
    } // end of fn create_checkout_session
} // end of impl AppProcessorStripeCtx
