use std::cmp::min;
use std::vec::{Vec, self};
use std::result::Result as DefaultResult;

use chrono::DateTime;
use chrono::offset::FixedOffset;

use crate::api::rpc::dto::{InventoryEditStockLevelDto, StockLevelPresentDto, StockQuantityPresentDto};
use crate::api::web::dto::{OrderLineCreateErrorDto, OrderLineErrorReason, OrderLineCreateErrNonExistDto};
use crate::constant::ProductType;
use crate::error::{AppError, AppErrorCode};

use super::{OrderLineModelSet, OrderLineModel};

pub struct ProductStockIdentity {
    pub store_id: u32,
    pub product_type: ProductType,
    pub product_id: u64, // TODO, declare type alias
    pub expiry: DateTime<FixedOffset>,
}
pub struct ProductStockIdentity2 {
    pub store_id: u32,
    pub product_type: ProductType,
    pub product_id: u64, // TODO, declare type alias
} // TODO, rename

#[derive(Debug)]
pub struct StockQuantityModel {
    pub total: u32,
    pub cancelled: u32,
    booked: u32,
    // TODO, new field for reservation detail, such as order ID and quantity
}
#[derive(Debug)]
pub struct ProductStockModel {
    pub type_: ProductType,
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

impl Into<StockQuantityPresentDto> for StockQuantityModel {
    fn into(self) -> StockQuantityPresentDto {
        StockQuantityPresentDto { total: self.total, booked: self.booked,
            cancelled: self.cancelled }
    }
}

impl Clone for ProductStockIdentity {
    fn clone(&self) -> Self {
        Self { store_id: self.store_id, product_type: self.product_type.clone(),
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
        Self { type_: self.type_.clone(), id_: self.id_, expiry: self.expiry.clone(),
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

impl StockQuantityModel {
    pub fn new(total:u32, cancelled:u32, booked:u32) -> Self {
        Self { total, cancelled, booked }
    }
    pub fn num_avail(&self) -> u32 {
        self.total - self.cancelled - self.booked
    }
    pub fn num_booked(&self) -> u32 { self.booked }
    
    pub fn reserve(&mut self, num:u32) {
        self.booked += num;
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

impl StoreStockModel {
    pub fn try_reserve(&mut self, req:&OrderLineModel) -> Option<(OrderLineErrorReason, u32)>
    {
        let mut num_required = req.qty.reserved;
        let _satisfied = self.products.iter().filter(|p| {
            req.product_type == p.type_ && req.product_id == p.id_
        }).any(|p| {
            let num_taking = min(p.quantity.num_avail(), num_required);
            num_required -= num_taking;
            num_required == 0
        }); // dry-run
        if num_required == 0 {
            assert!(_satisfied);
            num_required = req.qty.reserved;
            let _ = self.products.iter_mut().filter(|p| {
                req.product_type == p.type_ && req.product_id == p.id_
            }).any(|p| {
                let num_taking = min(p.quantity.num_avail(), num_required);
                p.quantity.reserve(num_taking);
                num_required -= num_taking;
                num_required == 0
            });
            None
        } else if num_required < req.qty.reserved {
            Some((OrderLineErrorReason::NotEnoughToClaim, num_required))
        } else {
            Some((OrderLineErrorReason::OutOfStock, num_required))
        }
    }
} // end of impl StoreStockModel

impl Into<Vec<StockLevelPresentDto>> for StockLevelModelSet {
    fn into(self) -> Vec<StockLevelPresentDto>
    {
        self.stores.into_iter().flat_map(|m| {
            let store_id = m.store_id;
            m.products.into_iter().map(move |p| {
                StockLevelPresentDto {
                    quantity: p.quantity.clone().into(), store_id, product_type: p.type_,
                    product_id: p.id_,  expiry: p.expiry.clone()
                }
            })
        }).collect()
    }
}

impl StockLevelModelSet {
    pub fn update(mut self, data:Vec<InventoryEditStockLevelDto>)
        -> DefaultResult<Self, AppError>
    {
        let mut errmsg = None;
        let err_caught = data.into_iter().find(|d| {
            let result = self.stores.iter_mut().find(|m| m.store_id==d.store_id);
            let store_found = if let Some(m) = result {
                m
            } else {
                let m = StoreStockModel {store_id:d.store_id, products:vec![]};
                self.stores.push(m);
                self.stores.last_mut().unwrap()
            }; // TODO,refactor
            let result = store_found.products.iter_mut().find(|m| {
                let duration = m.expiry - d.expiry;
                m.type_==d.product_type && m.id_==d.product_id && duration.num_seconds() == 0
            });
            if let Some(_product_found) = result {
                if d.qty_add >= 0 {
                    _product_found.quantity.total += d.qty_add as u32;
                } else {
                    let num_avail = _product_found.quantity.total - _product_found.quantity.cancelled;
                    let num_cancel = num_avail.min(d.qty_add.abs() as u32);
                    _product_found.quantity.cancelled += num_cancel;
                } // TODO, consider to adjust  `product.quantity.booked` whenever customers :
                  // - reserved stock items but cancel them later without paying them.
                  // - return product items they paid (or even received) before the warranty
                false
            } else { // insert new instance
                if d.qty_add >= 0 {
                    let new_prod = ProductStockModel { is_create: true, type_: d.product_type.clone(),
                        id_: d.product_id, expiry: d.expiry,  quantity: StockQuantityModel {
                            total: d.qty_add as u32, booked: 0, cancelled: 0}};
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
            let prod_typ_num:u8 = d.product_type.into();
            let final_detail = format!("store:{}, product:({},{}), exp:{}, qty_add:{}, reason:{}",
                                   d.store_id, prod_typ_num, d.product_id, d.expiry.to_rfc3339(),
                                   d.qty_add, msg) ;
            Err(AppError { code: AppErrorCode::InvalidInput, detail: Some(final_detail) })
        } else {
            Ok(self)
        }
    } // end of fn update


    // If error happenes in the middle with some internal fields modified,
    // this model instance will be no longer clean and should be discarded immediately.
    pub fn try_reserve(&mut self, ol_set:&OrderLineModelSet) -> Vec<OrderLineCreateErrorDto>
    {
        self.sort_by_expiry();
        ol_set.lines.iter().filter_map(|req| {
            let mut error = OrderLineCreateErrorDto {seller_id:req.seller_id,
                product_id:req.product_id, product_type:req.product_type.clone(),
                reason: OrderLineErrorReason::NotExist,  nonexist:None, shortage:None
            };
            let result = self.stores.iter_mut().find(|m| {req.seller_id == m.store_id});
            let opt_err = if let Some(store) = result {
                if let Some((errtype, num_required)) = store.try_reserve(req) {
                    error.shortage = Some(num_required);
                    Some(errtype)
                } else { None }
            } else {
                error.nonexist = Some(OrderLineCreateErrNonExistDto { product_policy: false,
                    product_price: false, stock_seller:true });
                Some(OrderLineErrorReason::NotExist)
            };
            if let Some(e) = opt_err {
                error.reason = e;
                Some(error)
            } else { None }
        }) .collect()
    } // end of try_reserve
    
    fn sort_by_expiry(&mut self) {
        // to ensure the items that expire soon will be taken first
        self.stores.iter_mut().map(|s| {
            s.products.sort_by(|a, b| a.expiry.cmp(&b.expiry));
        }).count();
    } // end of sort_by_expiry
} // end of impl StockLevelModelSet

