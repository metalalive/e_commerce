use chrono::offset::FixedOffset;
use chrono::DateTime;
use std::cmp::{Eq, PartialEq};
use std::result::Result as DefaultResult;
use std::vec::Vec;

use ecommerce_common::api::dto::CurrencyDto;
use ecommerce_common::error::AppErrorCode;

use crate::api::rpc::dto::ProductPriceEditDto;
use crate::api::web::dto::OrderLineReqDto;
use crate::error::AppError;

pub type ProductPriceCreateArgs = (u64, u32, [DateTime<FixedOffset>; 2]);

#[derive(Debug, Eq)]
pub struct ProductPriceModel {
    price: u32, // TODO, rename to base-price
    start_after: DateTime<FixedOffset>,
    end_before: DateTime<FixedOffset>,
    product_id: u64,
    is_create: bool,
} // TODO, extra pricing from product attributes

impl PartialEq for ProductPriceModel {
    fn eq(&self, other: &Self) -> bool {
        (self.price == other.price)
            && (self.product_id == other.product_id)
            && (self.start_after == other.start_after)
            && (self.end_before == other.end_before)
    }
}

impl Clone for ProductPriceModel {
    fn clone(&self) -> Self {
        Self {
            price: self.price,
            product_id: self.product_id,
            start_after: self.start_after,
            end_before: self.end_before,
            is_create: self.is_create,
        }
    }
}

impl<'a> From<&'a ProductPriceEditDto> for ProductPriceModel {
    fn from(d: &'a ProductPriceEditDto) -> Self {
        Self {
            price: d.price,
            product_id: d.product_id,
            start_after: d.start_after,
            end_before: d.end_before,
            is_create: true,
        }
    }
}

impl From<ProductPriceCreateArgs> for ProductPriceModel {
    fn from(d: ProductPriceCreateArgs) -> Self {
        Self {
            product_id: d.0,
            price: d.1,
            start_after: d.2[0],
            end_before: d.2[1],
            is_create: false,
        }
    }
}

impl ProductPriceModel {
    #[rustfmt::skip]
    pub(crate) fn into_parts(self) -> ProductPriceCreateArgs {
        let Self {product_id, price, start_after, end_before, is_create: _} = self;
        (product_id, price, [start_after, end_before])
    }
    pub(crate) fn base_price(&self) -> u32 {
        // TODO, separate method for calculating price with extra attribute combination
        self.price
    }
    pub fn product_id(&self) -> u64 {
        self.product_id
    }
    pub(crate) fn start_after(&self) -> DateTime<FixedOffset> {
        self.start_after
    }
    pub(crate) fn end_before(&self) -> DateTime<FixedOffset> {
        self.end_before
    }
    pub(crate) fn split_by_update_state(ms: Vec<Self>) -> (Vec<Self>, Vec<Self>) {
        let (mut l_add, mut l_modify) = (vec![], vec![]);
        ms.into_iter()
            .map(|p| {
                if p.is_create {
                    l_add.push(p);
                } else {
                    l_modify.push(p)
                }
            })
            .count(); // TODO, swtich to feature `drain-filter` when it becomes stable
        (l_add, l_modify)
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
        let mut new_items = creating.iter().map(ProductPriceModel::from).collect();
        self.items.append(&mut new_items);
        self.currency = new_currency;
        Ok(self)
    } // end of fn update

    pub(crate) fn find_product(&self, d: &OrderLineReqDto) -> Option<&ProductPriceModel> {
        // TODO, validate expiry of the pricing rule
        if self.store_id == d.seller_id {
            self.items.iter().find(|m| m.product_id() == d.product_id)
        } else {
            None
        }
    }
} // end of impl ProductPriceModelSet
