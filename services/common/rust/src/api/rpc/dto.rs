use crate::api::dto::{
    jsn_serialize_product_type, jsn_validate_product_type, BillingDto, OrderCurrencySnapshotDto,
    OrderLinePayDto,
};
use crate::constant::ProductType;
use serde::{Deserialize, Serialize};

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

#[derive(Deserialize)]
pub struct OrderLinePaidUpdateDto {
    pub seller_id: u32,
    pub product_id: u64,
    #[serde(deserialize_with = "jsn_validate_product_type")]
    pub product_type: ProductType,
    pub qty: u32,
}

#[derive(Deserialize)]
pub struct OrderPaymentUpdateDto {
    pub oid: String,
    // stringified date time with UTC time zone
    pub charge_time: String,
    pub lines: Vec<OrderLinePaidUpdateDto>,
}

#[derive(Serialize)]
pub enum OrderLinePayUpdateErrorReason {
    NotExist,
    InvalidQuantity,
    Omitted,
}
#[derive(Serialize)]
pub struct OrderLinePayUpdateErrorDto {
    pub seller_id: u32,
    pub product_id: u64,
    #[serde(serialize_with = "jsn_serialize_product_type")]
    pub product_type: ProductType,
    pub reason: OrderLinePayUpdateErrorReason,
}

#[derive(Serialize)]
pub struct OrderPaymentUpdateErrorDto {
    pub oid: String,
    pub charge_time: Option<String>,
    pub lines: Vec<OrderLinePayUpdateErrorDto>,
}
