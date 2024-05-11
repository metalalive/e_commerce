use chrono::{DateTime, FixedOffset};
use serde::{Deserialize, Serialize};

use ecommerce_common::api::dto::{
    jsn_serialize_product_type, jsn_validate_product_type, GenericRangeErrorDto, PayAmountDto,
};
use ecommerce_common::constant::ProductType;

#[derive(Deserialize)]
pub enum StripeCheckoutUImodeDto {
    RedirectPage,
    EmbeddedJs,
}
#[derive(Deserialize)]
pub struct StripeCheckoutSessionReqDto {
    pub customer_id: String,
    pub ui_mode: StripeCheckoutUImodeDto,
}

#[derive(Deserialize)]
#[serde(tag = "label")]
pub enum PaymentMethodReqDto {
    Stripe(StripeCheckoutSessionReqDto),
}
#[allow(clippy::upper_case_acronyms)]
#[derive(Deserialize)]
pub enum PaymentCurrencyDto {
    TWD,
    USD,
    THB,
}
#[derive(Deserialize)]
pub struct ChargeAmountOlineDto {
    pub seller_id: u32,
    pub product_id: u64,
    #[serde(deserialize_with = "jsn_validate_product_type")]
    pub product_type: ProductType,
    pub quantity: u32,
    pub amount: PayAmountDto,
    // TODO, tax and discount
}
#[derive(Deserialize)]
pub struct ChargeReqDto {
    pub order_id: String,
    pub method: PaymentMethodReqDto,
    pub lines: Vec<ChargeAmountOlineDto>,
    // TODO, tax and discount
    pub currency: PaymentCurrencyDto,
}

#[derive(Serialize)]
pub struct StripeCheckoutSessionRespDto {
    pub redirect_url: Option<String>, // redirect to Stripe-hosted payment page
    pub client_session: Option<String>, // for Stripe.js embedded checkout
}
#[derive(Serialize)]
#[serde(tag = "label")]
pub enum PaymentMethodRespDto {
    Stripe(StripeCheckoutSessionRespDto),
    // TODO, integrate with Wise (TransferWise) wallet
}
#[derive(Serialize)]
pub struct ChargeRespDto {
    pub method: PaymentMethodRespDto,
    pub create_time: DateTime<FixedOffset>,
}

#[derive(Serialize)]
pub enum OrderErrorReason {
    InvalidOrder,
}
#[derive(Serialize)]
pub enum PaymentMethodErrorReason {
    InvalidUser,
    OperationRefuse,
}
#[derive(Serialize)]
pub struct ChargeOlineErrorDto {
    pub seller_id: u32,
    pub product_id: u64,
    #[serde(serialize_with = "jsn_serialize_product_type")]
    pub product_type: ProductType,
    pub quantity: Option<GenericRangeErrorDto>,
    // to indicate mismatch,  this backend app returns the estimated amount
    pub amount: Option<PayAmountDto>,
    pub expired: Option<bool>,
}

#[derive(Serialize)]
pub struct ChargeRespErrorDto {
    pub order_id: Option<OrderErrorReason>,
    pub method: Option<PaymentMethodErrorReason>,
    pub lines: Option<Vec<ChargeOlineErrorDto>>,
}

#[derive(Serialize)]
pub enum ChargeStatusDto {
    // --- retryable ---
    // session initialized, customer hasn't approved the final payment yet
    SessionInitialized,
    // payment refused at 3rd-party provider (PSP) for some reasons
    // e.g. 3D secure validation failure
    PspRefused,
    // is it possible the PSP is still processing on refreshing
    // this charge status
    PspProcessing,
    // network error to PSP
    PspTimedOut,
    // --- non-retryable ---
    // charge session has expired, the expiry time specified by 3rd-party PSP
    SessionExpired,
    // happened when customer decided to cancel the charge at PSP host payment page
    Cancelled,
    // customer completed the charge, now issuing bank may authorise the transfer
    // then merchant can capture the fund
    Completed,
}

#[derive(Serialize)]
pub struct ChargeRefreshRespDto {
    pub status: ChargeStatusDto,
}
