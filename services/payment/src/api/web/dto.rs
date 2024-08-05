use chrono::{DateTime, FixedOffset, Utc};
use serde::{Deserialize, Serialize};

use ecommerce_common::api::dto::{
    jsn_serialize_product_type, jsn_validate_product_type, CurrencyDto, GenericRangeErrorDto,
    PayAmountDto,
};
use ecommerce_common::constant::ProductType;

#[derive(Deserialize)]
pub enum StripeCheckoutUImodeDto {
    RedirectPage,
    EmbeddedJs,
}
#[derive(Deserialize)]
pub struct StripeCheckoutSessionReqDto {
    pub customer_id: Option<String>,
    pub cancel_url: Option<String>,
    pub success_url: Option<String>, // only for redirect-page UI-mode
    pub return_url: Option<String>,  // for return / refund, TODO, verify
    pub ui_mode: StripeCheckoutUImodeDto,
}

#[derive(Deserialize)]
#[serde(tag = "label")]
pub enum PaymentMethodReqDto {
    Stripe(StripeCheckoutSessionReqDto),
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
pub struct ChargeReqOrderDto {
    pub id: String,
    pub lines: Vec<ChargeAmountOlineDto>,
    // currency and exchange rate should be determined on creating
    // a new order, not on charging
    pub currency: CurrencyDto,
    // TODO,
    // - tax and discount
}
#[derive(Deserialize)]
pub struct ChargeReqDto {
    pub order: ChargeReqOrderDto,
    pub method: PaymentMethodReqDto,
}
impl ChargeReqDto {
    pub(crate) fn into_parts(self) -> (ChargeReqOrderDto, PaymentMethodReqDto) {
        let Self { order, method } = self;
        (order, method)
    }
}

#[derive(Serialize)]
pub struct StripeCheckoutSessionRespDto {
    pub id: String,
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
pub struct ChargeCreateRespDto {
    pub id: String,
    pub method: PaymentMethodRespDto,
    pub create_time: DateTime<FixedOffset>,
}

#[derive(Serialize)]
pub enum OrderErrorReason {
    InvalidOrder,
}
#[derive(Serialize, Debug)]
pub enum PaymentMethodErrorReason {
    InvalidUser,
    OperationRefuse,
    ProcessorFailure,
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
    pub not_exist: bool,
}

#[derive(Serialize, Default)]
pub struct ChargeRespErrorDto {
    pub order_id: Option<OrderErrorReason>,
    pub method: Option<PaymentMethodErrorReason>,
    pub lines: Option<Vec<ChargeOlineErrorDto>>,
    pub currency: Option<CurrencyDto>, // the currency chosen by buyer on creating the order
}

#[derive(Serialize)]
pub enum ChargeStatusDto {
    // --- retryable ---
    Initialized,
    // is it possible the PSP is still processing on refreshing
    // this charge status
    PspProcessing,
    // network error to PSP
    PspTimedOut,
    // happened after 3rd party processor is done before remote order
    // app is synced with the charge update
    InternalSyncing,
    // --- non-retryable ---
    // when front-end clients receive following status, they don't need to refresh
    // charge status again because backend will not change anything
    // -----
    // payment refused at 3rd-party provider (PSP) for some reasons
    // e.g. 3D secure validation failure
    PspRefused,
    // charge session has expired, the expiry time specified by 3rd-party PSP
    SessionExpired,
    // customer completed the charge, now issuing bank may authorise the transfer
    // then merchant can capture the fund
    Completed,
    UnknownPsp,
}

#[derive(Serialize)]
pub struct ChargeRefreshRespDto {
    pub status: ChargeStatusDto,
    pub order_id: String,
    pub create_time: DateTime<Utc>,
}
