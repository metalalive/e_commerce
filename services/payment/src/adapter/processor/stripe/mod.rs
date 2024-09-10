mod client;
mod mock;
mod resources;

use std::boxed::Box;
use std::result::Result;
use std::sync::Arc;

use async_trait::async_trait;
use chrono::{DateTime, Duration, Local, Utc};
use http_body_util::{Empty, Full};
use hyper::body::Bytes;
use hyper::header::{HeaderName, HeaderValue};
use hyper::Method;
use tokio_native_tls::{native_tls, TlsConnector as TlsConnectorWrapper};

use ecommerce_common::api::rpc::dto::StoreProfileReplicaDto;
use ecommerce_common::confidentiality::AbstractConfidentiality;
use ecommerce_common::logging::{app_log_event, AppLogContext, AppLogLevel};

use self::client::AppStripeClient;
pub(super) use self::mock::MockProcessorStripeCtx;
use self::resources::{
    AccountLink, AccountRequirement, AccountSettings, CheckoutSession, CheckoutSessionMode,
    ConnectAccount, CreateAccountLink, CreateCheckoutSession, CreateCheckoutSessionLineItem,
    CreateCheckoutSessionPaymentIntentData, CreateConnectAccount,
};
use super::{
    AppProcessorErrorReason, AppProcessorMerchantResult, AppProcessorPayInResult, BaseClientError,
};
use crate::api::web::dto::{
    PaymentMethodRespDto, StoreOnboardRespDto, StoreOnboardStripeReqDto,
    StripeCheckoutSessionReqDto, StripeCheckoutSessionRespDto, StripeCheckoutUImodeDto,
};
use crate::model::{
    BuyerPayInState, Charge3partyModel, Charge3partyStripeModel, ChargeBuyerModel,
    Merchant3partyModel, Merchant3partyStripeModel, StripeAccountSettingModel,
};

const HEADER_NAME_IDEMPOTENCY: &str = "Idempotency-Key";
const CHECKOUT_SESSION_MIN_SECONDS: i64 = 1800;
const ACCOUNT_LINK_EXPIRY_MIN_DAYS: i64 = 2;

#[async_trait]
pub(super) trait AbstStripeContext: Send + Sync {
    async fn pay_in_start(
        &self,
        req: &StripeCheckoutSessionReqDto,
        meta: &ChargeBuyerModel,
    ) -> Result<(AppProcessorPayInResult, Charge3partyModel), AppProcessorErrorReason>;

    async fn pay_in_progress(
        &self,
        detail3pty: &Charge3partyStripeModel,
    ) -> Result<Charge3partyStripeModel, AppProcessorErrorReason>;

    async fn onboard_merchant(
        &self,
        store_profile: StoreProfileReplicaDto,
        req: StoreOnboardStripeReqDto,
    ) -> Result<AppProcessorMerchantResult, AppProcessorErrorReason>;
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
    ) -> Result<Box<dyn AbstStripeContext>, AppProcessorErrorReason> {
        let serial = cfdntl
            .try_get_payload(confidential_path)
            .map_err(|_e| AppProcessorErrorReason::MissingCredential)?;
        let api_key = serde_json::from_str::<String>(serial.as_str())
            .map_err(|_e| AppProcessorErrorReason::CredentialCorrupted)?;
        let secure_connector = {
            let mut builder = native_tls::TlsConnector::builder();
            builder.min_protocol_version(Some(native_tls::Protocol::Tlsv12));
            let c = builder
                .build()
                .map_err(|e| BaseClientError { reason: e.into() })
                .map_err(AppProcessorErrorReason::from)?;
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

    fn map_log_err(&self, label: &str, e: BaseClientError) -> AppProcessorErrorReason {
        let logger = &self.logctx;
        app_log_event!(logger, AppLogLevel::ERROR, "{label}: {:?}", &e);
        AppProcessorErrorReason::from(e)
    }
} // end of impl AppProcessorStripeCtx

#[async_trait]
impl AbstStripeContext for AppProcessorStripeCtx {
    async fn pay_in_start(
        &self,
        req: &StripeCheckoutSessionReqDto,
        charge_buyer: &ChargeBuyerModel,
    ) -> Result<(AppProcessorPayInResult, Charge3partyModel), AppProcessorErrorReason> {
        let _logctx = &self.logctx;
        let is_embed_ui = matches!(req.ui_mode, StripeCheckoutUImodeDto::EmbeddedJs);
        let is_redirect_pg = matches!(req.ui_mode, StripeCheckoutUImodeDto::RedirectPage);
        if is_embed_ui {
            if req.success_url.is_some() {
                return Err(AppProcessorErrorReason::InvalidMethod(
                    "embed-ui-success-url".to_string(),
                ));
            }
            if req.return_url.is_none() {
                return Err(AppProcessorErrorReason::InvalidMethod(
                    "embed-ui-missing-return-url".to_string(),
                ));
            }
        }
        if is_redirect_pg && req.return_url.is_some() {
            return Err(AppProcessorErrorReason::InvalidMethod(
                "redirect-page-with-return-url".to_string(),
            ));
        }
        let buyer_currency =
            charge_buyer
                .get_buyer_currency()
                .ok_or(AppProcessorErrorReason::MissingCurrency(
                    charge_buyer.meta.owner(),
                ))?;

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
            client_reference_id: format!(
                "{}-{}",
                charge_buyer.meta.owner(),
                charge_buyer.meta.oid()
            ),
            currency: buyer_currency.label.clone(),
            customer: req.customer_id.clone(),
            expires_at: charge_buyer.meta.create_time().timestamp() + CHECKOUT_SESSION_MIN_SECONDS,
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
        .map_err(AppProcessorErrorReason::from)?;

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
            .map_err(|e| self.map_log_err("new-sess", e))?;
        let time_now = Utc::now();
        let result = AppProcessorPayInResult {
            charge_id: charge_buyer.meta.token().0.to_vec(),
            method: PaymentMethodRespDto::Stripe(StripeCheckoutSessionRespDto {
                id: resp.id.clone(),
                redirect_url: resp.url.clone(),
                client_session: resp.client_secret.clone(),
            }),
            state: BuyerPayInState::ProcessorAccepted(time_now),
            completed: false, // TODO, deprecated
        };
        let time_end = time_now + Duration::seconds(CHECKOUT_SESSION_MIN_SECONDS);
        let mthd_3pty = Charge3partyStripeModel::from((resp, time_end));
        Ok((result, Charge3partyModel::Stripe(mthd_3pty)))
    } // end of fn create_checkout_session

    async fn pay_in_progress(
        &self,
        old: &Charge3partyStripeModel,
    ) -> Result<Charge3partyStripeModel, AppProcessorErrorReason> {
        let mut _client = AppStripeClient::<Empty<Bytes>>::try_build(
            self.logctx.clone(),
            &self.secure_connector,
            self.host.clone(),
            self.port,
            self.api_key.clone(),
        )
        .await
        .map_err(AppProcessorErrorReason::from)?;
        let resource_path = format!("/checkout/sessions/{}", old.checkout_session_id);
        let new_session = _client
            .execute::<CheckoutSession>(resource_path.as_str(), Method::GET, Vec::new())
            .await
            .map_err(|e| self.map_log_err("refresh-sess", e))?;
        let arg = (new_session, old.expiry);
        Ok(Charge3partyStripeModel::from(arg))
    }

    async fn onboard_merchant(
        &self,
        store_profile: StoreProfileReplicaDto,
        req: StoreOnboardStripeReqDto,
    ) -> Result<AppProcessorMerchantResult, AppProcessorErrorReason> {
        // TODO, clean up the accounts which are disabled and expired
        let body_obj = CreateConnectAccount::try_from(store_profile)?;

        let mut _client = AppStripeClient::<Full<Bytes>>::try_build(
            self.logctx.clone(),
            &self.secure_connector,
            self.host.clone(),
            self.port,
            self.api_key.clone(),
        )
        .await
        .map_err(AppProcessorErrorReason::from)?;

        let acct = _client
            .execute_form::<ConnectAccount, CreateConnectAccount>(
                "/accounts",
                Method::POST,
                &body_obj,
                Vec::new(),
            )
            .await
            .map_err(|e| self.map_log_err("acct", e))?;

        let body_obj = CreateAccountLink::from((req, acct.id.as_str()));
        let acc_link = _client
            .execute_form::<AccountLink, CreateAccountLink>(
                "/account_links",
                Method::POST,
                &body_obj,
                Vec::new(),
            )
            .await
            .map_err(|e| self.map_log_err("acct-link", e))?;

        AppProcessorMerchantResult::try_from((acct, acc_link))
    } // end of fn onboard_merchant
} // end of impl AppProcessorStripeCtx

impl From<(CheckoutSession, DateTime<Utc>)> for Charge3partyStripeModel {
    fn from(value: (CheckoutSession, DateTime<Utc>)) -> Self {
        let (session, time_end) = value;
        Self {
            checkout_session_id: session.id,
            session_state: session.status,
            payment_state: session.payment_status,
            payment_intent_id: session.payment_intent,
            expiry: DateTime::from_timestamp(session.expires_at, 0).unwrap_or(time_end),
        }
    }
}

impl<'a> From<(&'a AccountRequirement, AccountLink)> for StoreOnboardRespDto {
    fn from(value: (&'a AccountRequirement, AccountLink)) -> Self {
        let (r, alink) = value;
        let expiry = DateTime::from_timestamp(alink.expires_at, 0)
            .unwrap_or(Local::now().to_utc() + Duration::days(ACCOUNT_LINK_EXPIRY_MIN_DAYS));
        Self::Stripe {
            fields_required: r.currently_due.clone(),
            disabled_reason: r.disabled_reason.clone(),
            url: Some(alink.url),
            expiry: Some(expiry),
        }
    }
}

impl From<AccountSettings> for StripeAccountSettingModel {
    fn from(value: AccountSettings) -> Self {
        let p = value.payouts;
        Self {
            payout_delay_days: p.schedule.delay_days,
            payout_interval: p.schedule.interval,
            debit_negative_balances: p.debit_negative_balances,
        }
    }
}
impl TryFrom<ConnectAccount> for Merchant3partyStripeModel {
    type Error = AppProcessorErrorReason;
    #[rustfmt::skip]
    fn try_from(value: ConnectAccount) -> Result<Self, Self::Error> {
        let ConnectAccount {
            id, country, email, capabilities,
            tos_acceptance, charges_enabled, payouts_enabled,
            details_submitted, created: created_ts, settings,
            requirements: _, type_: _,
        } = value;
        let created = DateTime::from_timestamp(created_ts, 0)
            .ok_or(AppProcessorErrorReason::CorruptedTimeStamp(
                "stripe.account.created".to_string(), created_ts,
            ))?;
        let tos_accepted = if let Some(orig) = tos_acceptance.date {
            let r = DateTime::from_timestamp(orig, 0)
                .ok_or(AppProcessorErrorReason::CorruptedTimeStamp(
                    "stripe.account.tos_acceptance.date".to_string(), orig,
                ))?;
            Some(r)
        } else {
            None
        };
        let settings = StripeAccountSettingModel::from(settings);
        let out = Self {
            id, country, email, capabilities, tos_accepted, settings,
            charges_enabled, payouts_enabled, details_submitted, created,
        };
        Ok(out)
    } // end of fn try-from
} // end of impl Merchant3partyStripeModel
impl TryFrom<ConnectAccount> for Merchant3partyModel {
    type Error = AppProcessorErrorReason;
    fn try_from(value: ConnectAccount) -> Result<Self, Self::Error> {
        let m = Merchant3partyStripeModel::try_from(value)?;
        Ok(Self::Stripe(m))
    }
}
impl TryFrom<(ConnectAccount, AccountLink)> for AppProcessorMerchantResult {
    type Error = AppProcessorErrorReason;
    fn try_from(value: (ConnectAccount, AccountLink)) -> Result<Self, Self::Error> {
        let (acct, alink) = value;
        let d = StoreOnboardRespDto::from((&acct.requirements, alink));
        let m = Merchant3partyModel::try_from(acct)?;
        Ok(Self { dto: d, model: m })
    }
}
