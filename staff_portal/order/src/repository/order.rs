use std::boxed::Box;
use std::sync::Arc;
use std::collections::HashMap;
use std::result::Result as DefaultResult;

use async_trait::async_trait;
use chrono::DateTime;

use crate::AppDataStoreContext;
use crate::constant::ProductType;
use crate::datastore::AbstInMemoryDStore;
use crate::error::{AppError, AppErrorCode};
use crate::model::{StockLevelModelSet, ProductStockIdentity, ProductStockModel, StoreStockModel, StockQuantityModel};

use super::{AbsOrderRepo, AbsOrderStockRepo};

mod _stock {
    pub(super) const TABLE_LABEL: &'static str = "order_stock_lvl";
    pub(super) enum InMemColIdx {Expiry, QtyTotal, QtyBooked, QtyCancelled, TotNumColumns}
    impl Into<usize> for InMemColIdx {
        fn into(self) -> usize {
            match self {
                Self::Expiry => 0,
                Self::QtyTotal  => 1,
                Self::QtyBooked => 2,
                Self::QtyCancelled  => 3,
                Self::TotNumColumns => 4,
            }
        }
    }
} // end of inner module _stock

struct StockLvlInMemRepo {
    datastore: Arc<Box<dyn AbstInMemoryDStore>>,
    _expiry_key_fmt:String,
}
pub struct OrderInMemRepo {
    _stock: Arc<Box<dyn AbsOrderStockRepo>>
}

#[async_trait]
impl AbsOrderStockRepo for StockLvlInMemRepo {
    async fn fetch(&self, pids:Vec<ProductStockIdentity>) -> DefaultResult<StockLevelModelSet, AppError>
    {
        let ids = pids.into_iter().map(|d| {
            let prod_typ_num:u8 = d.product_type.into();
            let exp_fmt = d.expiry.format(self._expiry_key_fmt.as_str());
            format!("{}-{}-{}-{}", d.store_id, prod_typ_num, d.product_id, exp_fmt)
        }).collect();
        let info = HashMap::from([(_stock::TABLE_LABEL.to_string(), ids)]);
        let resultset = self.datastore.fetch(info) ?;
        if let Some((_label, rows)) = resultset.into_iter().next() {
            let mut out = StockLevelModelSet {stores:vec![]};
            let _ = rows.into_iter().map(|(key, row)| {
                let id_elms = key.split("-").collect::<Vec<&str>>();
                let prod_typ_num:u8 = id_elms[1].parse().unwrap();
                let (store_id, prod_typ, prod_id, exp_from_combo) = (
                    id_elms[0].parse().unwrap(),  ProductType::from(prod_typ_num),
                    id_elms[2].parse().unwrap(),  id_elms[3]    );
                let result = out.stores.iter_mut().find(|m| m.store_id==store_id);
                let store_rd = if let Some(m) = result {
                    m
                } else {
                    let m = StoreStockModel {store_id, products:vec![]};
                    out.stores.push(m);
                    out.stores.last_mut().unwrap()
                };
                let result = store_rd.products.iter().find(|m| {
                    let exp_fmt_verify = m.expiry.format(self._expiry_key_fmt.as_str()).to_string();
                    m.type_==prod_typ && m.id_==prod_id && exp_fmt_verify==exp_from_combo
                });
                if let Some(_product_rd) = result {
                    let _prod_typ_num:u8 = _product_rd.type_.clone().into();
                    panic!("report error, data corruption, store:{}, product: ({}, {})", 
                           store_rd.store_id, _prod_typ_num, _product_rd.id_);
                    // TODO, return error instead 
                } else {
                    let total = row.get::<usize>(_stock::InMemColIdx::QtyTotal.into())
                        .unwrap().parse().unwrap();
                    let booked = row.get::<usize>(_stock::InMemColIdx::QtyBooked.into())
                        .unwrap().parse().unwrap();
                    let cancelled = row.get::<usize>(_stock::InMemColIdx::QtyCancelled.into())
                        .unwrap().parse().unwrap();
                    let expiry = row.get::<usize>(_stock::InMemColIdx::Expiry.into()).unwrap();
                    let expiry = DateTime::parse_from_rfc3339(&expiry).unwrap();
                    let m = ProductStockModel {is_create:false, type_:prod_typ, id_:prod_id,
                        expiry, quantity: StockQuantityModel{total, booked, cancelled}
                    };
                    store_rd.products.push(m);
                }
            }).collect::<Vec<()>>();
            Ok(out)
        } else {
            Err(AppError { code:AppErrorCode::DataTableNotExist,
                detail:Some(_stock::TABLE_LABEL.to_string())  })
        }
    } // end of fn fetch

    
    async fn save(&self, slset:StockLevelModelSet) -> DefaultResult<(), AppError>
    {
        let rows = {
            let kv_pairs = slset.stores.iter().flat_map(|m1| {
                m1.products.iter().map(|m2| {
                    let exp_fmt = m2.expiry_without_millis().format(self._expiry_key_fmt.as_str());
                    let prod_typ_num:u8 = m2.type_.clone().into();
                    let pkey = format!("{}-{}-{}-{}", m1.store_id, prod_typ_num, m2.id_, exp_fmt);
                    let mut row = (0 .. _stock::InMemColIdx::TotNumColumns.into())
                        .map(|_n| {String::new()}).collect::<Vec<String>>();
                    let _ = [
                        (_stock::InMemColIdx::QtyCancelled, m2.quantity.cancelled.to_string()),
                        (_stock::InMemColIdx::QtyBooked, m2.quantity.booked.to_string()),
                        (_stock::InMemColIdx::QtyTotal,  m2.quantity.total.to_string()),
                        (_stock::InMemColIdx::Expiry,  m2.expiry.to_rfc3339()),
                    ].into_iter().map(|(idx, val)| {
                        let idx:usize = idx.into();
                        row[idx] = val;
                    }).collect::<Vec<()>>();
                    (pkey, row)
                }) // end of inner iter
            }); // end of outer iter
            HashMap::from_iter(kv_pairs)
        };
        let table = (_stock::TABLE_LABEL.to_string(), rows);
        let data = HashMap::from([table]);
        let _num_saved = self.datastore.save(data)?;
        Ok(())
    } // end of fn save
} // end of impl StockLvlInMemRepo


impl AbsOrderRepo for OrderInMemRepo {
    fn new(ds:Arc<AppDataStoreContext>) -> DefaultResult<Box<dyn AbsOrderRepo>, AppError>
        where Self:Sized
    {
        match Self::build(ds) {
            Ok(obj) => Ok(Box::new(obj)),
            Err(e) => Err(e)
        }
    }
    fn stock(&self) -> Arc<Box<dyn AbsOrderStockRepo>>
    { self._stock.clone() }
} // end of impl AbsOrderRepo


impl OrderInMemRepo {
    pub fn build(ds:Arc<AppDataStoreContext>) -> DefaultResult<Self, AppError>
    {
        if let Some(m) = &ds.in_mem {
            m.create_table(self::_stock::TABLE_LABEL)?;
            let _stock = StockLvlInMemRepo {datastore: m.clone(),
                _expiry_key_fmt:"%Y%m%d%H%M%S".to_string() };
            let obj = Self{_stock:Arc::new(Box::new(_stock))};
            Ok(obj)
        } else {
            Err(AppError {code:AppErrorCode::MissingDataStore,
                detail: Some(format!("in-memory"))}  )
        }
    }
}
