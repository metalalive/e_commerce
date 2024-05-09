use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize)]
pub struct PayAmountDto {
    pub unit: u32,
    pub total: u32,
}

#[derive(Deserialize, Serialize)]
pub struct GenericRangeErrorDto {
    pub max_: u16,
    pub min_: u16,
    pub given: u32,
}
