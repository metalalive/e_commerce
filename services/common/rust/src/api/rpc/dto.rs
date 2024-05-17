use serde::{Serialize, Deserialize};
use crate::api::dto::{OrderLinePayDto, BillingDto};

#[derive(Serialize, Deserialize)]
pub struct OrderReplicaPaymentReqDto {
    pub order_id: String,
}

#[derive(Serialize, Deserialize)]
pub struct OrderReplicaPaymentDto {
    pub oid: String,
    pub usr_id: u32,
    pub lines: Vec<OrderLinePayDto>,
    // TODO, add currency field
    pub billing: BillingDto,
}
