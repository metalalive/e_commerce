use std::cmp::min;
use std::vec::Vec;
use std::collections::{HashMap, HashSet};
use std::collections::hash_map::RandomState;
use std::result::Result as DefaultResult;

use chrono::DateTime;
use chrono::offset::FixedOffset;

use crate::api::rpc::dto::{
    InventoryEditStockLevelDto, StockLevelPresentDto, StockQuantityPresentDto, StockLevelReturnDto, StockReturnErrorDto, StockReturnErrorReason
};
use crate::api::web::dto::{OrderLineCreateErrorDto, OrderLineCreateErrorReason, OrderLineCreateErrNonExistDto};
use crate::constant::ProductType;
use crate::error::{AppError, AppErrorCode};

use super::{OrderLineModelSet, OrderLineModel, BaseProductIdentity, dtime_without_millis};

pub struct ProductStockIdentity {
    pub store_id: u32,
    pub product_type: ProductType,
    pub product_id: u64, // TODO, declare type alias
    pub expiry: DateTime<FixedOffset>,
}
pub type ProductStockIdentity2 = BaseProductIdentity; // TODO, rename

#[derive(Debug)]
pub struct StockQuantityModel {
    pub total: u32,
    pub cancelled: u32,
    rsv_detail: HashMap<String, u32>, // reserved quantity for specific order ID
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
        StockQuantityPresentDto { total: self.total, cancelled: self.cancelled,
            booked: self.num_booked() }
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
        Self {total:self.total, cancelled:self.cancelled,
            rsv_detail:self.rsv_detail.clone() }
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
        let b1:HashSet<(&String,&u32), RandomState>  = HashSet::from_iter(self.rsv_detail.iter());
        let b2 = HashSet::from_iter(other.rsv_detail.iter());
        let rsv_any_diff = b2.difference(&b1).any(|(_k, _v)| true);
        self.total == other.total  && rsv_any_diff == false
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
    pub fn new(total:u32, cancelled:u32, detail:Option<Vec<(&str,u32)>>) -> Self
    {
        let rsv_detail = if let Some(d) = detail {
            let data_iter = d.into_iter().map(|(k, v)| (k.to_string(), v));
            HashMap::from_iter(data_iter)
        } else { HashMap::new() };
        Self { total, cancelled, rsv_detail }
    }
    pub fn reservation(&self) -> &HashMap<String, u32> {
        &self.rsv_detail
    }
    pub fn num_booked(&self) -> u32 {
        self.rsv_detail.values().sum()
    }
    pub fn num_avail(&self) -> u32 {
        self.total - self.cancelled - self.num_booked()
    }
    pub fn reserve(&mut self, oid:&str, num_req:u32) -> u32
    {
        let n_avail = self.num_avail();
        let num_taking = min(n_avail, num_req);
        if num_taking > 0 {
            if let Some(entry) = self.rsv_detail.get_mut(oid) {
                *entry += num_taking;
            } else {
                self.rsv_detail.insert(oid.to_string(), num_taking);
            }
        }
        num_taking
    }
    pub fn try_return(&mut self, oid:&str, num_req:u32) -> u32
    {
        if let Some(entry) = self.rsv_detail.get_mut(oid) {
            let n_taking = min(*entry, num_req);
            *entry -= n_taking;
            if *entry == 0 {
                let _ = self.rsv_detail.remove(oid);
            }
            n_taking
        } else { 0 }
    }
} // end of impl StockQuantityModel

impl ProductStockModel {
    pub fn expiry_without_millis(&self) -> DateTime<FixedOffset>
    { dtime_without_millis(&self.expiry) }
}

impl StoreStockModel {
    pub fn try_reserve(&mut self, oid:&str, req:&OrderLineModel) -> Option<(OrderLineCreateErrorReason, u32)>
    {
        let mut num_required = req.qty.reserved;
        let _satisfied = self.products.iter().filter(|p| {
            req.id_.product_type == p.type_ && req.id_.product_id == p.id_
        }).any(|p| {
            let num_taking = min(p.quantity.num_avail(), num_required);
            num_required -= num_taking;
            num_required == 0
        }); // dry-run
        if num_required == 0 {
            assert!(_satisfied);
            num_required = req.qty.reserved;
            let _ = self.products.iter_mut().filter(|p| {
                req.id_.product_type == p.type_ && req.id_.product_id == p.id_
            }).any(|p| {
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

    pub fn return_across_expiry(&mut self, oid:&str, req:InventoryEditStockLevelDto)
        -> Option<StockReturnErrorReason>
    {
        assert!(req.qty_add > 0);
        let mut num_returning = req.qty_add as u32;
        let _ = self.products.iter().filter(|p| {
            p.type_ == req.product_type && p.id_ == req.product_id
        }).any(|p| {
            if let Some(num_rsved) = p.quantity.reservation().get(oid) {
                let num_return  = min(*num_rsved, num_returning);
                num_returning -= num_return;
            }
            num_returning == 0
        }); // dry-run
        if num_returning == 0 {
            num_returning = req.qty_add as u32;
            let _ = self.products.iter_mut().filter(|p| {
                p.type_ == req.product_type && p.id_ == req.product_id
            }).any(|p| {
                let num_returned  = p.quantity.try_return(oid, num_returning);
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
    
    pub fn return_by_id(&mut self, oid:&str, req:InventoryEditStockLevelDto)
        -> Option<StockReturnErrorReason>
    {
        assert!(req.qty_add > 0);
        let result = self.products.iter_mut().find(|p| {
            p.type_ == req.product_type && p.id_ == req.product_id && p.expiry == req.expiry
        });
        if let Some(p) = result  {
            if let Some(num_rsved) = p.quantity.reservation().get(oid) {
                let num_returning = req.qty_add as u32;
                if *num_rsved >= num_returning {
                    let num_returned  = p.quantity.try_return(oid, num_returning);
                    assert_eq!(num_returning, num_returned);
                    None
                } else { Some(StockReturnErrorReason::InvalidQuantity) }
            } else { Some(StockReturnErrorReason::InvalidQuantity) }
        } else { Some(StockReturnErrorReason::NotExist) }
    } // end of fn return_by_id
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

type InnerStoreStockReturnFn = fn(&mut StoreStockModel, &str, InventoryEditStockLevelDto)
    -> Option<StockReturnErrorReason>;

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
                    let new_prod = ProductStockModel {type_: d.product_type.clone(),
                        id_: d.product_id, expiry: d.expiry, is_create: true, 
                        quantity: StockQuantityModel::new(d.qty_add as u32, 0, None) 
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
        self.sort_by_expiry(true);
        let oid = ol_set.order_id.as_str();
        ol_set.lines.iter().filter_map(|req| {
            let mut error = OrderLineCreateErrorDto {seller_id:req.id_.store_id,
                product_id:req.id_.product_id, product_type:req.id_.product_type.clone(),
                reason: OrderLineCreateErrorReason::NotExist,  nonexist:None, shortage:None
            };
            let result = self.stores.iter_mut().find(|m| {req.id_.store_id == m.store_id});
            let opt_err = if let Some(store) = result {
                if let Some((errtype, num)) = store.try_reserve(oid, req) {
                    error.shortage = Some(num);
                    Some(errtype)
                } else { None }
            } else {
                error.nonexist = Some(OrderLineCreateErrNonExistDto { product_policy: false,
                    product_price: false, stock_seller:true });
                Some(OrderLineCreateErrorReason::NotExist)
            };
            if let Some(e) = opt_err {
                error.reason = e;
                Some(error)
            } else { None }
        }) .collect()
    } // end of try_reserve
    
    fn return_common(&mut self, data:StockLevelReturnDto, store_fn: InnerStoreStockReturnFn)
        -> Vec<StockReturnErrorDto>
    {
        let oid = data.order_id.as_str();
        data.items.into_iter().filter_map(|req| {
            let mut error = StockReturnErrorDto {
                reason: StockReturnErrorReason::NotExist, product_id: req.product_id,
                seller_id: req.store_id, product_type: req.product_type.clone()
            };
            let found = self.stores.iter_mut().find(|m| {m.store_id == req.store_id});
            let opt_detail = if let Some(store) = found {
                store_fn(store, oid, req)
            } else { Some(StockReturnErrorReason::NotExist) };
            if let Some(r) = opt_detail {
                error.reason = r;
                Some(error)
            } else { None }
        }).collect()
    } // end of fn return_across_expiry
    
    pub fn return_across_expiry(&mut self, data:StockLevelReturnDto) -> Vec<StockReturnErrorDto>
    {
        self.sort_by_expiry(false);
        self.return_common(data, StoreStockModel::return_across_expiry)
    } 
    pub fn return_by_id(&mut self, data:StockLevelReturnDto) -> Vec<StockReturnErrorDto>
    {
        self.return_common(data, StoreStockModel::return_by_id)
    }
    
    fn sort_by_expiry(&mut self, ascending:bool) {
        // to ensure the items that expire soon will be taken first
        self.stores.iter_mut().map(|s| {
            s.products.sort_by(|a, b| {
                if ascending { a.expiry.cmp(&b.expiry) }
                else { b.expiry.cmp(&a.expiry) }
            });
        }).count();
    } // end of sort_by_expiry
} // end of impl StockLevelModelSet
