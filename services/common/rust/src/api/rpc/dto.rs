use crate::api::dto::{BillingDto, OrderCurrencySnapshotDto, OrderLinePayDto};
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
