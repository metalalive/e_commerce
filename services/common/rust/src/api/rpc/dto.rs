use crate::api::dto::{BillingDto, OrderLinePayDto};
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
    // TODO, add the fields
    // - target currency
    // - the currency rate on creating the order
    pub billing: BillingDto,
}
