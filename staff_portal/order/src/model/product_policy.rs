use std::cmp::PartialEq;
use std::vec::Vec;
use std::result::Result as DefaultResult;

use crate::api::web::dto::{ProductPolicyDto, ProductPolicyClientErrorDto, ProductPolicyClientLimitDto};
use crate::constant::ProductType;
use crate::error::{AppError, AppErrorCode};

#[derive(Debug)]
pub struct ProductPolicyModel {
    pub product_type: ProductType,
    pub product_id: u64,
    pub auto_cancel_secs: u32,
    pub warranty_hours: u32,
    pub is_create: bool,
}

impl PartialEq for ProductPolicyModel {
    fn eq(&self, other: &Self) -> bool {
        let p_typ_self: u8 = self.product_type.clone().into();
        let p_typ_other: u8 = other.product_type.clone().into();
            (self.product_id == other.product_id) && (p_typ_self == p_typ_other) &&
            (self.auto_cancel_secs == other.auto_cancel_secs) &&
            (self.warranty_hours == other.warranty_hours)
    }
    fn ne(&self, other: &Self) -> bool {
        !self.eq(other)
    }
}


const HARD_LIMIT_AUTO_CANCEL_SECS: u32 = 3600 * 24; // one day
const HARD_LIMIT_WARRANTY_HOURS: u32 = 365 * 24 * 20; // 20 years

pub struct ProductPolicyModelSet {
    pub policies : Vec<ProductPolicyModel>
}

impl ProductPolicyModelSet
{
    pub fn validate (newdata:&Vec<ProductPolicyDto>) -> DefaultResult<(), Vec<ProductPolicyClientErrorDto>>
    {
        if newdata.is_empty() {
            let ce = ProductPolicyClientErrorDto {product_type:ProductType::Unknown(0),
                product_id: 0u64, auto_cancel_secs: None, warranty_hours:None,
                err_type: format!("{:?}", AppErrorCode::EmptyInputData) };
            return Err(vec![ce]);
        }
        let detected_invalid = newdata.iter().filter_map(|item| {
            let cond = (item.auto_cancel_secs > HARD_LIMIT_AUTO_CANCEL_SECS) ||
                (item.warranty_hours > HARD_LIMIT_WARRANTY_HOURS);
            if cond {
                let auto_cancel_secs = Some(ProductPolicyClientLimitDto {
                    given:item.auto_cancel_secs, limit:HARD_LIMIT_AUTO_CANCEL_SECS});
                let warranty_hours = Some(ProductPolicyClientLimitDto {
                    given:item.warranty_hours, limit:HARD_LIMIT_WARRANTY_HOURS});
                let ce = ProductPolicyClientErrorDto {
                    product_id: item.product_id, product_type: item.product_type.clone(),
                    auto_cancel_secs, warranty_hours, 
                    err_type: format!("{:?}", AppErrorCode::ExceedingMaxLimit),
                };
                Some(ce)
            } else { None }
        }).collect::<Vec<ProductPolicyClientErrorDto>>();
        if detected_invalid.is_empty() {
            Ok(())
        } else {
            Err(detected_invalid)
        }
    } // end of fn validate

    pub fn update(mut self, newdata:&Vec<ProductPolicyDto>)
        -> DefaultResult<Self, AppError>
    {
        let mut _new_objs = newdata.iter().filter_map(|item| {
            let result = self.policies.iter_mut().find(|o| {
                o.product_id == item.product_id && o.product_type == item.product_type
            });
            if let Some(obj) = result {
                obj.auto_cancel_secs = item.auto_cancel_secs;
                obj.warranty_hours = item.warranty_hours;
                None
            } else {
                Some(ProductPolicyModel {
                    is_create: true, product_id: item.product_id,
                    product_type: item.product_type.clone(),
                    auto_cancel_secs: item.auto_cancel_secs,
                    warranty_hours: item.warranty_hours,
                })
            }
        }).collect();
        self.policies.append(&mut _new_objs);
        Ok(self)
    } // end of fn update
      // TODO, consider append-only approach, for the order lines which apply previous setup
} // end of impl ProductPolicyModelSet

