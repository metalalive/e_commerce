use std::cmp::min;
use std::result::Result as DefaultResult;
use std::vec::Vec;

use chrono::{DateTime, SubsecRound, Utc};

use ecommerce_common::error::AppErrorCode;

use crate::api::rpc::dto::{
    InventoryEditStockLevelDto, StockLevelPresentDto, StockLevelReturnDto, StockQuantityPresentDto,
    StockReturnErrorDto, StockReturnErrorReason,
};
use crate::api::web::dto::{
    OrderLineCreateErrNonExistDto, OrderLineCreateErrorDto, OrderLineCreateErrorReason,
};
use crate::error::AppError;

use super::{BaseProductIdentity, OrderLineModel, OrderLineModelSet};

pub struct ProductStockIdentity {
    pub store_id: u32,
    pub product_id: u64, // TODO, declare type alias
    pub expiry: DateTime<Utc>,
}
pub type ProductStockIdentity2 = BaseProductIdentity; // TODO, rename

#[derive(Debug)]
pub struct StockQtyRsvModel {
    pub oid: String, // order ID
    pub reserved: u32,
}
#[derive(Debug)]
pub struct StockQuantityModel {
    pub total: u32,
    pub cancelled: u32,
    pub booked: u32, // number of booked in all saved orders
    // quantities of specific order ID
    pub rsv_detail: Option<StockQtyRsvModel>,
}
#[derive(Debug)]
pub struct ProductStockModel {
    pub id_: u64, // TODO, declare type alias
    pub expiry: DateTime<Utc>,
    pub quantity: StockQuantityModel,
    pub is_create: bool,
}
pub struct StoreStockModel {
    pub store_id: u32,
    pub products: Vec<ProductStockModel>,
}
pub struct StockLevelModelSet {
    pub stores: Vec<StoreStockModel>,
}

impl From<StockQuantityModel> for StockQuantityPresentDto {
    fn from(value: StockQuantityModel) -> StockQuantityPresentDto {
        StockQuantityPresentDto {
            total: value.total,
            cancelled: value.cancelled,
            booked: value.booked,
        }
    }
}

impl Clone for ProductStockIdentity {
    fn clone(&self) -> Self {
        Self {
            store_id: self.store_id,
            product_id: self.product_id,
            expiry: self.expiry,
        }
    }
}
impl Clone for StockQtyRsvModel {
    fn clone(&self) -> Self {
        Self {
            oid: self.oid.clone(),
            reserved: self.reserved,
        }
    }
}
impl Clone for StockQuantityModel {
    fn clone(&self) -> Self {
        Self {
            total: self.total,
            cancelled: self.cancelled,
            booked: self.booked,
            rsv_detail: self.rsv_detail.clone(),
        }
    }
}
impl Clone for ProductStockModel {
    fn clone(&self) -> Self {
        Self {
            id_: self.id_,
            expiry: self.expiry,
            quantity: self.quantity.clone(),
            is_create: self.is_create,
        }
    }
}
impl Clone for StoreStockModel {
    fn clone(&self) -> Self {
        Self {
            store_id: self.store_id,
            products: self.products.clone(),
        }
    }
}
impl Clone for StockLevelModelSet {
    fn clone(&self) -> Self {
        Self {
            stores: self.stores.clone(),
        }
    }
}

impl PartialEq for StockQtyRsvModel {
    fn eq(&self, other: &Self) -> bool {
        self.oid == other.oid && self.reserved == other.reserved
    }
}
impl PartialEq for StockQuantityModel {
    fn eq(&self, other: &Self) -> bool {
        self.total == other.total
            && self.rsv_detail == other.rsv_detail
            && self.booked == other.booked
            && self.cancelled == other.cancelled
    }
}
impl PartialEq for ProductStockModel {
    fn eq(&self, other: &Self) -> bool {
        self.id_ == other.id_
            && self.quantity == other.quantity
            && self.expiry_without_millis() == other.expiry_without_millis()
    }
}

impl StockQuantityModel {
    pub fn new(
        total: u32,
        cancelled: u32,
        booked: u32,
        rsv_detail: Option<StockQtyRsvModel>,
    ) -> Self {
        Self {
            total,
            cancelled,
            booked,
            rsv_detail,
        }
    }
    pub fn num_avail(&self) -> u32 {
        self.total - self.cancelled - self.booked
    }
    pub fn reserve(&mut self, oid: &str, num_req: u32) -> u32 {
        let n_avail = self.num_avail();
        let mut num_taking = min(n_avail, num_req);
        if num_taking > 0 {
            if let Some(r) = self.rsv_detail.as_mut() {
                if r.oid.as_str() == oid {
                    r.reserved += num_taking;
                } else {
                    num_taking = 0;
                }
            } else {
                self.rsv_detail = Some(StockQtyRsvModel {
                    oid: oid.to_string(),
                    reserved: num_taking,
                });
            }
        }
        if num_taking > 0 {
            self.booked += num_taking;
        }
        num_taking
    }
    pub fn try_return(&mut self, num_req: u32) -> u32 {
        if let Some(r) = self.rsv_detail.as_mut() {
            let n_taking = min(r.reserved, num_req);
            r.reserved -= n_taking;
            self.booked -= n_taking;
            n_taking
        } else {
            0
        }
    }
} // end of impl StockQuantityModel

fn dtime_without_millis(value: &DateTime<Utc>) -> DateTime<Utc> {
    let orig_tz = value.timezone();
    let ts_secs = value.timestamp(); // erase milliseconds
    let _dt = DateTime::from_timestamp(ts_secs, 0).unwrap();
    _dt.with_timezone(&orig_tz)
    //println!("time1:{}, time2: {}", self.expiry.to_rfc3339(), out.to_rfc3339());
}
impl ProductStockIdentity {
    pub fn expiry_without_millis(&self) -> DateTime<Utc> {
        dtime_without_millis(&self.expiry)
    }
}
impl ProductStockModel {
    pub fn expiry_without_millis(&self) -> DateTime<Utc> {
        dtime_without_millis(&self.expiry)
    }
}

impl StoreStockModel {
    pub fn try_reserve(
        &mut self,
        oid: &str,
        req: &OrderLineModel,
    ) -> Option<(OrderLineCreateErrorReason, u32)> {
        let mut num_required = req.qty.reserved;
        let _satisfied = self
            .products
            .iter()
            .filter(|p| req.id().product_id == p.id_)
            .any(|p| {
                let num_taking = min(p.quantity.num_avail(), num_required);
                num_required -= num_taking;
                num_required == 0
            }); // dry-run
        if num_required == 0 {
            assert!(_satisfied);
            num_required = req.qty.reserved;
            let _ = self
                .products
                .iter_mut()
                .filter(|p| req.id().product_id == p.id_)
                .any(|p| {
                    let num_taking = p.quantity.reserve(oid, num_required);
                    num_required -= num_taking;
                    num_required == 0
                });
            None
        } else if num_required < req.qty.reserved {
            Some((OrderLineCreateErrorReason::NotEnoughToClaim, num_required))
        } else {
            Some((OrderLineCreateErrorReason::OutOfStock, num_required))
        }
    }

    pub fn return_across_expiry(
        &mut self,
        req: InventoryEditStockLevelDto,
    ) -> Option<StockReturnErrorReason> {
        assert!(req.qty_add > 0);
        let mut num_returning = req.qty_add as u32;
        let _ = self
            .products
            .iter()
            .filter(|p| p.id_ == req.product_id)
            .any(|p| {
                if let Some(rsv) = p.quantity.rsv_detail.as_ref() {
                    let num_return = min(rsv.reserved, num_returning);
                    num_returning -= num_return;
                }
                num_returning == 0
            }); // dry-run
        if num_returning == 0 {
            num_returning = req.qty_add as u32;
            let _ = self
                .products
                .iter_mut()
                .filter(|p| p.id_ == req.product_id)
                .any(|p| {
                    let num_returned = p.quantity.try_return(num_returning);
                    num_returning -= num_returned;
                    num_returning == 0
                });
            assert_eq!(num_returning, 0);
            None
        } else if num_returning < (req.qty_add as u32) {
            Some(StockReturnErrorReason::InvalidQuantity)
        } else {
            Some(StockReturnErrorReason::NotExist)
        }
    } // end of fn return_across_expiry

    pub fn return_by_expiry(
        &mut self,
        req: InventoryEditStockLevelDto,
    ) -> Option<StockReturnErrorReason> {
        assert!(req.qty_add > 0);
        let result = self.products.iter_mut().find(|p| {
            p.id_ == req.product_id && p.expiry.trunc_subsecs(0) == req.expiry.trunc_subsecs(0)
        });
        if let Some(p) = result {
            if let Some(rsv) = &p.quantity.rsv_detail {
                let num_returning = req.qty_add as u32;
                if rsv.reserved >= num_returning {
                    let num_returned = p.quantity.try_return(num_returning);
                    assert_eq!(num_returning, num_returned);
                    None
                } else {
                    Some(StockReturnErrorReason::InvalidQuantity)
                }
            } else {
                Some(StockReturnErrorReason::InvalidQuantity)
            }
        } else {
            Some(StockReturnErrorReason::NotExist)
        }
    } // end of fn return_by_expiry
} // end of impl StoreStockModel

impl From<StockLevelModelSet> for Vec<StockLevelPresentDto> {
    fn from(value: StockLevelModelSet) -> Vec<StockLevelPresentDto> {
        value
            .stores
            .into_iter()
            .flat_map(|m| {
                let store_id = m.store_id;
                m.products.into_iter().map(move |p| StockLevelPresentDto {
                    quantity: p.quantity.clone().into(),
                    store_id,
                    product_id: p.id_,
                    expiry: p.expiry.fixed_offset(),
                })
            })
            .collect()
    }
}

type InnerStoreStockReturnFn =
    fn(&mut StoreStockModel, InventoryEditStockLevelDto) -> Option<StockReturnErrorReason>;

impl StockLevelModelSet {
    pub fn update(
        mut self,
        data: Vec<InventoryEditStockLevelDto>,
    ) -> DefaultResult<Self, AppError> {
        let mut errmsg = None;
        let err_caught = data.into_iter().find(|d| {
            let result = self.stores.iter_mut().find(|m| m.store_id == d.store_id);
            let store_found = if let Some(m) = result {
                m
            } else {
                let m = StoreStockModel {
                    store_id: d.store_id,
                    products: vec![],
                };
                self.stores.push(m);
                self.stores.last_mut().unwrap()
            }; // TODO,refactor
            let result = store_found.products.iter_mut().find(|m| {
                let duration = m.expiry.fixed_offset() - d.expiry;
                m.id_ == d.product_id && duration.num_seconds() == 0
            });
            if let Some(_product_found) = result {
                if d.qty_add >= 0 {
                    _product_found.quantity.total += d.qty_add as u32;
                } else {
                    let num_avail =
                        _product_found.quantity.total - _product_found.quantity.cancelled;
                    let num_cancel = num_avail.min(d.qty_add.unsigned_abs());
                    _product_found.quantity.cancelled += num_cancel;
                } // TODO, consider to adjust  `product.quantity.booked` whenever customers :
                  // - reserved stock items but cancel them later without paying them.
                  // - return product items they paid (or even received) before the warranty
                false
            } else {
                // insert new instance
                if d.qty_add >= 0 {
                    let new_prod = ProductStockModel {
                        id_: d.product_id,
                        expiry: d.expiry.into(),
                        is_create: true,
                        quantity: StockQuantityModel::new(d.qty_add as u32, 0, 0, None),
                    };
                    store_found.products.push(new_prod);
                    false
                } else {
                    errmsg = Some("negative-initial-quantity");
                    true
                }
            }
        }); // end of input-data iteration
        if let Some(d) = err_caught {
            let msg = errmsg.unwrap_or("");
            let final_detail = format!(
                "store:{}, product:{}, exp:{}, qty_add:{}, reason:{}",
                d.store_id,
                d.product_id,
                d.expiry.to_rfc3339(),
                d.qty_add,
                msg
            );
            Err(AppError {
                code: AppErrorCode::InvalidInput,
                detail: Some(final_detail),
            })
        } else {
            Ok(self)
        }
    } // end of fn update

    // If error happenes in the middle with some internal fields modified,
    // this model instance will be no longer clean and should be discarded immediately.
    pub fn try_reserve(&mut self, ol_set: &OrderLineModelSet) -> Vec<OrderLineCreateErrorDto> {
        self.sort_by_expiry(true);
        let oid = ol_set.id().as_str();
        ol_set
            .lines()
            .iter()
            .filter_map(|req| {
                let mut error = OrderLineCreateErrorDto {
                    seller_id: req.id().store_id,
                    product_id: req.id().product_id,
                    rsv_limit: None,
                    shortage: None,
                    reason: OrderLineCreateErrorReason::NotExist,
                    nonexist: None,
                };
                let result = self
                    .stores
                    .iter_mut()
                    .find(|m| req.id().store_id == m.store_id);
                let opt_err = if let Some(store) = result {
                    if let Some((errtype, num)) = store.try_reserve(oid, req) {
                        error.shortage = Some(num);
                        Some(errtype)
                    } else {
                        None
                    }
                } else {
                    error.nonexist = Some(OrderLineCreateErrNonExistDto {
                        product_policy: false,
                        product_price: false,
                        stock_seller: true,
                    });
                    Some(OrderLineCreateErrorReason::NotExist)
                };
                if let Some(e) = opt_err {
                    error.reason = e;
                    Some(error)
                } else {
                    None
                }
            })
            .collect()
    } // end of try_reserve

    fn return_common(
        &mut self,
        data: StockLevelReturnDto,
        store_fn: InnerStoreStockReturnFn,
    ) -> Vec<StockReturnErrorDto> {
        data.items
            .into_iter()
            .filter_map(|req| {
                let mut error = StockReturnErrorDto {
                    reason: StockReturnErrorReason::NotExist,
                    product_id: req.product_id,
                    seller_id: req.store_id,
                };
                let found = self.stores.iter_mut().find(|m| m.store_id == req.store_id);
                let opt_detail = if let Some(store) = found {
                    store_fn(store, req)
                } else {
                    Some(StockReturnErrorReason::NotExist)
                };
                if let Some(r) = opt_detail {
                    error.reason = r;
                    Some(error)
                } else {
                    None
                }
            })
            .collect()
    } // end of fn return_across_expiry

    pub fn return_across_expiry(&mut self, data: StockLevelReturnDto) -> Vec<StockReturnErrorDto> {
        self.sort_by_expiry(false);
        self.return_common(data, StoreStockModel::return_across_expiry)
    }
    pub fn return_by_expiry(&mut self, data: StockLevelReturnDto) -> Vec<StockReturnErrorDto> {
        self.return_common(data, StoreStockModel::return_by_expiry)
    }

    fn sort_by_expiry(&mut self, ascending: bool) {
        // to ensure the items that expire soon will be taken first
        self.stores
            .iter_mut()
            .map(|s| {
                s.products.sort_by(|a, b| {
                    if ascending {
                        a.expiry.cmp(&b.expiry)
                    } else {
                        b.expiry.cmp(&a.expiry)
                    }
                });
            })
            .count();
    } // end of sort_by_expiry
} // end of impl StockLevelModelSet
