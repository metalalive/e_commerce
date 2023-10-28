use serde::{Deserialize, Serialize};

use crate::constant::ProductType;
use crate::api::{jsn_validate_product_type, jsn_serialize_product_type};

// TODO, merge the 2 DTO modules in `/web` and `/rpc` package

#[derive(Deserialize, Serialize)]
pub struct PayAmountDto {
    pub unit: u32,
    pub total: u32
}

#[derive(Deserialize, Serialize)]
pub struct OrderLinePayDto {
    pub seller_id: u32,
    pub product_id: u64,
    #[serde(deserialize_with="jsn_validate_product_type", serialize_with="jsn_serialize_product_type")]
    pub product_type: ProductType,
    pub reserved_until: String, // date-time formatted in RFC3339 spec
    pub quantity: u32,
    pub amount: PayAmountDto
}
