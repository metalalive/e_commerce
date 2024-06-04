use std::collections::HashMap;
use std::result::Result as DefaultResult;
use std::sync::Arc;
use std::vec::Vec;

use async_trait::async_trait;
use chrono::{DateTime, FixedOffset, NaiveDateTime};
use sqlx::mysql::{MySqlArguments, MySqlRow};
use sqlx::{Acquire, Arguments, Executor, IntoArguments, MySql, Row, Statement, Transaction};

use ecommerce_common::adapter::repository::OidBytes;
use ecommerce_common::constant::ProductType;
use ecommerce_common::error::AppErrorCode;

use crate::constant::hard_limit;
use crate::datastore::AppMariaDbStore;
use crate::error::AppError;
use crate::model::{OrderLineIdentity, OrderLinePriceModel, OrderReturnModel};
use crate::repository::AbsOrderReturnRepo;

use super::{run_query_once, to_app_oid};

struct InsertReqArg(OidBytes, u16, Vec<OrderReturnModel>);
struct FetchByIdArg(OidBytes, Vec<OrderLineIdentity>);
struct FetchByTimeArg(DateTime<FixedOffset>, DateTime<FixedOffset>);
struct FetchByIdAndTimeArg(OidBytes, DateTime<FixedOffset>, DateTime<FixedOffset>);

struct ReturnsPerOrder(Vec<OrderReturnModel>);
struct ReturnOidMap {
    rows: Vec<MySqlRow>,
    _map: HashMap<String, ReturnsPerOrder>,
}

impl InsertReqArg {
    fn sql_pattern(num_batch: usize) -> String {
        let col_seq = "`o_id`,`seq`,`store_id`,`product_type`,`product_id`,\
            `create_time`,`quantity`,`price_unit`,`price_total`";
        let items = (0..num_batch)
            .map(|_| "(?,?,?,?,?,?,?,?,?)")
            .collect::<Vec<_>>();
        format!(
            "INSERT INTO `oline_return_req`({col_seq}) VALUES {}",
            items.join(",")
        )
    }
}
impl<'q> IntoArguments<'q, MySql> for InsertReqArg {
    fn into_arguments(self) -> <MySql as sqlx::database::HasArguments<'q>>::Arguments {
        let (oid_b, mut seq_start, reqs) = (self.0, self.1, self.2);
        let oid = oid_b.as_column();
        let mut args = MySqlArguments::default();
        reqs.into_iter()
            .map(|req| {
                let (id_, qty_map) = (req.id_, req.qty);
                let (seller_id, prod_typ, prod_id) =
                    (id_.store_id, id_.product_type, id_.product_id);
                let prod_typ_num: u8 = prod_typ.into();
                qty_map
                    .into_iter()
                    .map(|(ctime, (qty, refund))| {
                        args.add(oid.clone());
                        args.add(seq_start);
                        args.add(seller_id);
                        args.add(prod_typ_num.to_string());
                        args.add(prod_id);
                        args.add(ctime.naive_utc());
                        args.add(qty);
                        args.add(refund.unit);
                        args.add(refund.total);
                        seq_start += 1;
                    })
                    .count();
            })
            .count();
        args
    }
}
impl From<InsertReqArg> for (String, MySqlArguments) {
    fn from(value: InsertReqArg) -> (String, MySqlArguments) {
        let num_batch = value.2.iter().map(|r| r.qty.len()).sum();
        (InsertReqArg::sql_pattern(num_batch), value.into_arguments())
    }
}

const COLUMN_SEQ_SELECT: &str = "`store_id`,`product_type`,`product_id`,\
            `create_time`,`quantity`,`price_unit`,`price_total`";

impl FetchByIdArg {
    fn sql_pattern(num_batch: usize) -> String {
        let items = (0..num_batch)
            .map(|_| "(`store_id`=? AND `product_type`=? AND `product_id`=?)")
            .collect::<Vec<_>>();
        format!(
            "SELECT {COLUMN_SEQ_SELECT} FROM `oline_return_req` WHERE `o_id`=? AND ({})",
            items.join("OR")
        )
    }
}
impl<'q> IntoArguments<'q, MySql> for FetchByIdArg {
    fn into_arguments(self) -> <MySql as sqlx::database::HasArguments<'q>>::Arguments {
        let (oid_b, pids) = (self.0, self.1);
        let mut args = MySqlArguments::default();
        args.add(oid_b.as_column());
        pids.into_iter()
            .map(|id_| {
                let (seller_id, prod_typ, prod_id) =
                    (id_.store_id, id_.product_type, id_.product_id);
                let prod_typ_num: u8 = prod_typ.into();
                args.add(seller_id);
                args.add(prod_typ_num.to_string());
                args.add(prod_id);
            })
            .count();
        args
    }
}
impl From<FetchByIdArg> for (String, MySqlArguments) {
    fn from(value: FetchByIdArg) -> (String, MySqlArguments) {
        let num_batch = value.1.len();
        (FetchByIdArg::sql_pattern(num_batch), value.into_arguments())
    }
}

impl From<FetchByTimeArg> for (String, MySqlArguments) {
    fn from(value: FetchByTimeArg) -> (String, MySqlArguments) {
        let (start, end) = (value.0, value.1);
        // TODO, improve query time, since the execution plan will not search in
        // primary index. Possible approach could be time-series database or
        // secondary index by `create-time`, since the time range argument in the
        // fetch method is used for querying recently added returns.
        let sql_patt = format!(
            "SELECT {COLUMN_SEQ_SELECT},`o_id` FROM `oline_return_req` \
                                WHERE `create_time` > ? AND `create_time` <= ?"
        );
        let mut args = MySqlArguments::default();
        args.add(start.naive_utc());
        args.add(end.naive_utc());
        (sql_patt, args)
    }
}

impl From<FetchByIdAndTimeArg> for (String, MySqlArguments) {
    fn from(value: FetchByIdAndTimeArg) -> (String, MySqlArguments) {
        let (oid_b, start, end) = (value.0, value.1, value.2);
        let sql_patt = format!(
            "SELECT {COLUMN_SEQ_SELECT} FROM `oline_return_req` \
                                WHERE `o_id`=? AND `create_time` > ? AND `create_time` <= ?"
        );
        let mut args = MySqlArguments::default();
        args.add(oid_b.as_column());
        args.add(start.naive_utc());
        args.add(end.naive_utc());
        (sql_patt, args)
    }
}

impl ReturnsPerOrder {
    fn new() -> Self {
        Self(vec![])
    }
    fn try_merge(&mut self, row: MySqlRow) -> DefaultResult<(), AppError> {
        let store_id = row.try_get::<u32, usize>(0)?;
        let product_type = row.try_get::<&str, usize>(1)?.parse::<ProductType>()?;
        let product_id = row.try_get::<u64, usize>(2)?;
        let id_ = OrderLineIdentity {
            store_id,
            product_type,
            product_id,
        };
        let result = self.0.iter_mut().find(|ret| ret.id_ == id_);
        let saved_ret = if let Some(v) = result {
            v
        } else {
            let item = OrderReturnModel {
                id_,
                qty: HashMap::new(),
            };
            self.0.push(item);
            self.0.last_mut().unwrap()
        };
        let create_time = row.try_get::<NaiveDateTime, usize>(3)?.and_utc().into();
        let quantity = row.try_get::<u32, usize>(4)?;
        let unit = row.try_get::<u32, usize>(5)?;
        let total = row.try_get::<u32, usize>(6)?;
        let refund = OrderLinePriceModel { unit, total };
        saved_ret.qty.insert(create_time, (quantity, refund));
        Ok(())
    }
}
impl From<ReturnsPerOrder> for Vec<OrderReturnModel> {
    fn from(value: ReturnsPerOrder) -> Vec<OrderReturnModel> {
        value.0
    }
}

impl ReturnOidMap {
    fn new(rows: Vec<MySqlRow>) -> Self {
        Self {
            rows,
            _map: HashMap::new(),
        }
    }
}
impl TryInto<Vec<(String, OrderReturnModel)>> for ReturnOidMap {
    type Error = AppError;
    fn try_into(self) -> DefaultResult<Vec<(String, OrderReturnModel)>, Self::Error> {
        let (mut ret_map, rows) = (self._map, self.rows);
        let has_error = rows
            .into_iter()
            .map(|row| {
                let oid = to_app_oid(&row, 7)?;
                if !ret_map.contains_key(oid.as_str()) {
                    ret_map.insert(oid.clone(), ReturnsPerOrder::new());
                }
                let entry = ret_map.get_mut(oid.as_str()).unwrap();
                entry.try_merge(row)?;
                Ok(())
            })
            .find_map(|r| match r {
                Ok(_) => None,
                Err(e) => Some(e),
            });
        if let Some(e) = has_error {
            return Err(e);
        }
        let out = ret_map
            .into_iter()
            .map(|(oid, inner_rets)| {
                let o_rets: Vec<OrderReturnModel> = inner_rets.into();
                (oid, o_rets)
            })
            .flat_map(|(oid, o_rets)| o_rets.into_iter().map(move |ret| (oid.clone(), ret)))
            .collect::<Vec<_>>();
        Ok(out)
    }
}

pub(crate) struct OrderReturnMariaDbRepo {
    _db: Arc<AppMariaDbStore>,
}

#[async_trait]
impl AbsOrderReturnRepo for OrderReturnMariaDbRepo {
    async fn fetch_by_pid(
        &self,
        oid: &str,
        pids: Vec<OrderLineIdentity>,
    ) -> DefaultResult<Vec<OrderReturnModel>, AppError> {
        if pids.is_empty() {
            Ok(vec![])
        } else {
            let oid_b = OidBytes::try_from(oid)?;
            let (sql_patt, args) = FetchByIdArg(oid_b, pids).into();
            self.fetch_by_oid_common(sql_patt, args).await
        }
    }
    async fn fetch_by_created_time(
        &self,
        start: DateTime<FixedOffset>,
        end: DateTime<FixedOffset>,
    ) -> DefaultResult<Vec<(String, OrderReturnModel)>, AppError> {
        let mut conn = self._db.acquire().await?;
        let (sql_patt, args) = FetchByTimeArg(start, end).into();
        let stmt = conn.prepare(sql_patt.as_str()).await?;
        let query = stmt.query_with(args);
        let exec = &mut *conn;
        let rows = exec.fetch_all(query).await?;
        ReturnOidMap::new(rows).try_into()
    }
    async fn fetch_by_oid_ctime(
        &self,
        oid: &str,
        start: DateTime<FixedOffset>,
        end: DateTime<FixedOffset>,
    ) -> DefaultResult<Vec<OrderReturnModel>, AppError> {
        let oid_b = OidBytes::try_from(oid)?;
        let (sql_patt, args) = FetchByIdAndTimeArg(oid_b, start, end).into();
        self.fetch_by_oid_common(sql_patt, args).await
    }
    async fn create(
        &self,
        oid: &str,
        reqs: Vec<OrderReturnModel>,
    ) -> DefaultResult<usize, AppError> {
        let oid_b = OidBytes::try_from(oid)?;
        let num_batch = reqs.iter().map(|r| r.qty.len()).sum();
        let mut conn = self._db.acquire().await?;
        let mut tx = conn.begin().await?;
        let num_returns = Self::get_num_reqs(&mut tx, &oid_b).await?;
        let (sql_patt, args) = InsertReqArg(oid_b, num_returns, reqs).into();
        let _rs = run_query_once(&mut tx, sql_patt, args, Some(num_batch)).await?;
        tx.commit().await?;
        Ok(num_batch)
    }
} // end of impl AbsOrderReturnRepo

impl OrderReturnMariaDbRepo {
    pub(crate) async fn new(dbs: Vec<Arc<AppMariaDbStore>>) -> DefaultResult<Self, AppError> {
        if dbs.is_empty() {
            Err(AppError {
                code: AppErrorCode::MissingDataStore,
                detail: Some("mariadb".to_string()),
            })
        } else {
            let _db = dbs.first().unwrap().clone();
            Ok(Self { _db })
        }
    }
    async fn get_num_reqs(
        tx: &mut Transaction<'_, MySql>,
        oid_b: &OidBytes,
    ) -> DefaultResult<u16, AppError> {
        let sql_patt = "SELECT COUNT(`seq`) FROM `oline_return_req` WHERE `o_id`=? GROUP BY `o_id`";
        let stmt = tx.prepare(sql_patt).await?;
        let query = stmt.query().bind(oid_b.as_column());
        let exec = &mut *tx;
        let result = exec.fetch_optional(query).await?;
        if let Some(row) = result {
            let num_returns = row.try_get::<i64, usize>(0)?;
            let req_limit = hard_limit::MAX_ORDER_LINES_PER_REQUEST.try_into().unwrap();
            if num_returns < req_limit {
                Ok(num_returns as u16)
            } else {
                let oid = String::from_utf8(oid_b.as_column()).unwrap();
                let detail = Some(format!("oid:{}, seq:{}", oid, num_returns));
                Err(AppError {
                    code: AppErrorCode::ExceedingMaxLimit,
                    detail,
                })
            }
        } else {
            Ok(0)
        }
    }
    async fn fetch_by_oid_common(
        &self,
        sql_patt: String,
        args: MySqlArguments,
    ) -> DefaultResult<Vec<OrderReturnModel>, AppError> {
        let mut conn = self._db.acquire().await?;
        let stmt = conn.prepare(sql_patt.as_str()).await?;
        let query = stmt.query_with(args);
        let exec = conn.as_mut();
        let rows: Vec<_> = exec.fetch_all(query).await?;
        let mut rets = ReturnsPerOrder::new();
        let maybe_error = rows
            .into_iter()
            .map(|row| rets.try_merge(row))
            .find_map(|r| if let Err(e) = r { Some(e) } else { None });
        if let Some(e) = maybe_error {
            Err(e)
        } else {
            Ok(rets.into())
        }
    }
} // end of impl OrderReturnMariaDbRepo
