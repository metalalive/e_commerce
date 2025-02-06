use chrono::offset::FixedOffset;
use chrono::DateTime;
use std::cmp::{Eq, PartialEq};
use std::result::Result as DefaultResult;
use std::vec::Vec;

use ecommerce_common::api::dto::CurrencyDto;
use ecommerce_common::error::AppErrorCode;

use crate::api::rpc::dto::ProductPriceEditDto;
use crate::error::AppError;

#[derive(Debug, Eq)]
pub struct ProductPriceModel {
    pub price: u32, // TODO, rename to base-price
    pub start_after: DateTime<FixedOffset>,
    pub end_before: DateTime<FixedOffset>,
    pub product_id: u64,
    pub is_create: bool,
} // TODO, extra pricing from product attributes

impl PartialEq for ProductPriceModel {
    fn eq(&self, other: &Self) -> bool {
        (self.price == other.price)
            && (self.product_id == other.product_id)
            && (self.start_after == other.start_after)
            && (self.end_before == other.end_before)
    }
}

pub struct ProductPriceModelSet {
    pub store_id: u32,
    pub currency: CurrencyDto,
    pub items: Vec<ProductPriceModel>,
}

impl ProductPriceModelSet {
    pub fn update(
        mut self,
        updating: Vec<ProductPriceEditDto>,
        creating: Vec<ProductPriceEditDto>,
        new_currency: CurrencyDto,
    ) -> DefaultResult<Self, AppError> {
        let num_updated = updating
            .iter()
            .filter_map(|d| {
                let result = self
                    .items
                    .iter_mut()
                    .find(|obj| obj.product_id == d.product_id && !obj.is_create);
                if let Some(obj) = result {
                    (obj.price, obj.end_before) = (d.price, d.end_before);
                    obj.start_after = d.start_after;
                    Some(1u8)
                } else {
                    None
                }
            })
            .count();
        if num_updated != updating.len() {
            return Err(AppError {
                code: AppErrorCode::InvalidInput,
                detail: Some("updating-data-to-nonexist-obj".to_string()),
            });
        }
        let mut new_items = creating
            .iter()
            .map(|d| ProductPriceModel {
                price: d.price,
                product_id: d.product_id,
                start_after: d.start_after,
                is_create: true,
                end_before: d.end_before,
            })
            .collect();
        self.items.append(&mut new_items);
        self.currency = new_currency;
        Ok(self)
    } // end of fn update
} // end of impl ProductPriceModelSet
