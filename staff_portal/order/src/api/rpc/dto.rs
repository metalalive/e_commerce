use std::vec::Vec;

use serde::Deserialize;
use chrono::DateTime;
use chrono::offset::Local;

#[derive(Deserialize)]
pub struct ProductPriceDeleteDto {
    pub items:Option<Vec<u64>>,
    pub pkgs :Option<Vec<u64>>,
    pub item_type:u8,
    pub pkg_type:u8,
}

#[derive(Deserialize)]
pub struct ProductPriceEditDto {
    pub price: u32,
    pub start_after: DateTime<Local>,
    pub end_before: DateTime<Local>,
    // Note: This order-processing application doesn't need to know the meaning
    // of the field `product type` from this API endpoint, it is just for identifying
    // specific product in specific storefront. There is no need to convert the value
    // at here.
    pub product_type: u8,
    pub product_id: u64, // TODO, declare type alias
}

#[derive(Deserialize)]
pub struct ProductPriceDto {
    pub s_id: u32, // store ID
    pub rm_all: bool,
    pub deleting: ProductPriceDeleteDto,
    pub updating: Vec<ProductPriceEditDto>,
    pub creating: Vec<ProductPriceEditDto>
}
