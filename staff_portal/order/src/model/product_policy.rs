use std::cmp::PartialEq;
use std::vec::Vec;

use crate::api::web::dto::ProductPolicyDto;

#[derive(Debug)]
pub struct ProductPolicyModel {
    pub usr_id : u32,
    pub product_id: u64,
    pub auto_cancel_secs: u32,
    pub warranty_hours: u32,
    pub async_stock_chk: bool,
    pub is_create: bool,
}

impl PartialEq for ProductPolicyModel {
    fn eq(&self, other: &Self) -> bool {
        (self.usr_id == other.usr_id) &&
        (self.product_id == other.product_id) &&
        (self.auto_cancel_secs == other.auto_cancel_secs) &&
        (self.warranty_hours == other.warranty_hours) &&
        (self.async_stock_chk == other.async_stock_chk)
    }
    fn ne(&self, other: &Self) -> bool {
        !self.eq(other)
    }
}

pub struct ProductPolicyModelSet {
    pub policies : Vec<ProductPolicyModel>
}

impl ProductPolicyModelSet
{
    pub fn update(mut self, newdata:&Vec<ProductPolicyDto>) -> Self
    {
        self
    }
}
