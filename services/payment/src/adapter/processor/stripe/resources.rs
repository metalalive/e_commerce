use serde::{Deserialize, Serialize};

use ecommerce_common::api::dto::CurrencyDto;
use crate::api::web::dto::StripeCheckoutUImodeDto;
use crate::model::ChargeLineBuyerModel;

#[derive(Deserialize)]
pub(super) struct CheckoutSession {
    pub id: String,
    pub client_secret: Option<String>,
    pub url: Option<String>,
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
    // `application_fee_amount` only supported in direct charge and payment charge,
    // in this application I use separate charge, the application fee will be charged
    // by reducing amount of payout to relevant  sellers
    pub transfer_group: Option<String>, // for seperate charges
}

#[derive(Serialize)]
pub(super) struct CreateCheckoutSessionProductData {
    pub name: String,
}

#[derive(Serialize)]
pub(super) struct CreateCheckoutSessionPriceData {
    pub product_data: CreateCheckoutSessionProductData,
    pub currency: CurrencyDto,
    pub unit_amount: u32,
}

#[derive(Serialize)]
pub(super) struct CreateCheckoutSessionLineItem {
    pub price_data: CreateCheckoutSessionPriceData,
    pub quantity: u32,
}

#[derive(Serialize)]
pub(super) struct CreateCheckoutSession {
    pub client_reference_id: String, // usr-profile-id followed by order-id
    pub currency: CurrencyDto,
    pub customer: Option<String>, // customer-id only, expandable object not supported
    pub expires_at: i64,          // epoch time in seconds at which the checkout will expire
    pub cancel_url: Option<String>,
    pub success_url: Option<String>,
    pub return_url: Option<String>, // for return / refund, TODO, verify
    // TODO, implement Price / Product objects, it is useless for this e-commerce
    // project but essential for Stripe platform
    pub line_items: Vec<CreateCheckoutSessionLineItem>,
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
impl From<&ChargeLineBuyerModel> for CreateCheckoutSessionLineItem {
    fn from(value: &ChargeLineBuyerModel) -> Self {
        Self {
            price_data: CreateCheckoutSessionPriceData {
                product_data: CreateCheckoutSessionProductData {
                    name: format!("{:?}", value.pid),
                }, // TODO, load product name, save the product ID in metadata
                currency: CurrencyDto::TWD, // TODO, should be a field from `value.amount`
                unit_amount: value.amount.unit * 100, // TODO, add field for smallest unit of specific
                                                      // currency
            },
            quantity: value.amount.qty,
        }
    }
}
