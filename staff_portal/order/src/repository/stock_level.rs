use std::boxed::Box;
use std::sync::Arc;
use std::collections::HashMap;
use std::result::Result as DefaultResult;

use async_trait::async_trait;
use chrono::{DateTime, FixedOffset};

use crate::constant::ProductType;
use crate::datastore::{
    AbstInMemoryDStore, AppInMemDstoreLock, AppInMemFetchedData, AppInMemFetchedSingleTable
};
use crate::error::{AppError, AppErrorCode};
use crate::model::{
    ProductStockModel, StoreStockModel, StockQuantityModel, ProductStockIdentity2,  ProductStockIdentity,
    StockLevelModelSet, OrderLineModelSet
};

use super::{AbsOrderStockRepo, AppStockRepoReserveUserFunc, AppStockRepoReserveReturn};

mod _stockm {
    use std::collections::HashSet;
    use crate::datastore::AbsDStoreFilterKeyOp;
    use super::{ProductStockIdentity2, DateTime, FixedOffset};

    pub(super) const TABLE_LABEL: &'static str = "order_stock_lvl";
    pub(super) const EXPIRY_KEY_FORMAT: &'static str = "%Y%m%d%H%M%S%z";
    pub(super) enum InMemColIdx {Expiry, QtyTotal, QtyBooked, QtyCancelled, TotNumColumns}
    impl Into<usize> for InMemColIdx {
        fn into(self) -> usize {
            match self {
                Self::Expiry => 0,  Self::QtyTotal  => 1,
                Self::QtyBooked => 2, Self::QtyCancelled  => 3,
                Self::TotNumColumns => 4,
            }
        }
    }
    pub(super) struct InMemDStoreFiltKeyOp {
        // it is combo of seller-id, product-type as u8, product-id
        options : HashSet<(u32, u8, u64)>,
        timenow: DateTime<FixedOffset>
    }
    impl AbsDStoreFilterKeyOp for InMemDStoreFiltKeyOp {
        fn filter(&self, k:&String) -> bool {
            let id_elms = k.split("/").collect::<Vec<&str>>();
            let (store_id, prod_typ, prod_id, exp_from_combo) = (
                id_elms[0].parse().unwrap(),  id_elms[1].parse().unwrap(),
                id_elms[2].parse().unwrap(),
                DateTime::parse_from_str(id_elms[3], EXPIRY_KEY_FORMAT).unwrap()
            );
            if self.options.contains(&(store_id, prod_typ, prod_id)) {
                // business logic in domain model should include more advanced expiry check,
                // this repository simply filters out the stock items which have expired
                exp_from_combo > self.timenow
            } else {false}
        }
    } // to fetch all keys in stock-level table whose records haven't expired yet.
    impl InMemDStoreFiltKeyOp {
        pub fn new(pids: Vec<ProductStockIdentity2>, timenow: DateTime<FixedOffset>) -> Self {
            let iter = pids.into_iter().map(|d| {
                let prod_typ_num:u8 = d.product_type.into();
                (d.store_id, prod_typ_num, d.product_id)
            });
            Self { timenow, options:HashSet::from_iter(iter) }
        }
    }
} // end of inner module _stockm

impl Into<StockLevelModelSet> for AppInMemFetchedSingleTable {
    fn into(self) -> StockLevelModelSet {
        let mut out = StockLevelModelSet {stores:vec![]};
        let _ = self.into_iter().map(|(key, row)| {
            let id_elms = key.split("/").collect::<Vec<&str>>();
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
                let exp_fmt_verify = m.expiry.format(_stockm::EXPIRY_KEY_FORMAT).to_string();
                m.type_==prod_typ && m.id_==prod_id && exp_fmt_verify==exp_from_combo
            });
            if let Some(_product_rd) = result {
                let _prod_typ_num:u8 = _product_rd.type_.clone().into();
                panic!("report error, data corruption, store:{}, product: ({}, {})", 
                       store_rd.store_id, _prod_typ_num, _product_rd.id_);
                // TODO, return error instead 
            } else {
                let total = row.get::<usize>(_stockm::InMemColIdx::QtyTotal.into())
                    .unwrap().parse().unwrap();
                let booked = row.get::<usize>(_stockm::InMemColIdx::QtyBooked.into())
                    .unwrap().parse().unwrap();
                let cancelled = row.get::<usize>(_stockm::InMemColIdx::QtyCancelled.into())
                    .unwrap().parse().unwrap();
                let expiry = row.get::<usize>(_stockm::InMemColIdx::Expiry.into()).unwrap();
                let expiry = DateTime::parse_from_rfc3339(&expiry).unwrap();
                let m = ProductStockModel {is_create:false, type_:prod_typ, id_:prod_id,
                    expiry, quantity: StockQuantityModel::new(total, cancelled, booked)
                };
                store_rd.products.push(m);
            }
        }).collect::<Vec<()>>();
        out
    }
} // end of impl Into for StockLevelModelSet

impl From<StockLevelModelSet> for AppInMemFetchedSingleTable {
    fn from(value: StockLevelModelSet) -> Self { 
        let kv_pairs = value.stores.iter().flat_map(|m1| {
            m1.products.iter().map(|m2| {
                let exp_fmt = m2.expiry_without_millis().format(_stockm::EXPIRY_KEY_FORMAT);
                let prod_typ_num:u8 = m2.type_.clone().into();
                let pkey = format!("{}/{}/{}/{}", m1.store_id, prod_typ_num, m2.id_, exp_fmt);
                let mut row = (0 .. _stockm::InMemColIdx::TotNumColumns.into())
                    .map(|_n| {String::new()}).collect::<Vec<String>>();
                let _ = [
                    (_stockm::InMemColIdx::QtyCancelled, m2.quantity.cancelled.to_string()),
                    (_stockm::InMemColIdx::QtyBooked, m2.quantity.num_booked().to_string()),
                    (_stockm::InMemColIdx::QtyTotal,  m2.quantity.total.to_string()),
                    (_stockm::InMemColIdx::Expiry,  m2.expiry.to_rfc3339()),
                ].into_iter().map(|(idx, val)| {
                    let idx:usize = idx.into();
                    row[idx] = val;
                }).collect::<Vec<()>>();
                (pkey, row)
            }) // end of inner iter
        }); // end of outer iter
        HashMap::from_iter(kv_pairs)
    }
} // end of impl From for StockLevelModelSet

// in-memory repo is unable to do concurrency test between web app
// and rpc consumer app, also it should't be deployed in production
// environment
pub(super) struct StockLvlInMemRepo
{
    // TODO, figure out how to add AppInMemDstoreLock<'a> to this struct
    // currently this is not allowed due to lifetime difference between
    // the lock guard and this repo type
    datastore: Arc<Box<dyn AbstInMemoryDStore>>,
    curr_time: DateTime<FixedOffset>
}

#[async_trait]
impl AbsOrderStockRepo for StockLvlInMemRepo
{
    async fn fetch(&self, pids:Vec<ProductStockIdentity>) -> DefaultResult<StockLevelModelSet, AppError>
    {
        let ids = pids.into_iter().map(|d| {
            let prod_typ_num:u8 = d.product_type.into();
            let exp_fmt = d.expiry.format(_stockm::EXPIRY_KEY_FORMAT);
            format!("{}/{}/{}/{}", d.store_id, prod_typ_num, d.product_id, exp_fmt)
        }).collect();
        let info = HashMap::from([(_stockm::TABLE_LABEL.to_string(), ids)]);
        let resultset = self.datastore.fetch(info).await ?;
        Self::try_into_modelset(resultset)
    } // end of fn fetch
 
    async fn save(&self, slset:StockLevelModelSet) -> DefaultResult<(), AppError>
    {
        let rows = AppInMemFetchedSingleTable::from(slset);
        let table = (_stockm::TABLE_LABEL.to_string(), rows);
        let data = HashMap::from([table]);
        let _num_saved = self.datastore.save(data).await?;
        Ok(())
    } // end of fn save
    
    async fn try_reserve(&self, usr_cb: AppStockRepoReserveUserFunc,
                         order_req: &OrderLineModelSet) -> AppStockRepoReserveReturn
    {
        let pids = order_req.lines.iter().map(|d|
            ProductStockIdentity2 {product_type:d.product_type.clone(),
                store_id:d.seller_id, product_id:d.product_id}
        ).collect();
        let (mut stock_mset, d_lock) = match self.fetch_for_reserve(pids).await
        {
            Ok(v) => v,
            Err(e) => {return Err(Err(e));}
        };
        usr_cb(&mut stock_mset, order_req)?;
        if let Err(e) = self.save_reserved(stock_mset, d_lock) {
            Err(Err(e))
        } else {
            Ok(())
        }
    } // end of fn try_reserve
} // end of impl StockLvlInMemRepo

impl StockLvlInMemRepo {
    pub async fn build(m:Arc<Box<dyn AbstInMemoryDStore>>, curr_time:DateTime<FixedOffset>)
        -> DefaultResult<Self, AppError>
    {
        m.create_table(_stockm::TABLE_LABEL).await?;
        let out = Self { datastore: m.clone(), curr_time };
        Ok(out)
    }

    async fn fetch_for_reserve(&self, pids:Vec<ProductStockIdentity2>)
        -> DefaultResult<(StockLevelModelSet, AppInMemDstoreLock), AppError> 
    {
        let tbl_label = _stockm::TABLE_LABEL.to_string();
        let op = _stockm::InMemDStoreFiltKeyOp::new(pids, self.curr_time.clone());
        let stock_ids = self.datastore.filter_keys(tbl_label.clone(), &op).await?;
        let info = HashMap::from([(tbl_label, stock_ids)]);
        let (tableset, _lock) = self.datastore.fetch_acquire(info).await?;
        let ms =  Self::try_into_modelset(tableset)?;
        Ok((ms, _lock))
    }
    fn save_reserved(&self, slset:StockLevelModelSet, lock:AppInMemDstoreLock)
        -> DefaultResult<(), AppError>
    {
        let rows = AppInMemFetchedSingleTable::from(slset);
        let table = (_stockm::TABLE_LABEL.to_string(), rows);
        let data = HashMap::from([table]);
        let _num_saved = self.datastore.save_release(data, lock)?;
        Ok(())
    }
    fn try_into_modelset (tableset:AppInMemFetchedData)
        -> DefaultResult<StockLevelModelSet, AppError>
    {
        if let Some((_label, rows)) = tableset.into_iter().next() {
            Ok(rows.into())
        } else {
            Err(AppError { code:AppErrorCode::DataTableNotExist,
                detail:Some(_stockm::TABLE_LABEL.to_string())  })
        }
    } // end of fn try_into_modelset
} // end of impl StockLvlInMemRepo

