use std::boxed::Box;
use std::collections::HashMap;
use std::sync::Arc;
use std::result::Result as DefaultResult;

use async_trait::async_trait;
use chrono::{DateTime, FixedOffset};

use crate::constant::ProductType;
use crate::datastore::{AbstInMemoryDStore, AppInMemFetchedSingleRow};
use crate::error::{AppError, AppErrorCode};
use crate::model::{OrderLineIdentity, OrderReturnModel};
use super::super::AbsOrderReturnRepo;

mod _oline_return {
    use crate::datastore::AbsDStoreFilterKeyOp;
    use crate::model::{OrderReturnQuantityModel, OrderLinePriceModel};
    use super::{HashMap, DateTime, FixedOffset, OrderReturnModel, ProductType};

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
    pub(super) fn inmem_get_oid(pkey:&str) -> &str {
        pkey.split("-").next().unwrap()
    }
    pub(super) fn inmem_qty2col(saved:Option<String>, map:OrderReturnQuantityModel) -> String
    {
        let orig = if let Some(s) = saved {
            s + QTY_DELIMITER
        } else { String::new() };
        let new = map.into_iter().map(|(time, (qty, refund))| {
            format!("{} {} {} {}", time.format(QTY_KEY_FORMAT).to_string(),
                qty, refund.unit, refund.total)
        }).collect::<Vec<_>>().join(QTY_DELIMITER);
        orig + new.as_str()
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
    pub(super) fn inmem_filt_qty_detail(
        src: OrderReturnModel, start: DateTime<FixedOffset>, end: DateTime<FixedOffset>
    ) -> OrderReturnModel
    {
        let (id_, map) = (src.id_, src.qty);
        let drained = map.into_iter().filter(
            |(time, _combo)| ((&start <= time) && (time <= &end))
        );
        let map = HashMap::from_iter(drained);
        assert!(!map.is_empty());
        OrderReturnModel {id_, qty:map}
    }
    pub(super) struct InMemDStoreFilterTimeRangeOp<'a> {
        pub t0: DateTime<FixedOffset>,
        pub t1: DateTime<FixedOffset>,
        pub oid: Option<&'a str>,
    }
    impl<'a> AbsDStoreFilterKeyOp for InMemDStoreFilterTimeRangeOp<'a> {
        fn filter(&self, key:&String, row:&Vec<String>) -> bool {
            let passed = if let Some(d) = self.oid.as_ref() {
                let curr_oid = inmem_get_oid(key);
                d.eq(&curr_oid)
            } else {true};
            if passed {
                let col_idx:usize = InMemColIdx::QtyRefund.into();
                let qty_raw = row.get(col_idx).unwrap();
                let map = inmem_col2qty(qty_raw.clone());
                map.keys().into_iter().any(|t| ((&self.t0 <= t) && (t <= &self.t1)) )
            } else { false }
        }
    }
} // end of inner module _oline_return

struct InsertOpArg(OrderReturnModel, Option<String>);

pub struct OrderReturnInMemRepo {
    datastore: Arc<Box<dyn AbstInMemoryDStore>>,
}

impl From<InsertOpArg> for AppInMemFetchedSingleRow {
    fn from(value: InsertOpArg) -> Self
    {
        let (model, serial_saved_qty) = (value.0, value.1);
        let (id_, map) = (model.id_ , model.qty);
        let qty_serial = _oline_return::inmem_qty2col(serial_saved_qty, map);
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
        let op = _oline_return::InMemDStoreFilterTimeRangeOp {t0:start, t1:end, oid:None};
        let pkeys = self.datastore.filter_keys(table_name.to_string(), &op).await?;
        let info = HashMap::from([(table_name.to_string(), pkeys)]);
        let mut data = self.datastore.fetch(info).await?;
        let rows = data.remove(table_name).unwrap();
        let out = rows.into_iter().map(|(key, row)| {
            let oid = _oline_return::inmem_get_oid(key.as_str()).to_string();
            let ret = _oline_return::inmem_filt_qty_detail(row.into(), start, end);
            (oid, ret)
        }).collect();
        Ok(out)
    }
    async fn fetch_by_oid_ctime(&self, oid:&str, start: DateTime<FixedOffset>, end: DateTime<FixedOffset>)
        -> DefaultResult<Vec<OrderReturnModel>, AppError>
    {
        let table_name = _oline_return::TABLE_LABEL;
        let op = _oline_return::InMemDStoreFilterTimeRangeOp {t0:start, t1:end, oid:Some(oid)};
        let pkeys = self.datastore.filter_keys(table_name.to_string(), &op).await?;
        let info = HashMap::from([(table_name.to_string(), pkeys)]);
        let mut data = self.datastore.fetch(info).await?;
        let rows = data.remove(table_name).unwrap();
        let out = rows.into_values().map(|row| {
            _oline_return::inmem_filt_qty_detail(row.into(), start, end)
        }).collect();
        Ok(out)
    }
    async fn create(&self, oid:&str, reqs:Vec<OrderReturnModel>) -> DefaultResult<usize, AppError>
    {
        let qty_empty = reqs.iter().find_map(|req| {
            if req.qty.is_empty() {
                let detail = format!("return-req, in-mem-repo, prod-id: {} {:?} {}",
                        req.id_.store_id,  req.id_.product_type, req.id_.product_id);
                Some(detail)
            } else {None}
        });
        if qty_empty.is_some() {
            return Err(AppError {code:AppErrorCode::EmptyInputData, detail:qty_empty });
        }
        let table_name = _oline_return::TABLE_LABEL.to_string();
        let num_saved = reqs.iter().map(|r| r.qty.len()).sum();
        let mut info = vec![];
        for req in reqs {
            let pkey = _oline_return::inmem_pkey(oid, req.id_.store_id,
                            req.id_.product_type.clone(),  req.id_.product_id );
            // load saved `qty` inner table
            let _info = HashMap::from([(table_name.clone(), vec![pkey.clone()])]);
            let mut _data = self.datastore.fetch(_info).await?;
            let rows = _data.remove(table_name.as_str()).unwrap() ;
            let rows = rows.into_values().collect::<Vec<_>>() ;
            assert!([0usize,1].contains(&rows.len()));
            let serial_saved_qty = if let Some(row) = rows.first() {
                let v = row.get::<usize>(_oline_return::InMemColIdx::QtyRefund.into())
                    .unwrap().to_owned();
                Some(v)
            } else {None};
            let m_serial = AppInMemFetchedSingleRow::from(InsertOpArg(req, serial_saved_qty));
            let item = (pkey, m_serial);
            info.push(item);
        } // end of loop
        let rows = HashMap::from_iter(info.into_iter());
        let data = HashMap::from([(table_name, rows)]);
        let _num_saved_ds = self.datastore.save(data).await?;
        Ok(num_saved)
    } // end of fn create
} // end of OrderReturnInMemRepo

impl OrderReturnInMemRepo {
    pub async fn new(m:Arc<Box<dyn AbstInMemoryDStore>>) -> DefaultResult<Self, AppError>
    {
        m.create_table(_oline_return::TABLE_LABEL).await?;
        Ok(Self {datastore:m.clone()}) 
    }
}
