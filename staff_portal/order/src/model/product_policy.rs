use std::cmp::PartialEq;
use std::vec::Vec;
use std::result::Result as DefaultResult;

use crate::api::web::dto::ProductPolicyDto;
use crate::error::{AppError, AppErrorCode};

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


const HARD_LIMIT_AUTO_CANCEL_SECS: u32 = 3600 * 24; // one day
const HARD_LIMIT_WARRANTY_HOURS: u32 = 365 * 24 * 20; // 20 years

pub struct ProductPolicyModelSet {
    pub policies : Vec<ProductPolicyModel>
}

impl ProductPolicyModelSet
{
    pub fn validate (newdata:&Vec<ProductPolicyDto>) -> DefaultResult<(), AppError>
    {
        if newdata.is_empty() {
            return Err(AppError { code:AppErrorCode::EmptyInputData,
                detail: Some(format!("ProductPolicyModelSet::update"))
            });
        }
        let detected_invalid = newdata.iter().find_map(|item| {
            let cond = (item.auto_cancel_secs > HARD_LIMIT_AUTO_CANCEL_SECS) ||
                (item.warranty_hours > HARD_LIMIT_WARRANTY_HOURS);
            if cond {
                let errmsg = format!(
                    r#"
                      {{ "product_id":{}, "err_type":"{:?}",
                         "warranty_hours":{{"given":{}, "limit":{} }},
                         "auto_cancel_secs":{{"given":{}, "limit":{} }}
                      }}
                    "# , item.product_id, AppErrorCode::ExceedingMaxLimit,
                    item.warranty_hours, HARD_LIMIT_WARRANTY_HOURS,
                    item.auto_cancel_secs, HARD_LIMIT_AUTO_CANCEL_SECS
                );
                Some(errmsg)
            } else { None }
        });
        if let Some(msg) = detected_invalid {
            Err(AppError{ code:AppErrorCode::InvalidInput, detail:Some(msg) })
        } else {
            Ok(())
        }
    } // end of fn validate

    pub fn update(mut self,  usr_id: u32, newdata:&Vec<ProductPolicyDto>)
        -> DefaultResult<Self, AppError>
    {
        Self::validate(newdata)?;
        self.check_user_consistency(usr_id)?;
        let mut _new_objs = newdata.iter().filter_map(|item| {
            let result = self.policies.iter_mut().find(|o| {o.product_id == item.product_id});
            if let Some(obj) = result {
                obj.auto_cancel_secs = item.auto_cancel_secs;
                obj.warranty_hours = item.warranty_hours;
                obj.async_stock_chk = item.async_stock_chk;
                None
            } else {
                Some(ProductPolicyModel {
                    is_create: true, usr_id, product_id: item.product_id,
                    auto_cancel_secs: item.auto_cancel_secs,
                    warranty_hours: item.warranty_hours,
                    async_stock_chk: item.async_stock_chk,
                })
            }
        }).collect();
        self.policies.append(&mut _new_objs);
        Ok(self)
    } // end of fn update

    fn check_user_consistency (&self, usr_id: u32) -> DefaultResult<(), AppError>
    {
        let detected_invalid = self.policies.iter().find_map(|obj| {
            if obj.usr_id == usr_id {
                None
            } else {
                let errmsg = format!(
                    r#"
                      {{ "product_id":{}, "model":"ProductPolicyModel",
                         "usr_id":{{"given":{}, "expect":{} }},
                      }}
                    "# , obj.product_id, obj.usr_id, usr_id
                );
                Some(errmsg)
            }
        });
        if let Some(msg) = detected_invalid {
            Err(AppError{code:AppErrorCode::DataCorruption, detail:Some(msg)})
        } else {
            Ok(())
        }
    }
} // end of impl ProductPolicyModelSet

