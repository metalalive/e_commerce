use std::collections::HashMap;
use std::sync::Arc;
use std::vec::Vec;
use std::result::Result as DefaultResult;

use async_trait::async_trait;
use chrono::{DateTime, FixedOffset, NaiveDateTime};
use sqlx::{Executor, MySql, Statement, Row, Acquire, Arguments, IntoArguments, Transaction};
use sqlx::mysql::{MySqlArguments, MySqlRow};

use crate::constant::{limit, ProductType};
use crate::datastore::AppMariaDbStore;
use crate::error::{AppError, AppErrorCode};
use crate::model::{OrderLineIdentity, OrderReturnModel, OrderLinePriceModel};
use crate::repository::AbsOrderReturnRepo;

use super::{OidBytes, run_query_once};

struct InsertReqArg(OidBytes, u16, Vec<OrderReturnModel>);
struct FetchByIdArg(OidBytes, Vec<OrderLineIdentity>);

struct ReturnsPerOrder(Vec<OrderReturnModel>);

impl InsertReqArg {
    fn sql_pattern(num_batch:usize) -> String {
        let col_seq = "`o_id`,`seq`,`store_id`,`product_type`,`product_id`,\
            `create_time`,`quantity`,`price_unit`,`price_total`";
        let items = (0..num_batch).into_iter().map(
            |_| "(?,?,?,?,?,?,?,?,?)"
        ).collect::<Vec<_>>();
        format!("INSERT INTO `oline_return_req`({col_seq}) VALUES {}",
                items.join(","))
    }
}
impl<'q> IntoArguments<'q, MySql> for InsertReqArg {
    fn into_arguments(self) -> <MySql as sqlx::database::HasArguments<'q>>::Arguments
    {
        let (oid_b, mut seq_start, reqs) = (self.0, self.1, self.2);
        let oid = oid_b.as_column();
        let mut args = MySqlArguments::default();
        reqs.into_iter().map(|req| {
            let (id_, qty_map) = (req.id_, req.qty);
            let (seller_id, prod_typ, prod_id) = (id_.store_id, id_.product_type, id_.product_id);
            let prod_typ_num:u8 = prod_typ.into();
            qty_map.into_iter().map(|(ctime, (qty, refund))| {
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
            }).count();
        }).count();
        args
    }
}
impl Into<(String, MySqlArguments)> for InsertReqArg
{
    fn into(self) -> (String, MySqlArguments) {
        let num_batch = self.2.iter().map(|r| r.qty.len()).sum();
        (Self::sql_pattern(num_batch), self.into_arguments())
    }
}

impl FetchByIdArg {
    fn sql_pattern(num_batch:usize) -> String {
        let col_seq = "`store_id`,`product_type`,`product_id`,\
            `create_time`,`quantity`,`price_unit`,`price_total`";
        let items = (0..num_batch).into_iter().map(
            |_| "(`store_id`=? AND `product_type`=? AND `product_id`=?)"
        ).collect::<Vec<_>>();
        format!("SELECT {col_seq} FROM `oline_return_req` WHERE `o_id`=? AND ({})",
                 items.join("OR"))
    }
}
impl<'q> IntoArguments<'q, MySql> for  FetchByIdArg {
    fn into_arguments(self) -> <MySql as sqlx::database::HasArguments<'q>>::Arguments
    {
        let (oid_b, pids) = (self.0, self.1);
        let mut args = MySqlArguments::default();
        args.add(oid_b.as_column());
        pids.into_iter().map(|id_| {
            let (seller_id, prod_typ, prod_id) = (id_.store_id, id_.product_type, id_.product_id);
            let prod_typ_num:u8 = prod_typ.into();
            args.add(seller_id);
            args.add(prod_typ_num.to_string());
            args.add(prod_id);
        }).count();
        args
    }
}
impl Into<(String, MySqlArguments)> for  FetchByIdArg {
    fn into(self) -> (String, MySqlArguments) {
        let num_batch = self.1.len();
        (Self::sql_pattern(num_batch), self.into_arguments())
    }
}

impl ReturnsPerOrder {
    fn new() -> Self { Self(vec![]) }
    fn try_merge(&mut self, row: MySqlRow) -> DefaultResult<(), AppError>
    {
        let store_id = row.try_get::<u32,usize>(0)?;
        let product_type = row.try_get::<&str,usize>(1)?.parse::<ProductType>()?;
        let product_id   = row.try_get::<u64,usize>(2)?;
        let id_ = OrderLineIdentity { store_id, product_type, product_id };
        let result = self.0.iter_mut().find(|ret| ret.id_ == id_);
        let saved_ret = if let Some(v) = result {
            v
        } else {
            let item = OrderReturnModel {id_, qty:HashMap::new()};
            self.0.push(item);
            self.0.last_mut().unwrap()
        };
        let create_time = row.try_get::<NaiveDateTime,usize>(3)?.and_utc().into();
        let quantity = row.try_get::<u32,usize>(4)?;
        let unit = row.try_get::<u32,usize>(5)?;
        let total = row.try_get::<u32,usize>(6)?;
        let refund = OrderLinePriceModel { unit, total };
        saved_ret.qty.insert(create_time, (quantity, refund));
        Ok(())
    }
}
impl Into<Vec<OrderReturnModel>> for ReturnsPerOrder
{
    fn into(self) -> Vec<OrderReturnModel> { self.0 }
}

pub(crate) struct OrderReturnMariaDbRepo {
    _db : Arc<AppMariaDbStore>,
}

#[async_trait]
impl AbsOrderReturnRepo for OrderReturnMariaDbRepo {
    async fn fetch_by_pid(&self, oid:&str, pids:Vec<OrderLineIdentity>)
        -> DefaultResult<Vec<OrderReturnModel>, AppError>
    {
        if pids.is_empty() {
            Ok(vec![])
        } else {
            let oid_b = OidBytes::try_from(oid)?;
            let mut conn = self._db.acquire().await?;
            let (sql_patt, args) = FetchByIdArg(oid_b, pids).into();
            let stmt = conn.prepare(sql_patt.as_str()).await?;
            let query = stmt.query_with(args);
            let exec = conn.as_mut();
            let rows:Vec<_> = exec.fetch_all(query).await?;
            let mut rets = ReturnsPerOrder::new();
            let errors = rows.into_iter().filter_map(|row| {
                if let Err(e) = rets.try_merge(row) {
                    Some(e)
                } else { None }
            }).collect::<Vec<_>>();
            if let Some(e) = errors.first() {
               Err(e.to_owned()) 
            } else { Ok(rets.into()) }
        }
    }
    async fn fetch_by_created_time(&self, start: DateTime<FixedOffset>, end: DateTime<FixedOffset>)
        -> DefaultResult<Vec<(String, OrderReturnModel)>, AppError>
    {
        Err(AppError { code: AppErrorCode::NotImplemented, detail: None })
    }
    async fn fetch_by_oid_ctime(&self, oid:&str, start: DateTime<FixedOffset>, end: DateTime<FixedOffset>)
        -> DefaultResult<Vec<OrderReturnModel>, AppError>
    {
        Err(AppError { code: AppErrorCode::NotImplemented, detail: None })
    }
    async fn create(&self, oid:&str, reqs:Vec<OrderReturnModel>) -> DefaultResult<usize, AppError>
    {
        let oid_b = OidBytes::try_from(oid)?;
        let num_batch = reqs.iter().map(|r| r.qty.len()).sum();
        let mut conn = self._db.acquire().await?;
        let mut tx = conn.begin().await?;
        let num_returns = Self::get_num_reqs(&mut tx, &oid_b).await?;
        let (sql_patt, args) = InsertReqArg(oid_b, num_returns, reqs).into();
        let _rs = run_query_once(&mut tx, sql_patt, args, num_batch).await ?;
        tx.commit().await?;
        Ok(num_batch)
    }
} // end of impl AbsOrderReturnRepo

impl OrderReturnMariaDbRepo {
    pub(crate) async fn new(dbs:Vec<Arc<AppMariaDbStore>>) -> DefaultResult<Self, AppError>
    {
        if dbs.is_empty() {
            Err(AppError { code: AppErrorCode::MissingDataStore,
                detail: Some(format!("mariadb"))  })
        } else {
            let _db = dbs.first().unwrap().clone();
            Ok(Self { _db })
        }
    }
    async fn get_num_reqs(tx:&mut Transaction<'_,MySql>, oid_b:&OidBytes)
        -> DefaultResult<u16, AppError>
    {
        let sql_patt = "SELECT COUNT(`seq`) FROM `oline_return_req` WHERE `o_id`=? GROUP BY `o_id`";
        let stmt = tx.prepare(sql_patt).await?;
        let query = stmt.query().bind(oid_b.as_column());
        let exec = &mut *tx;
        let result = exec.fetch_optional(query).await?;
        if let Some(row) = result {
            let num_returns = row.try_get::<i64, usize>(0)?;
            let req_limit = limit::MAX_ORDER_LINES_PER_REQUEST.try_into().unwrap();
            if num_returns < req_limit {
                Ok(num_returns as u16)
            } else {
                let oid = String::from_utf8(oid_b.as_column()).unwrap();
                let detail = Some(format!("oid:{}, seq:{}", oid, num_returns));
                Err(AppError {code:AppErrorCode::ExceedingMaxLimit, detail })
            }
        } else { Ok(0) } 
    }
} // end of impl OrderReturnMariaDbRepo
