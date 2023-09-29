use std::vec::Vec;
use std::result::Result as DefaultResult;

use chrono::DateTime;
use chrono::offset::FixedOffset;

use crate::api::rpc::dto::{InventoryEditStockLevelDto, StockLevelPresentDto};
use crate::error::AppError;

pub struct ProductStockIdentity {
    pub store_id: u32,
    pub product_type: u8,
    pub product_id: u64, // TODO, declare type alias
    pub expiry: DateTime<FixedOffset>,
}
#[derive(Debug)]
pub struct StockQuantityModel {
    pub total: u32,
    pub booked: u32,
    pub cancelled: u32,
}
#[derive(Debug)]
pub struct ProductStockModel {
    pub type_: u8,
    pub id_: u64, // TODO, declare type alias
    pub expiry: DateTime<FixedOffset>,
    pub quantity: StockQuantityModel,
    pub is_create: bool,
}
pub struct StoreStockModel {
    pub store_id: u32,
    pub products: Vec<ProductStockModel>
}
pub struct StockLevelModelSet {
    pub stores: Vec<StoreStockModel>
}

impl Clone for ProductStockIdentity {
    fn clone(&self) -> Self {
        Self { store_id: self.store_id, product_type: self.product_type,
            product_id: self.product_id, expiry: self.expiry.clone() }
    }
}
impl Clone for StockQuantityModel {
    fn clone(&self) -> Self {
        Self {total:self.total, booked:self.booked, cancelled:self.cancelled}
    }
}
impl Clone for ProductStockModel {
    fn clone(&self) -> Self {
        Self { type_: self.type_, id_: self.id_, expiry: self.expiry.clone(),
            quantity: self.quantity.clone(), is_create: self.is_create }
    }
}
impl Clone for StoreStockModel {
    fn clone(&self) -> Self {
        Self { store_id: self.store_id, products: self.products.clone() }
    }
}
impl Clone for StockLevelModelSet {
    fn clone(&self) -> Self {
        Self {stores:self.stores.clone()}
    }
}

impl PartialEq for StockQuantityModel {
    fn eq(&self, other: &Self) -> bool {
        self.total == other.total && self.booked == other.booked
            && self.cancelled == other.cancelled
    }
    fn ne(&self, other: &Self) -> bool {
        !self.eq(other)
    }
}
impl PartialEq for ProductStockModel {
    fn eq(&self, other: &Self) -> bool {
        self.type_ == other.type_ && self.id_ == other.id_
            && self.quantity == other.quantity
            && self.expiry_without_millis() == other.expiry_without_millis()
    }
    fn ne(&self, other: &Self) -> bool {
        !self.eq(other)
    }
}

impl ProductStockModel {
    pub fn expiry_without_millis(&self) -> DateTime<FixedOffset>
    { // ignore more-previse-but-impractical detail less than one second.
        let orig_tz = self.expiry.timezone();
        let ts_secs = self.expiry.timestamp(); // erase milliseconds
        let _dt = DateTime::from_timestamp(ts_secs, 0).unwrap();
        let out = _dt.with_timezone(&orig_tz);
        //println!("time1:{}, time2: {}", self.expiry.to_rfc3339(), out.to_rfc3339());
        out
    }
}

impl StockLevelModelSet {
    pub fn update(mut self, data:Vec<InventoryEditStockLevelDto>)
        -> DefaultResult<Self, AppError>
    { Ok(self) }
    
    pub fn present(&self) -> Vec<StockLevelPresentDto>
    { Vec::new() }
}

