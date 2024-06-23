use serde::{Deserialize, Serialize};

use crate::api::web::dto::{PaymentCurrencyDto, StripeCheckoutUImodeDto};

#[derive(Deserialize)]
pub(super) struct CheckoutSession {
    pub id: String,
    pub client_secret: String,
    // TODO, finish implementation
}

#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum CheckoutSessionMode {
    Payment,
    // currently not support other options : Setup, Subscription,
}

#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum CheckoutSessionUiMode {
    Embedded,
    Hosted,
}

#[derive(Serialize)]
pub(super) struct CreateCheckoutSessionPaymentIntentData {
    pub application_fee_amount: i64,
    pub transfer_group: Option<String>, // for seperate charges
}

#[derive(Serialize)]
pub(super) struct CreateCheckoutSession {
    pub client_reference_id: String, // usr-profile-id followed by order-id
    pub currency: PaymentCurrencyDto,
    pub customer: Option<String>, // customer-id only, expandable object not supported
    pub expires_at: i64,          // epoch time in seconds at which the checkout will expire
    pub cancel_url: Option<String>,
    pub success_url: Option<String>,
    pub return_url: Option<String>, // for return / refund, TODO, verify
    pub livemode: bool,             // false means test mode
    // TODO, implement Price / Product objects, it is useless for this e-commerce
    // project but essential for Stripe platform
    pub line_items: Vec<u32>,
    pub payment_intent_data: CreateCheckoutSessionPaymentIntentData,
    pub mode: CheckoutSessionMode,
    pub ui_mode: CheckoutSessionUiMode,
}

impl From<&StripeCheckoutUImodeDto> for CheckoutSessionUiMode {
    fn from(value: &StripeCheckoutUImodeDto) -> Self {
        match value {
            StripeCheckoutUImodeDto::EmbeddedJs => Self::Embedded,
            StripeCheckoutUImodeDto::RedirectPage => Self::Hosted,
        }
    }
}
