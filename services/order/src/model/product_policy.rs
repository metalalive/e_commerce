use std::cmp::PartialEq;
use std::result::Result as DefaultResult;
use std::vec::Vec;

use ecommerce_common::error::AppErrorCode;

use crate::api::web::dto::{
    ProductPolicyClientErrorDto, ProductPolicyClientLimitDto, ProductPolicyDto,
    ProductPolicyNumRsvLimitDto,
};
use crate::error::AppError;

#[derive(Debug)]
pub struct ProductPolicyModel {
    pub product_id: u64,
    pub auto_cancel_secs: u32,
    pub warranty_hours: u32,
    // indicate max/min number of items for each type of product
    // per order transaction.
    pub max_num_rsv: u16,
    pub min_num_rsv: u16,
    // TODO, add following field:
    // - `store_id`: u32, each store front could configure its own policy
    //    even with the same product.
    pub is_create: bool,
}

impl PartialEq for ProductPolicyModel {
    fn eq(&self, other: &Self) -> bool {
        (self.product_id == other.product_id)
            && (self.auto_cancel_secs == other.auto_cancel_secs)
            && (self.warranty_hours == other.warranty_hours)
            && (self.max_num_rsv == other.max_num_rsv)
            && (self.min_num_rsv == other.min_num_rsv)
    }
}

const HARD_LIMIT_AUTO_CANCEL_SECS: u32 = 3600 * 24; // one day
const HARD_LIMIT_WARRANTY_HOURS: u32 = 365 * 24 * 20; // 20 years

pub struct ProductPolicyModelSet {
    pub policies: Vec<ProductPolicyModel>,
}

impl ProductPolicyModelSet {
    pub fn validate(
        newdata: &Vec<ProductPolicyDto>,
    ) -> DefaultResult<(), Vec<ProductPolicyClientErrorDto>> {
        if newdata.is_empty() {
            let ce = ProductPolicyClientErrorDto {
                product_id: 0u64,
                auto_cancel_secs: None,
                warranty_hours: None,
                num_rsv: None,
                err_type: format!("{:?}", AppErrorCode::EmptyInputData),
            };
            return Err(vec![ce]);
        }
        let zero_num_rsv = 0u16;
        let detected_invalid = newdata
            .iter()
            .filter_map(|item| {
                let auto_cancel_secs = if item.auto_cancel_secs > HARD_LIMIT_AUTO_CANCEL_SECS {
                    Some(ProductPolicyClientLimitDto {
                        given: item.auto_cancel_secs,
                        limit: HARD_LIMIT_AUTO_CANCEL_SECS,
                    })
                } else {
                    None
                };
                let warranty_hours = if item.warranty_hours > HARD_LIMIT_WARRANTY_HOURS {
                    Some(ProductPolicyClientLimitDto {
                        given: item.warranty_hours,
                        limit: HARD_LIMIT_WARRANTY_HOURS,
                    })
                } else {
                    None
                };
                let max_num_rsv = item.max_num_rsv.as_ref().unwrap_or(&zero_num_rsv);
                let min_num_rsv = item.min_num_rsv.as_ref().unwrap_or(&zero_num_rsv);
                let num_rsv = if min_num_rsv > max_num_rsv {
                    Some(ProductPolicyNumRsvLimitDto {
                        min_items: *min_num_rsv,
                        max_items: *max_num_rsv,
                    })
                } else {
                    None
                };

                if num_rsv.is_some() || auto_cancel_secs.is_some() || warranty_hours.is_some() {
                    let ce = ProductPolicyClientErrorDto {
                        product_id: item.product_id,
                        auto_cancel_secs,
                        warranty_hours,
                        num_rsv,
                        err_type: format!("{:?}", AppErrorCode::ExceedingMaxLimit),
                    };
                    Some(ce)
                } else {
                    None
                }
            })
            .collect::<Vec<ProductPolicyClientErrorDto>>();
        if detected_invalid.is_empty() {
            Ok(())
        } else {
            Err(detected_invalid)
        }
    } // end of fn validate

    pub fn update(mut self, newdata: Vec<ProductPolicyDto>) -> DefaultResult<Self, AppError> {
        let zero_num_rsv = 0u16;
        let mut _new_objs = newdata
            .into_iter()
            .filter_map(|mut item| {
                let max_num_rsv = item.max_num_rsv.take().unwrap_or(zero_num_rsv);
                let min_num_rsv = item.min_num_rsv.take().unwrap_or(zero_num_rsv);
                let result = self
                    .policies
                    .iter_mut()
                    .find(|o| o.product_id == item.product_id);
                if let Some(obj) = result {
                    obj.auto_cancel_secs = item.auto_cancel_secs;
                    obj.warranty_hours = item.warranty_hours;
                    obj.max_num_rsv = max_num_rsv;
                    obj.min_num_rsv = min_num_rsv;
                    None
                } else {
                    Some(ProductPolicyModel {
                        is_create: true,
                        product_id: item.product_id,
                        max_num_rsv,
                        min_num_rsv,
                        auto_cancel_secs: item.auto_cancel_secs,
                        warranty_hours: item.warranty_hours,
                    })
                }
            })
            .collect();
        self.policies.append(&mut _new_objs);
        Ok(self)
    } // end of fn update
      // TODO, consider append-only approach, for the order lines which apply previous setup
} // end of impl ProductPolicyModelSet
