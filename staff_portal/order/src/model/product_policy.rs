use std::vec::Vec;

use crate::api::web::dto::ProductPolicyDto;

pub struct ProductPolicyModel {
    pub product_id: u64,
    pub auto_cancel_secs: u32,
    pub warranty_hours: u32,
    pub async_stock_chk: bool,
    is_create: bool,
}

pub struct ProductPolicyModelSet {
    pub usr_id : u32,
    pub policies : Vec<ProductPolicyModel>
}

impl ProductPolicyModelSet
{
    pub fn update(mut self, newdata:&Vec<ProductPolicyDto>) -> Self
    {
        self
    }
}
