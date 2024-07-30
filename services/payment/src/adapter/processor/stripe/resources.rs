use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use crate::api::web::dto::StripeCheckoutUImodeDto;
use crate::model::{
    ChargeLineBuyerModel, StripeCheckoutPaymentStatusModel, StripeSessionStatusModel,
};
use ecommerce_common::api::dto::CurrencyDto;

#[derive(Deserialize)]
pub(super) struct CheckoutSession {
    pub id: String,
    pub client_secret: Option<String>,
    pub url: Option<String>,
    pub status: StripeSessionStatusModel,
    pub payment_status: StripeCheckoutPaymentStatusModel,
    pub payment_intent: String,
    pub expires_at: i64,
    // TODO, record more fields for payout at later time
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
    pub unit_amount_decimal: String,
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

impl CreateCheckoutSessionPriceData {
    fn new(cline: &ChargeLineBuyerModel, currency_label: CurrencyDto) -> Self {
        let m = Self::subunit_multiplier(currency_label.clone());
        let m = Decimal::new(m, 0);
        // TODO, overflow error handling
        let amt_unit_represent = cline.amount.unit * m;
        CreateCheckoutSessionPriceData {
            product_data: CreateCheckoutSessionProductData {
                name: format!("{:?}", cline.pid),
            }, // TODO, load product name, save the product ID in metadata
            currency: currency_label,
            // the unit-amount field has to contain smallest unit
            // of specific currency
            unit_amount_decimal: amt_unit_represent.to_string(),
        }
    }
    fn subunit_multiplier(given: CurrencyDto) -> i64 {
        // [reference]
        // check `number to basic` column in the table listing currency
        // subunit (minor unit) below
        // https://en.wikipedia.org/wiki/List_of_circulating_currencies#T
        // https://en.wikipedia.org/wiki/New_Taiwan_dollar
        match given {
            CurrencyDto::INR
            | CurrencyDto::IDR
            | CurrencyDto::TWD
            | CurrencyDto::THB
            | CurrencyDto::USD => 100,
            CurrencyDto::Unknown => 1,
        }
    }
} // end of impl CreateCheckoutSessionPriceData

impl From<(CurrencyDto, &ChargeLineBuyerModel)> for CreateCheckoutSessionLineItem {
    fn from(value: (CurrencyDto, &ChargeLineBuyerModel)) -> Self {
        let (currency_label, cline) = value;
        let quantity = cline.amount.qty;
        let price_data = CreateCheckoutSessionPriceData::new(cline, currency_label);
        Self {
            price_data,
            quantity,
        }
    }
}
