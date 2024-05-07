use std::boxed::Box;
use std::collections::HashMap;
use std::result::Result as DefaultResult;
use std::sync::Arc;

use async_trait::async_trait;
use chrono::{DateTime, FixedOffset};

use ecommerce_common::constant::ProductType;
use ecommerce_common::error::AppErrorCode;

use crate::api::rpc::dto::{StockLevelReturnDto, StockReturnErrorDto};
use crate::datastore::{
    AbstInMemoryDStore, AppInMemDstoreLock, AppInMemFetchedData, AppInMemFetchedSingleTable,
};
use crate::error::AppError;
use crate::model::{
    OrderLineModelSet, ProductStockIdentity, ProductStockIdentity2, ProductStockModel,
    StockLevelModelSet, StockQtyRsvModel, StockQuantityModel, StoreStockModel,
};

use super::super::{
    AbsOrderStockRepo, AppStockRepoReserveReturn, AppStockRepoReserveUserFunc,
    AppStockRepoReturnUserFunc,
};
use super::order::OrderInMemRepo;

mod _stockm {
    use super::{DateTime, FixedOffset, ProductStockIdentity2};
    use crate::datastore::AbsDStoreFilterKeyOp;
    use std::collections::HashSet;

    pub(super) const TABLE_LABEL: &str = "order_stock_lvl";
    pub(super) const EXPIRY_KEY_FORMAT: &str = "%Y%m%d%H%M%S%z";
    pub(super) enum InMemColIdx {
        Expiry,
        QtyTotal,
        QtyRsvDetail,
        QtyCancelled,
        TotNumColumns,
    }
    impl From<InMemColIdx> for usize {
        fn from(value: InMemColIdx) -> usize {
            match value {
                InMemColIdx::Expiry => 0,
                InMemColIdx::QtyTotal => 1,
                InMemColIdx::QtyRsvDetail => 2,
                InMemColIdx::QtyCancelled => 3,
                InMemColIdx::TotNumColumns => 4,
            }
        }
    }
    pub(super) struct InMemDStoreFiltKeyOp {
        // it is combo of seller-id, product-type as u8, product-id
        options: HashSet<(u32, u8, u64)>,
        timenow: Option<DateTime<FixedOffset>>,
    }
    impl AbsDStoreFilterKeyOp for InMemDStoreFiltKeyOp {
        fn filter(&self, k: &String, _v: &Vec<String>) -> bool {
            let id_elms = k.split('/').collect::<Vec<&str>>();
            let (store_id, prod_typ, prod_id, exp_from_combo) = (
                id_elms[0].parse().unwrap(),
                id_elms[1].parse().unwrap(),
                id_elms[2].parse().unwrap(),
                DateTime::parse_from_str(id_elms[3], EXPIRY_KEY_FORMAT).unwrap(),
            );
            if self.options.contains(&(store_id, prod_typ, prod_id)) {
                // business logic in domain model should include more advanced expiry check,
                // this repository simply filters out the stock items which have expired
                if let Some(v) = self.timenow.as_ref() {
                    &exp_from_combo > v
                } else {
                    true
                }
            } else {
                false
            }
        }
    } // to fetch all keys in stock-level table whose records haven't expired yet.
    impl InMemDStoreFiltKeyOp {
        pub fn new(
            pids: Vec<ProductStockIdentity2>,
            timenow: Option<DateTime<FixedOffset>>,
        ) -> Self {
            let iter = pids.into_iter().map(|d| {
                let prod_typ_num: u8 = d.product_type.into();
                (d.store_id, prod_typ_num, d.product_id)
            });
            Self {
                timenow,
                options: HashSet::from_iter(iter),
            }
        }
    }
} // end of inner module _stockm

// list of tuple with order-id and number of reserved for each order
type FetchedRsv = Vec<(String, u32)>;
struct FetchedRsvSet(HashMap<String, FetchedRsv>);
struct FetchArg(AppInMemFetchedSingleTable, Option<String>);
struct SaveArg(StockLevelModelSet, FetchedRsvSet);

impl FetchArg {
    fn create_iter_rsv(row: &[String]) -> impl Iterator<Item = (String, u32)> + '_ {
        let rsv_str = row
            .get::<usize>(_stockm::InMemColIdx::QtyRsvDetail.into())
            .unwrap();
        rsv_str.split(' ').filter_map(|d| {
            let mut kv = d.split('/');
            if let (Some(k), Some(v)) = (kv.next(), kv.next()) {
                let k = k.to_string();
                let v = v.parse().unwrap();
                Some((k, v))
            } else {
                None
            }
        })
    }
    fn to_product_stock(
        prod_typ: ProductType,
        prod_id: u64,
        row: Vec<String>,
        maybe_order_id: &Option<String>,
    ) -> ProductStockModel {
        let rsv = Self::create_iter_rsv(&row).collect::<FetchedRsv>();
        let total = row
            .get::<usize>(_stockm::InMemColIdx::QtyTotal.into())
            .unwrap()
            .parse()
            .unwrap();
        let rsv_detail = if let Some(orderid) = maybe_order_id {
            rsv.iter()
                .find(|(oid, _num)| oid.as_str() == orderid.as_str())
                .map(|d| StockQtyRsvModel {
                    oid: d.0.clone(),
                    reserved: d.1,
                })
        } else {
            None
        };
        let cancelled = row
            .get::<usize>(_stockm::InMemColIdx::QtyCancelled.into())
            .unwrap()
            .parse()
            .unwrap();
        let booked = rsv.iter().map(|(_oid, num)| num).sum();
        let expiry = row
            .get::<usize>(_stockm::InMemColIdx::Expiry.into())
            .unwrap();
        let expiry = DateTime::parse_from_rfc3339(expiry).unwrap();
        ProductStockModel {
            is_create: false,
            type_: prod_typ,
            id_: prod_id,
            expiry: expiry.into(),
            quantity: StockQuantityModel::new(total, cancelled, booked, rsv_detail),
        }
    }
} // end of impl FetchArg
#[allow(clippy::from_over_into)]
impl Into<StockLevelModelSet> for FetchArg {
    fn into(self) -> StockLevelModelSet {
        let (rows, maybe_order_id) = (self.0, self.1);
        let mut out = StockLevelModelSet { stores: vec![] };
        rows.into_iter()
            .map(|(key, row)| {
                let id_elms = key.split('/').collect::<Vec<&str>>();
                let prod_typ_num: u8 = id_elms[1].parse().unwrap();
                let (store_id, prod_typ, prod_id, exp_from_combo) = (
                    id_elms[0].parse().unwrap(),
                    ProductType::from(prod_typ_num),
                    id_elms[2].parse::<u64>().unwrap(),
                    id_elms[3],
                );
                let result = out.stores.iter_mut().find(|m| m.store_id == store_id);
                let store_rd = if let Some(m) = result {
                    m
                } else {
                    let m = StoreStockModel {
                        store_id,
                        products: vec![],
                    };
                    out.stores.push(m);
                    out.stores.last_mut().unwrap()
                };
                let result = store_rd.products.iter().find(|m| {
                    let exp_fmt_verify = m.expiry.format(_stockm::EXPIRY_KEY_FORMAT).to_string();
                    m.type_ == prod_typ && m.id_ == prod_id && exp_fmt_verify == exp_from_combo
                });
                if let Some(_product_rd) = result {
                    // TODO, return error instead
                    let _prod_typ_num: u8 = _product_rd.type_.clone().into();
                    panic!(
                        "report error, data corruption, store:{}, product: ({}, {})",
                        store_rd.store_id, _prod_typ_num, _product_rd.id_
                    );
                } else {
                    let m = Self::to_product_stock(prod_typ, prod_id, row, &maybe_order_id);
                    store_rd.products.push(m);
                }
            })
            .count();
        out
    }
} // end of impl Into for StockLevelModelSet

impl From<&AppInMemFetchedSingleTable> for FetchedRsvSet {
    fn from(value: &AppInMemFetchedSingleTable) -> Self {
        let iter = value.iter().map(|(key, row)| {
            let rsv = FetchArg::create_iter_rsv(row).collect::<FetchedRsv>();
            (key.clone(), rsv)
        });
        let map = HashMap::from_iter(iter);
        Self(map)
    }
}

impl From<SaveArg> for AppInMemFetchedSingleTable {
    fn from(value: SaveArg) -> Self {
        let (slset, FetchedRsvSet(rsv_set)) = (value.0, value.1);
        let kv_pairs = slset.stores.iter().flat_map(|m1| {
            m1.products.iter().map(|m2| {
                let exp_fmt = m2
                    .expiry_without_millis()
                    .format(_stockm::EXPIRY_KEY_FORMAT);
                let prod_typ_num: u8 = m2.type_.clone().into();
                let pkey = format!("{}/{}/{}/{}", m1.store_id, prod_typ_num, m2.id_, exp_fmt);
                let rsv_prod = if let Some(r) = rsv_set.get(pkey.as_str()) {
                    r.clone()
                } else {
                    Vec::new()
                };
                let rsv_prod = if let Some(r) = &m2.quantity.rsv_detail {
                    let mut rsv = rsv_prod
                        .into_iter()
                        .filter(|(oid, _)| oid.as_str() != r.oid.as_str())
                        .collect::<Vec<_>>();
                    if r.reserved > 0 {
                        rsv.push((r.oid.clone(), r.reserved));
                    } // otherwise delete the reservation by excluding it
                    rsv
                } else {
                    rsv_prod
                };
                let rsv_detail_str = rsv_prod
                    .into_iter()
                    .map(|(oid, n_rsved)| format!("{oid}/{n_rsved}"))
                    .collect::<Vec<_>>()
                    .join(" ");
                let mut row = (0.._stockm::InMemColIdx::TotNumColumns.into())
                    .map(|_n| String::new())
                    .collect::<Vec<String>>();
                let _ = [
                    (
                        _stockm::InMemColIdx::QtyCancelled,
                        m2.quantity.cancelled.to_string(),
                    ),
                    (_stockm::InMemColIdx::QtyRsvDetail, rsv_detail_str),
                    (
                        _stockm::InMemColIdx::QtyTotal,
                        m2.quantity.total.to_string(),
                    ),
                    (_stockm::InMemColIdx::Expiry, m2.expiry.to_rfc3339()),
                ]
                .into_iter()
                .map(|(idx, val)| {
                    let idx: usize = idx.into();
                    row[idx] = val;
                })
                .collect::<Vec<()>>();
                (pkey, row)
            }) // end of inner iter
        }); // end of outer iter
        HashMap::from_iter(kv_pairs)
    }
} // end of impl From for StockLevelModelSet

// in-memory repo is unable to do concurrency test between web app
// and rpc consumer app, also it should't be deployed in production
// environment
pub(super) struct StockLvlInMemRepo {
    // TODO, figure out how to add AppInMemDstoreLock<'a> to this struct
    // currently this is not allowed due to lifetime difference between
    // the lock guard and this repo type
    datastore: Arc<Box<dyn AbstInMemoryDStore>>,
    curr_time: DateTime<FixedOffset>,
}

#[async_trait]
impl AbsOrderStockRepo for StockLvlInMemRepo {
    async fn fetch(
        &self,
        pids: Vec<ProductStockIdentity>,
    ) -> DefaultResult<StockLevelModelSet, AppError> {
        let ids = pids
            .into_iter()
            .map(|d| {
                let prod_typ_num: u8 = d.product_type.into();
                let exp_fmt = d.expiry.format(_stockm::EXPIRY_KEY_FORMAT);
                format!(
                    "{}/{}/{}/{}",
                    d.store_id, prod_typ_num, d.product_id, exp_fmt
                )
            })
            .collect();
        let info = HashMap::from([(_stockm::TABLE_LABEL.to_string(), ids)]);
        let resultset = self.datastore.fetch(info).await?;
        Self::try_into_modelset(None, resultset)
    } // end of fn fetch

    async fn save(&self, slset: StockLevelModelSet) -> DefaultResult<(), AppError> {
        let rsv_set = {
            let ids = slset
                .stores
                .iter()
                .flat_map(|s| {
                    s.products.iter().map(|p| {
                        let prod_typ_num: u8 = p.type_.clone().into();
                        let exp_fmt = p.expiry.format(_stockm::EXPIRY_KEY_FORMAT);
                        format!("{}/{}/{}/{}", s.store_id, prod_typ_num, p.id_, exp_fmt)
                    })
                })
                .collect();
            let info = HashMap::from([(_stockm::TABLE_LABEL.to_string(), ids)]);
            let results = self.datastore.fetch(info).await?;
            let rows = results.into_values().next().unwrap();
            FetchedRsvSet::from(&rows)
        };
        let rows = AppInMemFetchedSingleTable::from(SaveArg(slset, rsv_set));
        let table = (_stockm::TABLE_LABEL.to_string(), rows);
        let data = HashMap::from([table]);
        let _num_saved = self.datastore.save(data).await?;
        Ok(())
    } // end of fn save

    async fn try_reserve(
        &self,
        usr_cb: AppStockRepoReserveUserFunc,
        order_req: &OrderLineModelSet,
    ) -> AppStockRepoReserveReturn {
        let pids = order_req
            .lines
            .iter()
            .map(|d| ProductStockIdentity2 {
                product_type: d.id_.product_type.clone(),
                store_id: d.id_.store_id,
                product_id: d.id_.product_id,
            })
            .collect();
        let (mut stock_mset, rsv_set, d_lock) = match self
            .fetch_with_lock(order_req.order_id.clone(), pids, Some(self.curr_time))
            .await
        {
            Ok(v) => v,
            Err(e) => {
                return Err(Err(e));
            }
        };
        usr_cb(&mut stock_mset, order_req)?;
        let data = {
            let mut seq = OrderInMemRepo::in_mem_olines(order_req);
            let rows = AppInMemFetchedSingleTable::from(SaveArg(stock_mset, rsv_set));
            seq.insert(0, (_stockm::TABLE_LABEL.to_string(), rows));
            HashMap::from_iter(seq)
        };
        if let Err(e) = self.datastore.save_release(data, d_lock) {
            Err(Err(e))
        } else {
            Ok(())
        }
    } // end of fn try_reserve

    async fn try_return(
        &self,
        cb: AppStockRepoReturnUserFunc,
        data: StockLevelReturnDto,
    ) -> DefaultResult<Vec<StockReturnErrorDto>, AppError> {
        let pids = data
            .items
            .iter()
            .map(|d| ProductStockIdentity2 {
                product_type: d.product_type.clone(),
                store_id: d.store_id,
                product_id: d.product_id,
            })
            .collect();
        // omit expiry check in the key filter
        let (mut mset, rsv_set, d_lock) = self
            .fetch_with_lock(data.order_id.clone(), pids, None)
            .await?;
        let caller_errors = cb(&mut mset, data);
        if caller_errors.is_empty() {
            let rows = AppInMemFetchedSingleTable::from(SaveArg(mset, rsv_set));
            let table = (_stockm::TABLE_LABEL.to_string(), rows);
            let data = HashMap::from([table]);
            let _num_saved = self.datastore.save_release(data, d_lock)?;
        }
        Ok(caller_errors)
    }
} // end of impl StockLvlInMemRepo

impl StockLvlInMemRepo {
    pub async fn build(
        m: Arc<Box<dyn AbstInMemoryDStore>>,
        curr_time: DateTime<FixedOffset>,
    ) -> DefaultResult<Self, AppError> {
        m.create_table(_stockm::TABLE_LABEL).await?;
        let out = Self {
            datastore: m.clone(),
            curr_time,
        };
        Ok(out)
    }

    async fn fetch_with_lock(
        &self,
        order_id: String,
        pids: Vec<ProductStockIdentity2>,
        curr_time: Option<DateTime<FixedOffset>>,
    ) -> DefaultResult<(StockLevelModelSet, FetchedRsvSet, AppInMemDstoreLock), AppError> {
        let tbl_label = _stockm::TABLE_LABEL.to_string();
        let op = _stockm::InMemDStoreFiltKeyOp::new(pids, curr_time);
        let stock_ids = self.datastore.filter_keys(tbl_label.clone(), &op).await?;
        let info = HashMap::from([(tbl_label, stock_ids)]);
        let (tableset, lock) = self.datastore.fetch_acquire(info).await?;
        let rsv_set = {
            let rows = tableset.values().next().unwrap();
            FetchedRsvSet::from(rows)
        };
        let ms = Self::try_into_modelset(Some(order_id), tableset)?;
        Ok((ms, rsv_set, lock))
    }
    fn try_into_modelset(
        order_id: Option<String>,
        tableset: AppInMemFetchedData,
    ) -> DefaultResult<StockLevelModelSet, AppError> {
        if let Some((_label, rows)) = tableset.into_iter().next() {
            Ok(FetchArg(rows, order_id).into())
        } else {
            Err(AppError {
                code: AppErrorCode::DataTableNotExist,
                detail: Some(_stockm::TABLE_LABEL.to_string()),
            })
        }
    } // end of fn try_into_modelset
} // end of impl StockLvlInMemRepo
