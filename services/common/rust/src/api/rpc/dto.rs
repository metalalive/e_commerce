use crate::api::dto::{
    jsn_serialize_product_type, jsn_validate_product_type, BillingDto, CountryCode,
    OrderCurrencySnapshotDto, OrderLinePayDto,
};
use crate::constant::ProductType;
use serde::{Deserialize, Serialize};
use super::super::dto::PayAmountDto;

#[derive(Deserialize)]
pub struct StoreEmailRepDto {
    pub addr: String,
}

#[derive(Deserialize)]
pub struct StorePhoneRepDto {
    pub country_code: String,
    pub line_number: String,
}

#[derive(Deserialize)]
pub struct ShopLocationRepDto {
    pub country: CountryCode,
    pub locality: String,
    pub street: String,
    pub detail: String,
    pub floor: i16,
}

#[derive(Deserialize)]
pub struct StoreStaffRepDto {
    pub staff_id: u32,
    pub start_after: String, // RFC 3339 stringified
    pub end_before: String,  // RFC 3339 stringified
}

#[derive(Deserialize)]
pub struct StoreProfileReplicaDto {
    pub label: String,
    pub active: bool,
    pub supervisor_id: u32,
    pub emails: Option<Vec<StoreEmailRepDto>>,
    pub phones: Option<Vec<StorePhoneRepDto>>,
    pub location: Option<ShopLocationRepDto>,
    pub staff: Option<Vec<StoreStaffRepDto>>,
}

#[derive(Serialize)]
pub struct StoreProfileReplicaReqDto {
    pub store_id: u32,
}

#[derive(Serialize, Deserialize)]
pub struct OrderReplicaPaymentReqDto {
    pub order_id: String,
}

#[derive(Serialize, Deserialize)]
pub struct OrderReplicaPaymentDto {
    pub oid: String,
    pub usr_id: u32,
    pub lines: Vec<OrderLinePayDto>,
    pub currency: OrderCurrencySnapshotDto,
    pub billing: BillingDto,
}

#[derive(Serialize, Deserialize)]
pub struct OrderLinePaidUpdateDto {
    pub seller_id: u32,
    pub product_id: u64,
    #[serde(
        serialize_with = "jsn_serialize_product_type",
        deserialize_with = "jsn_validate_product_type"
    )]
    pub product_type: ProductType,
    pub qty: u32,
}

#[derive(Serialize, Deserialize)]
pub struct OrderPaymentUpdateDto {
    pub oid: String,
    // stringified date time with UTC time zone
    pub charge_time: String,
    pub lines: Vec<OrderLinePaidUpdateDto>,
}

#[derive(Serialize, Deserialize, Debug)]
pub enum OrderLinePayUpdateErrorReason {
    NotExist,
    InvalidQuantity,
    Omitted,
}
#[derive(Serialize, Deserialize, Debug)]
pub struct OrderLinePayUpdateErrorDto {
    pub seller_id: u32,
    pub product_id: u64,
    #[serde(
        serialize_with = "jsn_serialize_product_type",
        deserialize_with = "jsn_validate_product_type"
    )]
    pub product_type: ProductType,
    pub reason: OrderLinePayUpdateErrorReason,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct OrderPaymentUpdateErrorDto {
    pub oid: String,
    pub charge_time: Option<String>,
    pub lines: Vec<OrderLinePayUpdateErrorDto>,
}

#[derive(Deserialize)]
pub struct OrderReplicaRefundReqDto {
    pub order_id: String,
    // the fields `start` and `end` should be serial RFC3339 date-time format
    pub start: String,
    pub end: String,
}
#[derive(Serialize)]
pub struct OrderLineReplicaRefundDto {
    pub seller_id: u32,
    pub product_id: u64,
    #[serde(
        deserialize_with = "jsn_validate_product_type",
        serialize_with = "jsn_serialize_product_type"
    )]
    pub product_type: ProductType,
    // the field `create-time` should be serial RFC3339 date-time format
    pub create_time: String,
    pub amount: PayAmountDto,
}
