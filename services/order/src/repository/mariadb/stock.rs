use std::cmp::min;
use std::collections::HashMap;
use std::ops::DerefMut;
use std::result::Result as DefaultResult;
use std::sync::Arc;
use std::vec::Vec;

use async_trait::async_trait;
use chrono::{DateTime, FixedOffset, NaiveDateTime};
use sqlx::database::Database as AbstractDatabase;
use sqlx::mysql::{MySqlArguments, MySqlRow};
use sqlx::{Arguments, Connection, Executor, IntoArguments, MySql, Row, Statement, Transaction};

use ecommerce_common::adapter::repository::OidBytes;
use ecommerce_common::error::AppErrorCode;

use crate::api::rpc::dto::{InventoryEditStockLevelDto, StockLevelReturnDto, StockReturnErrorDto};
use crate::api::web::dto::OrderLineCreateErrorDto;
use crate::datastore::AppMariaDbStore;
use crate::error::AppError;
use crate::model::{
    OrderLineModel, OrderLineModelSet, ProductStockIdentity, ProductStockModel, StockLevelModelSet,
    StockQtyRsvModel, StockQuantityModel, StoreStockModel,
};
use crate::repository::{
    AbsOrderStockRepo, AppStockRepoReserveReturn, AppStockRepoReserveUserFunc,
    AppStockRepoReturnUserFunc,
};

use super::order::OrderMariaDbRepo;
use super::{run_query_once, to_app_oid};

struct InsertQtyArg(Vec<(u32, ProductStockModel)>);
struct UpdateQtyArg(Vec<(u32, ProductStockModel)>);
struct ReserveArg(Vec<(u32, ProductStockModel)>);
struct ReturnArg(Vec<(u32, ProductStockModel)>);

struct FetchQtyArg(Vec<ProductStockIdentity>);
struct FetchQtyForRsvArg<'a>(&'a [OrderLineModel]); // TODO, add current time for expiry filtering
struct FetchRsvOrderArg<'a>(OidBytes, &'a Vec<InventoryEditStockLevelDto>);

struct StkProdRows(Vec<MySqlRow>);
struct StkProdRow(MySqlRow);
struct StkRsvDetailRows(Vec<MySqlRow>);
struct StkRsvDetailRow(MySqlRow);

impl InsertQtyArg {
    fn sql_pattern(num_batch: usize) -> String {
        let col_seq = "`store_id`,`product_id`,`expiry`,`qty_total`,`qty_cancelled`";
        let items = (0..num_batch).map(|_| "(?,?,?,?,?)").collect::<Vec<_>>();
        format!(
            "INSERT INTO `stock_level_inventory`({}) VALUES {}",
            col_seq,
            items.join(",")
        )
    }
}
impl<'q> IntoArguments<'q, MySql> for InsertQtyArg {
    fn into_arguments(self) -> <MySql as AbstractDatabase>::Arguments<'q> {
        let mut out = MySqlArguments::default();
        self.0
            .into_iter()
            .map(|(store_id, p)| {
                let (expiry, prod_id, q_total, q_cancelled) = (
                    p.expiry_without_millis().naive_utc(),
                    p.id_,
                    p.quantity.total,
                    p.quantity.cancelled,
                );
                out.add(store_id).unwrap();
                out.add(prod_id).unwrap();
                out.add(expiry).unwrap();
                out.add(q_total).unwrap();
                out.add(q_cancelled).unwrap();
            })
            .count();
        out
    }
}
impl From<InsertQtyArg> for Vec<(String, MySqlArguments)> {
    fn from(value: InsertQtyArg) -> Vec<(String, MySqlArguments)> {
        let c = (
            InsertQtyArg::sql_pattern(value.0.len()),
            value.into_arguments(),
        );
        vec![c]
    }
}

impl UpdateQtyArg {
    fn sql_pattern(num_batch: usize) -> String {
        let condition = "(`store_id`=? AND `product_id`=? AND `expiry`=?)";
        let case_ops = (0..num_batch)
            .flat_map(|_| ["WHEN", condition, "THEN", "?"])
            .collect::<Vec<_>>()
            .join(" ");
        let pid_cmps = (0..num_batch)
            .map(|_| condition)
            .collect::<Vec<_>>()
            .join("OR");
        let portions = [
            format!("`qty_total` = CASE {case_ops} ELSE `qty_total` END"),
            format!("`qty_cancelled` = CASE {case_ops} ELSE `qty_cancelled` END"),
        ];
        format!(
            "UPDATE `stock_level_inventory` SET {},{} WHERE {}",
            portions[0], portions[1], pid_cmps
        )
    }
}
impl<'q> IntoArguments<'q, MySql> for UpdateQtyArg {
    fn into_arguments(self) -> <MySql as AbstractDatabase>::Arguments<'q> {
        let mut out = MySqlArguments::default();
        self.0
            .iter()
            .map(|(store_id, p)| {
                let (prod_id, expiry, q_total) = (
                    p.id_,
                    p.expiry_without_millis().naive_utc(),
                    p.quantity.total,
                );
                out.add(store_id).unwrap();
                out.add(prod_id).unwrap();
                out.add(expiry).unwrap();
                out.add(q_total).unwrap();
            })
            .count();
        self.0
            .iter()
            .map(|(store_id, p)| {
                let (prod_id, expiry, q_cancelled) = (
                    p.id_,
                    p.expiry_without_millis().naive_utc(),
                    p.quantity.cancelled,
                );
                out.add(store_id).unwrap();
                out.add(prod_id).unwrap();
                out.add(expiry).unwrap();
                out.add(q_cancelled).unwrap();
            })
            .count();
        self.0
            .into_iter()
            .map(|(store_id, p)| {
                let (expiry, prod_id) = (p.expiry_without_millis().naive_utc(), p.id_);
                out.add(store_id).unwrap();
                out.add(prod_id).unwrap();
                out.add(expiry).unwrap();
            })
            .count();
        out
    }
} // end of impl IntoArguments for UpdateQtyArg
impl From<UpdateQtyArg> for Vec<(String, MySqlArguments)> {
    fn from(value: UpdateQtyArg) -> Vec<(String, MySqlArguments)> {
        let c = (
            UpdateQtyArg::sql_pattern(value.0.len()),
            value.into_arguments(),
        );
        vec![c]
    }
}

impl ReserveArg {
    fn pattern_update_block(num_batch: usize) -> (String, String) {
        let condition = "(`store_id`=? AND `product_id`=? AND `expiry`=?)";
        let case_ops = (0..num_batch)
            .flat_map(|_| ["WHEN", condition, "THEN", "?"])
            .collect::<Vec<_>>()
            .join(" ");
        let pid_cmps = (0..num_batch)
            .map(|_| condition)
            .collect::<Vec<_>>()
            .join("OR");
        (case_ops, pid_cmps)
    }
    fn pattern_update_total_rsv(num_batch: usize) -> String {
        let (case_ops, pid_cmps) = Self::pattern_update_block(num_batch);
        format!(
            "UPDATE `stock_level_inventory` SET `qty_tot_rsv` = CASE {case_ops} \
                ELSE `qty_tot_rsv` END WHERE {pid_cmps}"
        )
    }
    fn pattern_add_order_rsv(num_batch: usize) -> String {
        let col_seq = "`store_id`,`product_id`,`expiry`,`order_id`,`qty_reserved`";
        let items = (0..num_batch).map(|_| "(?,?,?,?,?)").collect::<Vec<_>>();
        format!(
            "INSERT INTO `stock_rsv_detail`({col_seq}) VALUES {}",
            items.join(",")
        )
    }
    fn args_update_total_rsv(stores: &[(u32, ProductStockModel)]) -> MySqlArguments {
        let mut out = MySqlArguments::default();
        stores
            .iter()
            .map(|(store_id, p)| {
                let (prod_id, expiry, q_booked) = (
                    p.id_,
                    p.expiry_without_millis().naive_utc(),
                    p.quantity.booked,
                );
                out.add(store_id).unwrap();
                out.add(prod_id).unwrap();
                out.add(expiry).unwrap();
                out.add(q_booked).unwrap();
            })
            .count();
        stores
            .iter()
            .map(|(store_id, p)| {
                let (expiry, prod_id) = (p.expiry_without_millis().naive_utc(), p.id_);
                out.add(store_id).unwrap();
                out.add(prod_id).unwrap();
                out.add(expiry).unwrap();
            })
            .count();
        out
    }
    fn args_add_order_rsv(self) -> MySqlArguments {
        let mut out = MySqlArguments::default();
        self.0
            .into_iter()
            .map(|(store_id, p)| {
                let (expiry, prod_id, detail) = (
                    p.expiry_without_millis().naive_utc(),
                    p.id_,
                    p.quantity.rsv_detail.unwrap(),
                );
                out.add(store_id).unwrap();
                out.add(prod_id).unwrap();
                out.add(expiry).unwrap();
                let (oid, rsv_per_item) = (detail.oid, detail.reserved);
                // TODO, move to beginning of `reserve()`
                let oid_b = OidBytes::try_from(oid.as_str()).unwrap();
                out.add(oid_b.as_column()).unwrap();
                out.add(rsv_per_item).unwrap();
            })
            .count();
        out
    }
}
impl From<ReserveArg> for Vec<(String, MySqlArguments)> {
    fn from(value: ReserveArg) -> Vec<(String, MySqlArguments)> {
        let num_batch = value.0.len();
        vec![
            (
                ReserveArg::pattern_update_total_rsv(num_batch),
                ReserveArg::args_update_total_rsv(&value.0),
            ),
            (
                ReserveArg::pattern_add_order_rsv(num_batch),
                value.args_add_order_rsv(),
            ),
        ]
    }
}
impl ReturnArg {
    fn pattern_update_order_rsv(num_batch: usize) -> String {
        let (case_ops, pid_cmps) = ReserveArg::pattern_update_block(num_batch);
        format!(
            "UPDATE `stock_rsv_detail` SET `qty_reserved` = CASE {case_ops} \
                ELSE `qty_reserved` END WHERE `order_id`=? AND ({pid_cmps})"
        )
    }
    fn args_update_order_rsv(self) -> MySqlArguments {
        let oid_b = {
            let _product = &self.0.first().unwrap().1;
            let _rsv_detail = _product.quantity.rsv_detail.as_ref().unwrap();
            OidBytes::try_from(_rsv_detail.oid.as_str()).unwrap()
        }; // order-id must not be modified in app callback
        let mut out = MySqlArguments::default();
        self.0
            .iter()
            .map(|(store_id, p)| {
                let (expiry, prod_id) = (p.expiry_without_millis().naive_utc(), p.id_);
                let _rsv_detail = p.quantity.rsv_detail.as_ref().unwrap();
                let qty_rsv_o = _rsv_detail.reserved;
                out.add(store_id).unwrap();
                out.add(prod_id).unwrap();
                out.add(expiry).unwrap();
                out.add(qty_rsv_o).unwrap();
            })
            .count();
        out.add(oid_b.as_column()).unwrap();
        self.0
            .iter()
            .map(|(store_id, p)| {
                let (expiry, prod_id) = (p.expiry_without_millis().naive_utc(), p.id_);
                out.add(store_id).unwrap();
                out.add(prod_id).unwrap();
                out.add(expiry).unwrap();
            })
            .count();
        out
    }
}
impl From<ReturnArg> for Vec<(String, MySqlArguments)> {
    fn from(value: ReturnArg) -> Vec<(String, MySqlArguments)> {
        let num_batch = value.0.len();
        vec![
            (
                ReserveArg::pattern_update_total_rsv(num_batch),
                ReserveArg::args_update_total_rsv(&value.0),
            ),
            (
                ReturnArg::pattern_update_order_rsv(num_batch),
                value.args_update_order_rsv(),
            ),
        ]
    }
}

impl FetchQtyArg {
    fn sql_pattern(num_batch: usize) -> String {
        let condition = "(`store_id`=? AND `product_id`=? AND `expiry`=?)";
        let pid_cmps = (0..num_batch).map(|_| condition).collect::<Vec<_>>();
        let col_seq = "`store_id`,`product_id`,`expiry`,`qty_total`,`qty_cancelled`,`qty_tot_rsv`";
        format!(
            "SELECT {col_seq} FROM `stock_level_inventory` WHERE {}",
            pid_cmps.join("OR")
        )
    }
}
impl<'q> IntoArguments<'q, MySql> for FetchQtyArg {
    fn into_arguments(self) -> <MySql as AbstractDatabase>::Arguments<'q> {
        let mut out = MySqlArguments::default();
        self.0
            .into_iter()
            .map(|co| {
                let expiry = co.expiry_without_millis().naive_utc();
                let (store_id, prod_id) = (co.store_id, co.product_id);
                out.add(store_id).unwrap();
                out.add(prod_id).unwrap();
                out.add(expiry).unwrap();
            })
            .count();
        out
    }
}
impl From<FetchQtyArg> for (String, MySqlArguments) {
    fn from(value: FetchQtyArg) -> (String, MySqlArguments) {
        (
            FetchQtyArg::sql_pattern(value.0.len()),
            value.into_arguments(),
        )
    }
}

impl<'a> FetchQtyForRsvArg<'a> {
    fn sql_pattern(num_batch: usize) -> String {
        let condition = "(`store_id`=? AND `product_id`=?)";
        let pid_cmps = (0..num_batch).map(|_| condition).collect::<Vec<_>>();
        let col_seq = "`store_id`,`product_id`,`expiry`,`qty_total`, `qty_cancelled`,`qty_tot_rsv`";
        format!(
            "SELECT {col_seq} FROM `stock_level_inventory` WHERE {}",
            pid_cmps.join("OR")
        )
    }
}
impl<'a, 'q> IntoArguments<'q, MySql> for FetchQtyForRsvArg<'a> {
    fn into_arguments(self) -> <MySql as AbstractDatabase>::Arguments<'q> {
        let mut out = MySqlArguments::default();
        self.0
            .iter()
            .map(|o| {
                out.add(o.id().store_id).unwrap();
                out.add(o.id().product_id).unwrap();
            })
            .count();
        out
    }
}
impl<'a> From<FetchQtyForRsvArg<'a>> for (String, MySqlArguments) {
    fn from(value: FetchQtyForRsvArg<'a>) -> (String, MySqlArguments) {
        (
            FetchQtyForRsvArg::sql_pattern(value.0.len()),
            value.into_arguments(),
        )
    }
}

impl<'a> FetchRsvOrderArg<'a> {
    fn sql_pattern(num_batch: usize) -> String {
        let condition = "(`a`.`store_id`=? AND `a`.`product_id`=?)";
        let pid_cmps = (0..num_batch).map(|_| condition).collect::<Vec<_>>();
        let col_seq = "`a`.`store_id`,`a`.`product_id`,`a`.`expiry`,`a`.`order_id`,\
            `a`.`qty_reserved`,`b`.`qty_total`,`b`.`qty_cancelled`,`b`.`qty_tot_rsv`";
        format!(
            "SELECT {col_seq} FROM `stock_rsv_detail` AS `a` INNER JOIN \
            `stock_level_inventory` AS `b` ON (`a`.`store_id`=`b`.`store_id` AND \
            `a`.`product_id`=`b`.`product_id` AND `a`.`expiry`=`b`.`expiry`) \
             WHERE `a`.`order_id`=? AND ({})",
            pid_cmps.join("OR")
        )
    }
}
impl<'a, 'q> IntoArguments<'q, MySql> for FetchRsvOrderArg<'a> {
    fn into_arguments(self) -> <MySql as AbstractDatabase>::Arguments<'q> {
        let (oid_b, items) = (self.0, self.1);
        let mut out = MySqlArguments::default();
        out.add(oid_b.as_column()).unwrap();
        items
            .iter()
            .map(|o| {
                out.add(o.store_id).unwrap();
                out.add(o.product_id).unwrap();
            })
            .count();
        out
    }
}
impl<'a> From<FetchRsvOrderArg<'a>> for (String, MySqlArguments) {
    fn from(value: FetchRsvOrderArg<'a>) -> (String, MySqlArguments) {
        let num_batch = value.1.len();
        (
            FetchRsvOrderArg::sql_pattern(num_batch),
            value.into_arguments(),
        )
    }
}

macro_rules! rows_to_stklvl_mset {
    ($rows:expr, $convertor:ident) => {{
        let mut errors: Vec<AppError> = Vec::new();
        let mut map: HashMap<u32, StoreStockModel> = HashMap::new();
        let num_fetched = $rows.len();
        let num_decoded = $rows
            .into_iter()
            .map(|row| {
                let store_id = row.try_get::<u32, usize>(0)?;
                let v = $convertor(row).try_into()?;
                Ok((store_id, v))
            })
            .filter_map(|r| match r {
                Ok(v) => Some(v),
                Err(e) => {
                    errors.push(e);
                    None
                }
            })
            .map(|(store_id, m)| {
                if !map.contains_key(&store_id) {
                    let s = StoreStockModel {
                        store_id,
                        products: vec![],
                    };
                    let _ = map.insert(store_id, s);
                }
                let store = map.get_mut(&store_id).unwrap();
                store.products.push(m);
            })
            .count();
        if errors.is_empty() {
            assert_eq!(num_fetched, num_decoded);
            let stores = map.into_values().collect();
            Ok(StockLevelModelSet { stores })
        } else {
            let detail = errors
                .into_iter()
                .map(|e| e.to_string())
                .collect::<Vec<_>>()
                .join(", ");
            Err(AppError {
                code: AppErrorCode::DataCorruption,
                detail: Some(detail),
            })
        }
    }};
}

impl TryInto<ProductStockModel> for StkProdRow {
    type Error = AppError;
    fn try_into(self) -> DefaultResult<ProductStockModel, Self::Error> {
        let row = self.0;
        let prod_id = row.try_get::<u64, usize>(1)?;
        let expiry = row.try_get::<NaiveDateTime, usize>(2)?.and_utc();
        let total = row.try_get::<u32, usize>(3)?;
        let cancelled = row.try_get::<u32, usize>(4)?;
        let booked = row.try_get::<u32, usize>(5)?;
        // Note, the conversion does not include reservation detail
        let quantity = StockQuantityModel::new(total, cancelled, booked, None);
        Ok(ProductStockModel {
            id_: prod_id,
            expiry,
            quantity,
            is_create: false,
        })
    }
}
impl TryInto<StockLevelModelSet> for StkProdRows {
    type Error = AppError;
    fn try_into(self) -> DefaultResult<StockLevelModelSet, Self::Error> {
        rows_to_stklvl_mset!(self.0, StkProdRow)
    }
}

impl TryInto<ProductStockModel> for StkRsvDetailRow {
    type Error = AppError;
    fn try_into(self) -> DefaultResult<ProductStockModel, Self::Error> {
        let row = self.0;
        let prod_id = row.try_get::<u64, usize>(1)?;
        let expiry = row.try_get::<NaiveDateTime, usize>(2)?.and_utc();
        let rsv_detail = {
            let oid = to_app_oid(&row, 3)?;
            let qty_rsv_o = row.try_get::<u32, usize>(4)?;
            StockQtyRsvModel {
                oid,
                reserved: qty_rsv_o,
            }
        };
        let quantity = {
            // quantity summary along with reservation detail
            let total = row.try_get::<u32, usize>(5)?;
            let cancelled = row.try_get::<u32, usize>(6)?;
            let booked = row.try_get::<u32, usize>(7)?;
            StockQuantityModel::new(total, cancelled, booked, Some(rsv_detail))
        };
        Ok(ProductStockModel {
            id_: prod_id,
            expiry,
            quantity,
            is_create: false,
        })
    }
}
impl TryInto<StockLevelModelSet> for StkRsvDetailRows {
    type Error = AppError;
    fn try_into(self) -> DefaultResult<StockLevelModelSet, Self::Error> {
        rows_to_stklvl_mset!(self.0, StkRsvDetailRow)
    }
}

pub(super) struct StockMariaDbRepo {
    _time_now: DateTime<FixedOffset>,
    _db: Arc<AppMariaDbStore>,
}

#[async_trait]
impl AbsOrderStockRepo for StockMariaDbRepo {
    async fn fetch(
        &self,
        pids: Vec<ProductStockIdentity>,
    ) -> DefaultResult<StockLevelModelSet, AppError> {
        let (sql_patt, args) = FetchQtyArg(pids).into();
        let mut conn = self._db.acquire().await?;
        let stmt = conn.prepare(sql_patt.as_str()).await?;
        let query = stmt.query_with(args);
        let exec = conn.as_mut();
        let rows = query.fetch_all(exec).await?;
        let msets = StkProdRows(rows).try_into()?;
        Ok(msets)
    }
    async fn save(&self, slset: StockLevelModelSet) -> DefaultResult<(), AppError> {
        // Note, the difference from `save()` in-memory repository is that this
        // function does not save reservation records.
        let (mut stk_add, mut stk_modify) = (vec![], vec![]);
        slset
            .stores
            .into_iter()
            .map(|s| {
                let (store_id, products) = (s.store_id, s.products);
                products
                    .into_iter()
                    .map(|p| {
                        let chosen = if p.is_create {
                            &mut stk_add
                        } else {
                            &mut stk_modify
                        };
                        chosen.push((store_id, p));
                    })
                    .count();
            })
            .count();
        let mut conn = self._db.acquire().await?;
        let mut tx = conn.begin().await?;
        Self::_save_base_qty("update", 16, &mut tx, stk_modify).await?;
        Self::_save_base_qty("insert", 32, &mut tx, stk_add).await?;
        tx.commit().await?;
        Ok(())
    }

    async fn try_reserve(
        &self,
        cb: AppStockRepoReserveUserFunc,
        order_req: &OrderLineModelSet,
    ) -> AppStockRepoReserveReturn {
        // TODO, figure out how to send `sqlx` transaction object between tasks (hard)
        match self._try_reserve(cb, order_req).await {
            Ok(c) => {
                if c.is_empty() {
                    Ok(())
                } else {
                    Err(Ok(c))
                }
            }
            Err(e) => Err(Err(e)),
        }
    } // end of fn try_reserve

    async fn try_return(
        &self,
        cb: AppStockRepoReturnUserFunc,
        data: StockLevelReturnDto,
    ) -> DefaultResult<Vec<StockReturnErrorDto>, AppError> {
        let mut objconn = self._db.acquire().await?;
        let conn = objconn.as_mut();
        let mut tx = conn.begin().await?;
        let mut mset = {
            let oid_b = OidBytes::try_from(data.order_id.as_str())?;
            let (sql_patt, args) = FetchRsvOrderArg(oid_b, &data.items).into(); // omit expiry check
            let stmt = tx.prepare(sql_patt.as_str()).await?;
            let query = stmt.query_with(args);
            let exec = &mut *tx;
            let rows = exec.fetch_all(query).await?;
            StkRsvDetailRows(rows).try_into()?
        };
        let errors = cb(&mut mset, data);
        if errors.is_empty() {
            let stk = mset
                .stores
                .into_iter()
                .flat_map(|s| {
                    let store_id = s.store_id;
                    s.products.into_iter().map(move |p| (store_id, p))
                })
                .collect();
            Self::_save_base_qty("return", 20, &mut tx, stk).await?;
            tx.commit().await?;
        }
        Ok(errors)
    } // end of fn try_return
} // end of impl AbsOrderStockRepo for StockMariaDbRepo

impl StockMariaDbRepo {
    pub(crate) fn new(time_now: DateTime<FixedOffset>, _db: Arc<AppMariaDbStore>) -> Self {
        Self {
            _time_now: time_now,
            _db,
        }
    }

    async fn _save_base_qty(
        cmd: &str,
        limit: usize,
        tx: &mut Transaction<'_, MySql>,
        mut data: Vec<(u32, ProductStockModel)>,
    ) -> DefaultResult<(), AppError> {
        while !data.is_empty() {
            let num_batch = min(data.len(), limit);
            let items_processing = data.split_off(data.len() - num_batch);
            assert!(!items_processing.is_empty());
            let sqls: Vec<(String, MySqlArguments)> = match cmd {
                "insert" => InsertQtyArg(items_processing).into(),
                "update" => UpdateQtyArg(items_processing).into(),
                "reserve" => ReserveArg(items_processing).into(),
                "return" => ReturnArg(items_processing).into(),
                _others => {
                    vec![]
                }
            };
            for (sql_patt, args) in sqls {
                let _rs = run_query_once(tx, sql_patt, args, Some(num_batch)).await?;
            }
        } // end of loop
        Ok(())
    } // end of fn _save_base_qty

    async fn _try_reserve(
        &self,
        usr_cb: AppStockRepoReserveUserFunc,
        order_req: &OrderLineModelSet,
    ) -> DefaultResult<Vec<OrderLineCreateErrorDto>, AppError> {
        let mut conn = self._db.acquire().await?;
        let mut tx = conn.begin().await?;
        let mut mset = {
            let (sql_patt, args) = FetchQtyForRsvArg(order_req.lines()).into();
            let stmt = tx.prepare(sql_patt.as_str()).await?;
            let query = stmt.query_with(args);
            let exec = tx.deref_mut();
            let rows = exec.fetch_all(query).await?;
            StkProdRows(rows).try_into()?
        };
        if let Err(e) = usr_cb(&mut mset, order_req) {
            e
        } else {
            let stk = mset
                .stores
                .into_iter()
                .flat_map(|s| {
                    let store_id = s.store_id;
                    s.products.into_iter().filter_map(move |p| {
                        if p.quantity.rsv_detail.is_some() {
                            Some((store_id, p))
                        } else {
                            None
                        }
                    })
                })
                .collect();
            Self::_save_base_qty("reserve", 20, &mut tx, stk).await?;
            OrderMariaDbRepo::create_lines(&mut tx, order_req, 22).await?;
            tx.commit().await?;
            Ok(vec![])
        }
    } // end of fn _try_reserve
} // end of impl StockMariaDbRepo
