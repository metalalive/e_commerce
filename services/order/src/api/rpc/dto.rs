use std::vec::Vec;

use chrono::offset::FixedOffset;
use chrono::DateTime;
use serde::{Deserialize, Serialize};

use ecommerce_common::api::dto::CurrencyDto;

use crate::api::dto::{ProdAttrValueDto, ShippingDto};

#[derive(Deserialize)]
pub struct ProductPriceDeleteDto {
    pub items: Option<Vec<u64>>,
}

#[derive(Deserialize)]
pub struct ProductAttrPriceDto {
    pub label_id: String,
    pub value: ProdAttrValueDto,
    pub price: i32, // extra amount to charge
}

#[derive(Deserialize)]
pub struct ProdAttrPriceSetDto {
    pub extra_charge: Vec<ProductAttrPriceDto>,
    pub last_update: DateTime<FixedOffset>,
}

#[derive(Deserialize)]
pub struct ProductPriceEditDto {
    pub price: u32, // should be base price
    pub start_after: DateTime<FixedOffset>,
    pub end_before: DateTime<FixedOffset>,
    pub product_id: u64, // TODO, declare type alias
    pub attributes: ProdAttrPriceSetDto,
}

#[derive(Deserialize)]
pub struct ProductPriceDto {
    pub s_id: u32, // store ID
    pub rm_all: bool,
    pub currency: Option<CurrencyDto>,
    pub deleting: ProductPriceDeleteDto,
    pub updating: Vec<ProductPriceEditDto>,
    pub creating: Vec<ProductPriceEditDto>,
}

#[derive(Deserialize)]
pub struct InventoryEditStockLevelDto {
    // number to add to stock level, negative number means cancellation
    // from either order-line model or inventory application
    // TODO, redesign the quantity field, double-meaning field doesn't look like good practice
    pub qty_add: i32,
    pub store_id: u32,
    pub product_id: u64, // TODO, declare type alias
    pub expiry: DateTime<FixedOffset>,
}

#[derive(Serialize)]
pub struct StockQuantityPresentDto {
    pub total: u32,
    pub booked: u32,
    pub cancelled: u32,
}

#[derive(Serialize)]
pub struct StockLevelPresentDto {
    pub quantity: StockQuantityPresentDto,
    pub store_id: u32,
    pub product_id: u64, // TODO, declare type alias
    pub expiry: DateTime<FixedOffset>,
}

#[derive(Deserialize)]
pub struct StockLevelReturnDto {
    pub order_id: String,
    pub items: Vec<InventoryEditStockLevelDto>,
}

#[derive(Deserialize)]
pub struct OrderReplicaInventoryReqDto {
    pub start: DateTime<FixedOffset>,
    pub end: DateTime<FixedOffset>,
}
#[derive(Serialize)]
pub struct OrderLineStockReservingDto {
    pub seller_id: u32,
    pub product_id: u64,
    pub qty: u32,
}
#[derive(Serialize)]
pub struct OrderLineStockReturningDto {
    pub seller_id: u32,
    pub product_id: u64,
    pub create_time: DateTime<FixedOffset>,
    pub qty: u32,
}
#[derive(Serialize)]
pub struct OrderReplicaStockReservingDto {
    pub oid: String,
    pub usr_id: u32,
    pub create_time: DateTime<FixedOffset>,
    pub lines: Vec<OrderLineStockReservingDto>,
    pub shipping: ShippingDto,
}
#[derive(Serialize)]
pub struct OrderReplicaStockReturningDto {
    pub oid: String,
    pub usr_id: u32,
    pub lines: Vec<OrderLineStockReturningDto>,
} // TODO, add shipping addresses for different returns

#[derive(Serialize)]
pub struct OrderReplicaInventoryDto {
    pub reservations: Vec<OrderReplicaStockReservingDto>,
    pub returns: Vec<OrderReplicaStockReturningDto>,
}

#[derive(Serialize, Debug)]
pub enum StockReturnErrorReason {
    NotExist,
    InvalidQuantity,
}

#[derive(Serialize, Debug)]
pub struct StockReturnErrorDto {
    pub seller_id: u32,
    pub product_id: u64,
    pub reason: StockReturnErrorReason,
}
