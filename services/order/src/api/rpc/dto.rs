use std::vec::Vec;

use serde::{Deserialize, Serialize};
use chrono::DateTime;
use chrono::offset::FixedOffset;

use crate::api::{jsn_validate_product_type, jsn_serialize_product_type };
use crate::api::dto::{OrderLinePayDto, BillingDto, ShippingDto, PayAmountDto};
use crate::constant::ProductType;

#[derive(Deserialize)]
pub struct ProductPriceDeleteDto {
    pub items:Option<Vec<u64>>,
    pub pkgs :Option<Vec<u64>>,
    #[serde(deserialize_with="jsn_validate_product_type")]
    pub item_type:ProductType,
    #[serde(deserialize_with="jsn_validate_product_type")]
    pub pkg_type:ProductType,
}

#[derive(Deserialize)]
pub struct ProductPriceEditDto {
    pub price: u32,
    pub start_after: DateTime<FixedOffset>,
    pub end_before: DateTime<FixedOffset>,
    // Note: This order-processing application doesn't need to know the meaning
    // of the field `product type` from this API endpoint, it is just for identifying
    // specific product in specific storefront. There is no need to convert the value
    // at here.
    #[serde(deserialize_with="jsn_validate_product_type")]
    pub product_type: ProductType,
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

#[derive(Deserialize)]
pub struct InventoryEditStockLevelDto {
    // number to add to stock level, negative number means cancellation
    // from either order-line model or inventory application
    // TODO, redesign the quantity field, double-meaning field doesn't look like good practice
    pub qty_add: i32,
    pub store_id: u32,
    #[serde(deserialize_with="jsn_validate_product_type")]
    pub product_type: ProductType,
    pub product_id: u64, // TODO, declare type alias
    pub expiry: DateTime<FixedOffset>
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
    #[serde(serialize_with="jsn_serialize_product_type")]
    pub product_type: ProductType,
    pub product_id: u64, // TODO, declare type alias
    pub expiry: DateTime<FixedOffset>
}

#[derive(Deserialize)]
pub struct StockLevelReturnDto {
    pub order_id: String,
    pub items: Vec<InventoryEditStockLevelDto>
}

#[derive(Deserialize)]
pub struct OrderReplicaPaymentReqDto {
    pub order_id: String
}
#[derive(Serialize)]
pub struct OrderReplicaPaymentDto {
    pub oid: String,
    pub usr_id: u32,
    pub lines: Vec<OrderLinePayDto>,
    pub billing: BillingDto,
}

#[derive(Deserialize)]
pub struct OrderReplicaRefundReqDto {
    pub order_id: String,
    pub start: DateTime<FixedOffset>,
    pub end:   DateTime<FixedOffset>,
}
#[derive(Serialize)]
pub struct OrderLineReplicaRefundDto {
    pub seller_id: u32,
    pub product_id: u64,
    #[serde(deserialize_with="jsn_validate_product_type", serialize_with="jsn_serialize_product_type")]
    pub product_type: ProductType,
    pub create_time: DateTime<FixedOffset>,
    pub amount: PayAmountDto
}


#[derive(Deserialize)]
pub struct OrderReplicaInventoryReqDto {
    pub start: DateTime<FixedOffset>,
    pub end:   DateTime<FixedOffset>,
}
#[derive(Serialize)]
pub struct OrderLineStockReservingDto {
    pub seller_id: u32,
    pub product_id: u64,
    #[serde(deserialize_with="jsn_validate_product_type", serialize_with="jsn_serialize_product_type")]
    pub product_type: ProductType,
    pub qty: u32 
}
#[derive(Serialize)]
pub struct OrderLineStockReturningDto {
    pub seller_id: u32,
    pub product_id: u64,
    #[serde(deserialize_with="jsn_validate_product_type", serialize_with="jsn_serialize_product_type")]
    pub product_type: ProductType,
    pub create_time: DateTime<FixedOffset>,
    pub qty: u32 
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
    pub returns:      Vec<OrderReplicaStockReturningDto>,
}


#[derive(Deserialize)]
pub struct OrderLinePaidUpdateDto {
    pub seller_id: u32,
    pub product_id: u64,
    #[serde(deserialize_with="jsn_validate_product_type")]
    pub product_type: ProductType,
    pub time: DateTime<FixedOffset>,
    pub qty: u32 
}

#[derive(Deserialize)]
pub struct OrderPaymentUpdateDto {
    pub oid: String,
    pub lines: Vec<OrderLinePaidUpdateDto>,
}

#[derive(Serialize)]
pub enum OrderLinePayUpdateErrorReason {
    NotExist, ReservationExpired, InvalidQuantity, Omitted
}

#[derive(Serialize)]
pub struct OrderLinePayUpdateErrorDto {
    pub seller_id: u32,
    pub product_id: u64,
    #[serde(serialize_with="jsn_serialize_product_type")]
    pub product_type: ProductType,
    pub reason: OrderLinePayUpdateErrorReason,
}

#[derive(Serialize)]
pub struct OrderPaymentUpdateErrorDto {
    pub oid: String,
    pub lines: Vec<OrderLinePayUpdateErrorDto>,
}

#[derive(Serialize, Debug)]
pub enum StockReturnErrorReason {
    NotExist, InvalidQuantity
}

#[derive(Serialize, Debug)]
pub struct StockReturnErrorDto {
    pub seller_id: u32,
    pub product_id: u64,
    #[serde(serialize_with="jsn_serialize_product_type")]
    pub product_type: ProductType,
    pub reason: StockReturnErrorReason,
}
