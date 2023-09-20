use std::vec::Vec;

use crate::api::rpc::dto::ProductPriceEditDto;

pub struct ProductPriceModel {
}

pub struct ProductPriceModelSet {
    pub items:Vec<ProductPriceModel>
}

impl ProductPriceModelSet {
    pub fn update(self, updating:Vec<ProductPriceEditDto>,
                  creating:Vec<ProductPriceEditDto>) -> Self
    {
        self
    }
}
