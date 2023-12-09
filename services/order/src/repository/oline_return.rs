use std::boxed::Box;
use std::collections::HashMap;
use std::sync::Arc;
use std::result::Result as DefaultResult;

use async_trait::async_trait;
use chrono::{DateTime, FixedOffset};

use crate::AppDataStoreContext;
use crate::constant::ProductType;
use crate::datastore::{AbstInMemoryDStore, AppInMemFetchedSingleRow};
use crate::error::{AppError, AppErrorCode};
use crate::model::{OrderLineIdentity, OrderReturnModel};
use super::AbsOrderReturnRepo;

mod _oline_return {
    use crate::datastore::AbsDStoreFilterKeyOp;
    use crate::model::{OrderReturnQuantityModel, OrderLinePriceModel};
    use super::{HashMap, DateTime, FixedOffset, ProductType};

    pub(super) const TABLE_LABEL:&'static str = "order_line_return";
    pub(super) const QTY_DELIMITER:&'static str = "/";
    pub(super) const QTY_KEY_FORMAT: &'static str = "%Y%m%d%H%M%S%z";
    
    pub(super) enum InMemColIdx { SellerID, ProductType, ProductId, QtyRefund, TotNumColumns}
    impl Into<usize> for InMemColIdx {
        fn into(self) -> usize {
            match self {
                Self::SellerID => 0,   Self::ProductType => 1,
                Self::ProductId => 2,  Self::QtyRefund => 3,
                Self::TotNumColumns => 4
            }
        }
    }
    pub(super) fn inmem_pkey(oid:&str, seller_id:u32, prod_typ:ProductType, prod_id:u64) -> String
    {
        let prod_typ: u8 = prod_typ.into();
        format!("{oid}-{seller_id}-{prod_typ}-{prod_id}")
    }
    pub(super) fn inmem_qty2col(map:OrderReturnQuantityModel) -> String {
        map.into_iter().map(|(time, (qty, refund))| {
            format!("{} {} {} {}", time.format(QTY_KEY_FORMAT).to_string(),
                qty, refund.unit, refund.total)
        }).collect::<Vec<String>>().join(QTY_DELIMITER)
    }
    pub(super) fn inmem_col2qty(raw:String) -> OrderReturnQuantityModel {
        let iter = raw.split(QTY_DELIMITER).map(|tkn| {
            let mut tokens = tkn.split(" ");
            let (time, q, unit, total) = (
                DateTime::parse_from_str(tokens.next().unwrap(), QTY_KEY_FORMAT).unwrap() ,
                tokens.next().unwrap().parse().unwrap(),
                tokens.next().unwrap().parse().unwrap(),
                tokens.next().unwrap().parse().unwrap(),
            );
            (time, (q, OrderLinePriceModel{unit, total}))
        });
        HashMap::from_iter(iter)
    }
    pub(super) struct InMemDStoreFilterTimeRangeOp {
        pub t0: DateTime<FixedOffset>,
        pub t1: DateTime<FixedOffset>,
    }
    impl AbsDStoreFilterKeyOp for InMemDStoreFilterTimeRangeOp {
        fn filter(&self, _key:&String, row:&Vec<String>) -> bool {
            let col_idx:usize = InMemColIdx::QtyRefund.into();
            let qty_raw = row.get(col_idx).unwrap();
            let map = inmem_col2qty(qty_raw.clone());
            map.keys().into_iter().any(|t| ((&self.t0 <= t) && (t <= &self.t1)) )
        }
    }
} // end of inner module _oline_return

pub struct OrderReturnInMemRepo {
    datastore: Arc<Box<dyn AbstInMemoryDStore>>,
}

impl From<OrderReturnModel> for AppInMemFetchedSingleRow {
    fn from(value: OrderReturnModel) -> Self {
        let (id_, map) = (value.id_ , value.qty);
        let qty_serial = _oline_return::inmem_qty2col(map);
        let mut rows = (0.._oline_return::InMemColIdx::TotNumColumns.into())
            .into_iter().map(|_n| String::new()).collect::<Self>();
        let _ = [
            (_oline_return::InMemColIdx::SellerID,  id_.store_id.to_string()),
            (_oline_return::InMemColIdx::ProductId, id_.product_id.to_string()),
            (_oline_return::InMemColIdx::ProductType,
                 <ProductType as Into<u8>>::into(id_.product_type).to_string()),
            (_oline_return::InMemColIdx::QtyRefund, qty_serial),
        ].into_iter().map(|(k, v)| {
            let idx:usize = k.into();
            rows[idx] = v;
        }).count() ;
        rows
    }
} // end of impl AppInMemFetchedSingleRow

impl Into<OrderReturnModel> for AppInMemFetchedSingleRow {
    fn into(self) -> OrderReturnModel {
        let (store_id, product_id, prod_typ_num, qty_serial) = (
            self.get::<usize>(_oline_return::InMemColIdx::SellerID.into())
                .unwrap().to_owned().parse().unwrap() ,
            self.get::<usize>(_oline_return::InMemColIdx::ProductId.into())
                .unwrap().to_owned().parse().unwrap() ,
            self.get::<usize>(_oline_return::InMemColIdx::ProductType.into())
                .unwrap().to_owned().parse::<u8>().unwrap() ,
            self.get::<usize>(_oline_return::InMemColIdx::QtyRefund.into())
                .unwrap().to_owned(),
        );
        let product_type = ProductType::from(prod_typ_num);
        OrderReturnModel {
            id_: OrderLineIdentity {store_id, product_id, product_type},
            qty: _oline_return::inmem_col2qty(qty_serial)
        }
    }
}

#[async_trait]
impl AbsOrderReturnRepo for OrderReturnInMemRepo
{
    async fn new(ds:Arc<AppDataStoreContext>) -> DefaultResult<Box<dyn AbsOrderReturnRepo>, AppError>
        where Self: Sized
    {
        let obj = Self::build(ds).await ? ;
        Ok(Box::new(obj))
    }
    async fn fetch_by_pid(&self, oid:&str, pids:Vec<OrderLineIdentity>)
        -> DefaultResult<Vec<OrderReturnModel>, AppError>
    {
        let table_name = _oline_return::TABLE_LABEL;
        let pkeys = pids.into_iter().map(|p| {
            _oline_return::inmem_pkey(oid, p.store_id, p.product_type, p.product_id)
        }).collect();
        let info = HashMap::from([(table_name.to_string(), pkeys)]);
        let mut data = self.datastore.fetch(info).await?;
        let rows = data.remove(table_name).unwrap();
        let out = rows.into_values().map(AppInMemFetchedSingleRow::into).collect();
        Ok(out)
    }
    async fn fetch_by_created_time(&self, start: DateTime<FixedOffset>, end: DateTime<FixedOffset>)
        -> DefaultResult<Vec<(String, OrderReturnModel)>, AppError>
    {
        let table_name = _oline_return::TABLE_LABEL;
        let op = _oline_return::InMemDStoreFilterTimeRangeOp {t0:start, t1:end};
        let pkeys = self.datastore.filter_keys(table_name.to_string(), &op).await?;
        let info = HashMap::from([(table_name.to_string(), pkeys)]);
        let mut data = self.datastore.fetch(info).await?;
        let rows = data.remove(table_name).unwrap();
        let out = rows.into_iter().map(|(key, row)| {
            let oid = key.split("-").next().unwrap().to_string();
            (oid, row.into())
        }).collect();
        Ok(out)
    }
    async fn fetch_by_oid_ctime(&self, oid:&str, start: DateTime<FixedOffset>, end: DateTime<FixedOffset>)
        -> DefaultResult<Vec<OrderReturnModel>, AppError>
    {
        Ok(vec![])
    }
    async fn save(&self, oid:&str, reqs:Vec<OrderReturnModel>) -> DefaultResult<usize, AppError>
    {
        let table_name = _oline_return::TABLE_LABEL.to_string();
        let info = reqs.into_iter().map(|r| {
            let pkey = _oline_return::inmem_pkey(oid, r.id_.store_id,
                            r.id_.product_type.clone(),  r.id_.product_id );
            (pkey, AppInMemFetchedSingleRow::from(r))
        });
        let rows = HashMap::from_iter(info);
        let data = HashMap::from([(table_name, rows)]);
        let num_saved = self.datastore.save(data).await?;
        Ok(num_saved)
    }
} // end of OrderReturnInMemRepo

impl OrderReturnInMemRepo {
    pub async fn build(ds:Arc<AppDataStoreContext>) -> DefaultResult<Self, AppError>
    {
        if let Some(m) = ds.in_mem.as_ref() {
            m.create_table(_oline_return::TABLE_LABEL).await?;
            Ok(Self {datastore:m.clone()}) 
        } else {
            Err(AppError {code:AppErrorCode::MissingDataStore,
                detail: Some(format!("in-memory"))}  )
        }
    }
}
