use std::boxed::Box;
use std::collections::HashMap;
use std::result::Result as DefaultResult;
use std::sync::Arc;

use async_trait::async_trait;
use chrono::{DateTime, FixedOffset};

use ecommerce_common::error::AppErrorCode;

use super::super::AbsOrderReturnRepo;
use crate::datastore::{AbstInMemoryDStore, AppInMemFetchedSingleRow};
use crate::error::AppError;
use crate::model::{OrderLineIdentity, OrderReturnModel};

mod _oline_return {
    use super::{DateTime, FixedOffset, HashMap, OrderReturnModel};
    use crate::datastore::AbsDStoreFilterKeyOp;
    use crate::model::{OrderLinePriceModel, OrderReturnQuantityModel};

    #[allow(clippy::redundant_static_lifetimes)]
    pub(super) const TABLE_LABEL: &'static str = "order_line_return";
    // static lifetime annotated by default
    pub(super) const QTY_DELIMITER: &str = "/";
    pub(super) const QTY_KEY_FORMAT: &str = "%Y%m%d%H%M%S%z";

    #[rustfmt::skip]
    pub(super) enum InMemColIdx {
        SellerID, ProductId, AttrSetSeq, QtyRefund, TotNumColumns,
    }
    impl From<InMemColIdx> for usize {
        fn from(value: InMemColIdx) -> usize {
            match value {
                InMemColIdx::SellerID => 0,
                InMemColIdx::ProductId => 1,
                InMemColIdx::AttrSetSeq => 2,
                InMemColIdx::QtyRefund => 3,
                InMemColIdx::TotNumColumns => 4,
            }
        }
    }
    pub(super) fn inmem_pkey(oid: &str, seller_id: u32, prod_id: u64, attr_seq: u16) -> String {
        format!("{oid}-{seller_id}-{prod_id}-{attr_seq}")
    }
    pub(super) fn inmem_get_oid(pkey: &str) -> &str {
        pkey.split('-').next().unwrap()
    }
    pub(super) fn inmem_qty2col(saved: Option<String>, map: OrderReturnQuantityModel) -> String {
        let orig = if let Some(s) = saved {
            s + QTY_DELIMITER
        } else {
            String::new()
        };
        let new = map
            .into_iter()
            .map(|(time, (qty, refund))| {
                format!(
                    "{} {} {} {}",
                    time.format(QTY_KEY_FORMAT),
                    qty,
                    refund.unit(),
                    refund.total()
                )
            })
            .collect::<Vec<_>>()
            .join(QTY_DELIMITER);
        orig + new.as_str()
    }
    pub(super) fn inmem_col2qty(raw: String) -> OrderReturnQuantityModel {
        let iter = raw.split(QTY_DELIMITER).map(|tkn| {
            let mut tokens = tkn.split(' ');
            let (time, q, unit, total) = (
                DateTime::parse_from_str(tokens.next().unwrap(), QTY_KEY_FORMAT).unwrap(),
                tokens.next().unwrap().parse().unwrap(),
                tokens.next().unwrap().parse().unwrap(),
                tokens.next().unwrap().parse().unwrap(),
            );
            (time, (q, OrderLinePriceModel::from((unit, total))))
        });
        HashMap::from_iter(iter)
    }
    pub(super) fn inmem_filt_qty_detail(
        src: OrderReturnModel,
        start: DateTime<FixedOffset>,
        end: DateTime<FixedOffset>,
    ) -> OrderReturnModel {
        let (id_, map) = (src.id_, src.qty);
        let drained = map
            .into_iter()
            .filter(|(time, _combo)| ((&start <= time) && (time <= &end)));
        let map = HashMap::from_iter(drained);
        assert!(!map.is_empty());
        OrderReturnModel { id_, qty: map }
    }
    pub(super) struct InMemDStoreFilterTimeRangeOp<'a> {
        pub t0: DateTime<FixedOffset>,
        pub t1: DateTime<FixedOffset>,
        pub oid: Option<&'a str>,
    }
    impl<'a> AbsDStoreFilterKeyOp for InMemDStoreFilterTimeRangeOp<'a> {
        fn filter(&self, key: &String, row: &Vec<String>) -> bool {
            let passed = if let Some(d) = self.oid.as_ref() {
                let curr_oid = inmem_get_oid(key);
                d.eq(&curr_oid)
            } else {
                true
            };
            if passed {
                let col_idx: usize = InMemColIdx::QtyRefund.into();
                let qty_raw = row.get(col_idx).unwrap();
                let map = inmem_col2qty(qty_raw.clone());
                map.keys().any(|t| ((&self.t0 <= t) && (t <= &self.t1)))
            } else {
                false
            }
        }
    }
} // end of inner module _oline_return

struct InsertOpArg(OrderReturnModel, Option<String>);

pub struct OrderReturnInMemRepo {
    datastore: Arc<Box<dyn AbstInMemoryDStore>>,
}

impl From<InsertOpArg> for AppInMemFetchedSingleRow {
    fn from(value: InsertOpArg) -> Self {
        let (model, serial_saved_qty) = (value.0, value.1);
        let (id_, map) = (model.id_, model.qty);
        let qty_serial = _oline_return::inmem_qty2col(serial_saved_qty, map);
        let mut rows = (0.._oline_return::InMemColIdx::TotNumColumns.into())
            .map(|_n| String::new())
            .collect::<Self>();
        let _ = [
            (
                _oline_return::InMemColIdx::SellerID,
                id_.store_id().to_string(),
            ),
            (
                _oline_return::InMemColIdx::ProductId,
                id_.product_id().to_string(),
            ),
            (
                _oline_return::InMemColIdx::AttrSetSeq,
                id_.attrs_seq_num().to_string(),
            ),
            (_oline_return::InMemColIdx::QtyRefund, qty_serial),
        ]
        .into_iter()
        .map(|(k, v)| {
            let idx: usize = k.into();
            rows[idx] = v;
        })
        .count();
        rows
    }
} // end of impl AppInMemFetchedSingleRow

impl From<AppInMemFetchedSingleRow> for OrderReturnModel {
    fn from(value: AppInMemFetchedSingleRow) -> OrderReturnModel {
        let store_id = value
            .get::<usize>(_oline_return::InMemColIdx::SellerID.into())
            .unwrap()
            .to_owned()
            .parse()
            .unwrap();
        let product_id = value
            .get::<usize>(_oline_return::InMemColIdx::ProductId.into())
            .unwrap()
            .to_owned()
            .parse()
            .unwrap();
        let attr_seq = value
            .get::<usize>(_oline_return::InMemColIdx::AttrSetSeq.into())
            .unwrap()
            .to_owned()
            .parse()
            .unwrap();
        let qty_serial = value
            .get::<usize>(_oline_return::InMemColIdx::QtyRefund.into())
            .unwrap()
            .to_owned();

        OrderReturnModel {
            id_: OrderLineIdentity::from((store_id, product_id, attr_seq)),
            qty: _oline_return::inmem_col2qty(qty_serial),
        }
    }
}

#[async_trait]
impl AbsOrderReturnRepo for OrderReturnInMemRepo {
    async fn fetch_by_pid(
        &self,
        oid: &str,
        pids: Vec<OrderLineIdentity>,
    ) -> DefaultResult<Vec<OrderReturnModel>, AppError> {
        let table_name = _oline_return::TABLE_LABEL;
        let pkeys = pids
            .into_iter()
            .map(|p| {
                _oline_return::inmem_pkey(oid, p.store_id(), p.product_id(), p.attrs_seq_num())
            })
            .collect();
        let info = HashMap::from([(table_name.to_string(), pkeys)]);
        let mut data = self.datastore.fetch(info).await?;
        let rows = data.remove(table_name).unwrap();
        let out = rows
            .into_values()
            .map(AppInMemFetchedSingleRow::into)
            .collect();
        Ok(out)
    }
    async fn fetch_by_created_time(
        &self,
        start: DateTime<FixedOffset>,
        end: DateTime<FixedOffset>,
    ) -> DefaultResult<Vec<(String, OrderReturnModel)>, AppError> {
        let table_name = _oline_return::TABLE_LABEL;
        let op = _oline_return::InMemDStoreFilterTimeRangeOp {
            t0: start,
            t1: end,
            oid: None,
        };
        let pkeys = self
            .datastore
            .filter_keys(table_name.to_string(), &op)
            .await?;
        let info = HashMap::from([(table_name.to_string(), pkeys)]);
        let mut data = self.datastore.fetch(info).await?;
        let rows = data.remove(table_name).unwrap();
        let out = rows
            .into_iter()
            .map(|(key, row)| {
                let oid = _oline_return::inmem_get_oid(key.as_str()).to_string();
                let ret = _oline_return::inmem_filt_qty_detail(row.into(), start, end);
                (oid, ret)
            })
            .collect();
        Ok(out)
    }
    async fn fetch_by_oid_ctime(
        &self,
        oid: &str,
        start: DateTime<FixedOffset>,
        end: DateTime<FixedOffset>,
    ) -> DefaultResult<Vec<OrderReturnModel>, AppError> {
        let table_name = _oline_return::TABLE_LABEL;
        let op = _oline_return::InMemDStoreFilterTimeRangeOp {
            t0: start,
            t1: end,
            oid: Some(oid),
        };
        let pkeys = self
            .datastore
            .filter_keys(table_name.to_string(), &op)
            .await?;
        let info = HashMap::from([(table_name.to_string(), pkeys)]);
        let mut data = self.datastore.fetch(info).await?;
        let rows = data.remove(table_name).unwrap();
        let out = rows
            .into_values()
            .map(|row| _oline_return::inmem_filt_qty_detail(row.into(), start, end))
            .collect();
        Ok(out)
    }
    async fn create(
        &self,
        oid: &str,
        reqs: Vec<OrderReturnModel>,
    ) -> DefaultResult<usize, AppError> {
        let qty_empty = reqs.iter().find_map(|req| {
            if req.qty.is_empty() {
                let detail = format!(
                    "return-req, in-mem-repo, prod-id: {} {}",
                    req.id_.store_id(),
                    req.id_.product_id()
                );
                Some(detail)
            } else {
                None
            }
        });
        if qty_empty.is_some() {
            return Err(AppError {
                code: AppErrorCode::EmptyInputData,
                detail: qty_empty,
            });
        }
        let table_name = _oline_return::TABLE_LABEL.to_string();
        let num_saved = reqs.iter().map(|r| r.qty.len()).sum();
        let mut info = vec![];
        for req in reqs {
            let pkey = _oline_return::inmem_pkey(
                oid,
                req.id_.store_id(),
                req.id_.product_id(),
                req.id_.attrs_seq_num(),
            );
            // load saved `qty` inner table
            let _info = HashMap::from([(table_name.clone(), vec![pkey.clone()])]);
            let mut _data = self.datastore.fetch(_info).await?;
            let rows = _data.remove(table_name.as_str()).unwrap();
            let rows = rows.into_values().collect::<Vec<_>>();
            assert!([0usize, 1].contains(&rows.len()));
            let serial_saved_qty = if let Some(row) = rows.first() {
                let v = row
                    .get::<usize>(_oline_return::InMemColIdx::QtyRefund.into())
                    .unwrap()
                    .to_owned();
                Some(v)
            } else {
                None
            };
            let m_serial = AppInMemFetchedSingleRow::from(InsertOpArg(req, serial_saved_qty));
            let item = (pkey, m_serial);
            info.push(item);
        } // end of loop
        let rows = HashMap::from_iter(info);
        let data = HashMap::from([(table_name, rows)]);
        let _num_saved_ds = self.datastore.save(data).await?;
        Ok(num_saved)
    } // end of fn create
} // end of OrderReturnInMemRepo

impl OrderReturnInMemRepo {
    pub async fn new(m: Arc<Box<dyn AbstInMemoryDStore>>) -> DefaultResult<Self, AppError> {
        m.create_table(_oline_return::TABLE_LABEL).await?;
        Ok(Self {
            datastore: m.clone(),
        })
    }
}
