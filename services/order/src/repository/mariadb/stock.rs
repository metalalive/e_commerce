use std::cmp::min;
use std::collections::HashMap;
use std::ops::DerefMut;
use std::sync::Arc;
use std::vec::Vec;
use std::result::Result as DefaultResult;

use async_trait::async_trait;
use chrono::{DateTime, FixedOffset, Utc};
use sqlx::{Connection, Transaction, MySql, IntoArguments, Arguments, Executor, Statement, Row};
use sqlx::mysql::{MySqlArguments, MySqlRow};
use sqlx::database::HasArguments;

use crate::api::rpc::dto::{StockLevelReturnDto, StockReturnErrorDto};
use crate::constant::ProductType;
use crate::datastore::AppMariaDbStore;
use crate::error::{AppError, AppErrorCode};
use crate::model::{
    ProductStockIdentity, StockLevelModelSet, OrderLineModelSet, ProductStockModel,
    StoreStockModel, StockQuantityModel
};
use crate::repository::{
    AbsOrderStockRepo, AppStockRepoReserveUserFunc, AppStockRepoReserveReturn, AppStockRepoReturnUserFunc
};

struct InsertQtyArg(Vec<(u32, ProductStockModel)>);
struct UpdateQtyArg(Vec<(u32, ProductStockModel)>);
struct FetchBaseQtyArg(Vec<ProductStockIdentity>);
struct FetchBaseQtyRows(Vec<MySqlRow>);
// TODO, add FetchReservedArg, ReserveArg and ReturnArg

impl InsertQtyArg {
    fn sql_pattern(num_batch : usize) -> String
    {
        let col_seq = "`store_id`,`product_type`,`product_id`,`expiry`,`qty_total`,`qty_cancelled`";
        let items = (0 .. num_batch).into_iter().map(
            |_| "(?,?,?,?,?,?)"
        ).collect::<Vec<_>>();
        format!("INSERT INTO `stock_level_inventory`({}) VALUES {}",
            col_seq, items.join(","))
    }
}
impl<'q> IntoArguments<'q, MySql> for InsertQtyArg
{
    fn into_arguments(self) -> <MySql as HasArguments<'q>>::Arguments {
        let mut out = MySqlArguments::default();
        self.0.into_iter().map(|(store_id, p)| {
            let (expiry, p_typ, prod_id, q_total, q_cancelled) = (
                p.expiry_without_millis(), p.type_, p.id_,
                p.quantity.total, p.quantity.cancelled
            );
            let prod_typ_num:u8 = p_typ.into();
            out.add(store_id);
            out.add(prod_typ_num.to_string());
            out.add(prod_id);
            out.add(expiry);
            out.add(q_total);
            out.add(q_cancelled);
        }).count();
        out
    }
}
impl Into<(String, MySqlArguments)> for InsertQtyArg {
    fn into(self) -> (String, MySqlArguments) {
        (Self::sql_pattern(self.0.len()), self.into_arguments())
    }
}

impl UpdateQtyArg {
    fn sql_pattern(num_batch : usize) -> String {
        let condition = "(`store_id`=? AND `product_type`=? AND `product_id`=? AND `expiry`=?)";
        let case_ops = (0 .. num_batch).into_iter().map(
            |_| ["WHEN", condition, "THEN", "?"]
        ).flatten().collect::<Vec<_>>().join(" ");
        let pid_cmps = (0 .. num_batch).into_iter().map(
            |_|  condition
        ).collect::<Vec<_>>().join("OR");
        let portions = [
            format!("`qty_total` = CASE {case_ops} ELSE `qty_total` END"),
            format!("`qty_cancelled` = CASE {case_ops} ELSE `qty_cancelled` END"),
        ];
        format!("UPDATE `stock_level_inventory` SET {},{} WHERE {}",
            portions[0], portions[1], pid_cmps)
    }
}
impl<'q> IntoArguments<'q, MySql> for UpdateQtyArg
{
    fn into_arguments(self) -> <MySql as HasArguments<'q>>::Arguments {
        let mut out = MySqlArguments::default();
        self.0.iter().map(|(store_id, p)| {
            let (p_typ, prod_id, expiry, q_total) = ( p.type_.clone(), p.id_,
                        p.expiry_without_millis(), p.quantity.total);
            let prod_typ_num:u8 = p_typ.into();
            out.add(store_id);
            out.add(prod_typ_num.to_string());
            out.add(prod_id);
            out.add(expiry);
            out.add(q_total);
        }).count();
        self.0.iter().map(|(store_id, p)| {
            let (p_typ, prod_id, expiry, q_cancelled) = ( p.type_.clone(), p.id_,
                p.expiry_without_millis(), p.quantity.cancelled);
            let prod_typ_num:u8 = p_typ.into();
            out.add(store_id);
            out.add(prod_typ_num.to_string());
            out.add(prod_id);
            out.add(expiry);
            out.add(q_cancelled);
        }).count();
        self.0.into_iter().map(|(store_id, p)| {
            let (expiry, p_typ, prod_id) = (p.expiry_without_millis(), p.type_, p.id_);
            let prod_typ_num:u8 = p_typ.into();
            out.add(store_id);
            out.add(prod_typ_num.to_string());
            out.add(prod_id);
            out.add(expiry);
        }).count();
        out
    }
} // end of impl IntoArguments for UpdateQtyArg
impl Into<(String, MySqlArguments)> for UpdateQtyArg {
    fn into(self) -> (String, MySqlArguments) {
        (Self::sql_pattern(self.0.len()), self.into_arguments())
    }
}

impl FetchBaseQtyArg {
    fn sql_pattern(num_batch : usize) -> String {
        let condition = "(`store_id`=? AND `product_type`=? AND `product_id`=? AND `expiry`=?)";
        let pid_cmps = (0 .. num_batch).into_iter().map(
            |_|  condition
        ).collect::<Vec<_>>();
        let col_seq = "`store_id`,`product_type`,`product_id`,`expiry`,`qty_total`,`qty_cancelled`"; 
        format!("SELECT {col_seq} FROM `stock_level_inventory` WHERE {}", pid_cmps.join("OR"))
    }
}
impl<'q> IntoArguments<'q, MySql> for FetchBaseQtyArg
{
    fn into_arguments(self) -> <MySql as HasArguments<'q>>::Arguments {
        let mut out = MySqlArguments::default();
        self.0.into_iter().map(|co| {
            let (store_id, p_typ, prod_id, expiry) = (co.store_id, co.product_type,
                                                      co.product_id, co.expiry );
            let prod_typ_num:u8 = p_typ.into();
            out.add(store_id);
            out.add(prod_typ_num.to_string());
            out.add(prod_id);
            out.add(expiry);
        }).count();
        out
    }
}
impl Into<(String, MySqlArguments)> for FetchBaseQtyArg {
    fn into(self) -> (String, MySqlArguments) {
        (Self::sql_pattern(self.0.len()), self.into_arguments())
    }
}

impl TryFrom <MySqlRow> for ProductStockModel {
    type Error = AppError;
    fn try_from(row: MySqlRow) -> DefaultResult<Self, Self::Error>
    {
        let prod_typ = row.try_get::<&str, usize>(1)?
            .parse::<ProductType>() ?;
        let prod_id = row.try_get::<u64, usize>(2) ?;
        let expiry  = row.try_get::<DateTime<Utc>, usize>(3)? .into();
        let total     = row.try_get::<u32, usize>(4)?;
        let cancelled = row.try_get::<u32, usize>(5)?;
        // TODO, options for importing reservation records
        let quantity = StockQuantityModel::new(total, cancelled, 0, None) ;
        Ok(Self { type_: prod_typ, id_: prod_id, expiry, quantity, is_create: false })
    }
}
impl TryFrom<FetchBaseQtyRows> for StockLevelModelSet {
    type Error = AppError;

    fn try_from(value: FetchBaseQtyRows) -> DefaultResult<Self, Self::Error> {
        let mut errors:Vec<AppError> = Vec::new();
        let mut map: HashMap<u32, StoreStockModel> = HashMap::new();
        let num_fetched = value.0.len();
        let num_decoded = value.0.into_iter().map(|row| {
            let store_id = row.try_get::<u32, usize>(0)?;
            let v = ProductStockModel::try_from(row)?;
            Ok((store_id, v))
        }).filter_map(|r| match r {
            Ok(v) => Some(v),
            Err(e) => {errors.push(e); None}
        }).map(|(store_id, m)| {
            if !map.contains_key(&store_id) {
                let s = StoreStockModel { store_id, products: vec![] };
                let _ = map.insert(store_id, s);
            }
            let store = map.get_mut(&store_id).unwrap();
            store.products.push(m);
        }).count() ;
        if errors.is_empty() {
            assert_eq!(num_fetched, num_decoded);
            let stores = map.into_values().collect();
            Ok(StockLevelModelSet {stores})
        } else {
            let detail = errors.into_iter().map(|e| e.to_string())
                .collect::<Vec<_>>().join(", ");
            Err(AppError { code: AppErrorCode::DataCorruption, detail: Some(detail) })
        }
    } // end of fn try_from
} // end of impl TryFrom<FetchBaseQtyRows> for StockLevelModelSet


pub(super) struct StockMariaDbRepo
{
    _time_now : DateTime<FixedOffset>,
    _db : Arc<AppMariaDbStore>,
}

#[async_trait]
impl AbsOrderStockRepo for StockMariaDbRepo
{
    async fn fetch(&self, pids:Vec<ProductStockIdentity>) -> DefaultResult<StockLevelModelSet, AppError>
    {
        let (sql_patt, args) = FetchBaseQtyArg(pids).into();
        let mut conn = self._db.acquire().await ?;
        let stmt = conn.prepare(sql_patt.as_str()).await ?;
        let query = stmt.query_with(args);
        let exec = conn.as_mut();
        let rows = query.fetch_all(exec).await ?;
        let msets = StockLevelModelSet::try_from(FetchBaseQtyRows(rows))?;
        Ok(msets)
    }
    async fn save(&self, slset:StockLevelModelSet) -> DefaultResult<(), AppError>
    { // Note, the difference from `save()` in-memory repository is that this
      // function does not save reservation records.
        let (mut stk_add, mut stk_modify) = (vec![], vec![]);
        slset.stores.into_iter().map(|s| {
            let (store_id, products) = (s.store_id, s.products);
            products.into_iter().map(|p| {
                let chosen = if p.is_create { &mut stk_add }
                          else { &mut stk_modify };
                chosen.push((store_id, p));
            }).count();
        }).count();
        let mut conn = self._db.acquire().await?;
        let mut tx = conn.begin().await ?;
        Self::_save_base_qty("update", 16, &mut tx, stk_modify).await ?;
        Self::_save_base_qty("insert", 32, &mut tx, stk_add).await ?;
        tx.commit().await?;
        Ok(())
    }
    async fn try_reserve(&self, _cb: AppStockRepoReserveUserFunc,
                         _order_req: &OrderLineModelSet) -> AppStockRepoReserveReturn
    { // TODO, figure out how to send `sqlx` transaction object between tasks
        let e = AppError { code: AppErrorCode::NotImplemented, detail: None };
        Err(Err(e))
    }
    async fn try_return(&self,  _cb: AppStockRepoReturnUserFunc,
                        _data: StockLevelReturnDto )
        -> DefaultResult<Vec<StockReturnErrorDto>, AppError>
    {
        Err(AppError { code: AppErrorCode::NotImplemented, detail: None })
    }
} // end of impl AbsOrderStockRepo for StockMariaDbRepo


impl StockMariaDbRepo {
    pub(crate) fn new (time_now: DateTime<FixedOffset>, _db: Arc<AppMariaDbStore>)
        -> Self
    { Self { _time_now: time_now, _db } }

    async fn _save_base_qty(
        cmd:&str, limit:usize, tx: &mut Transaction<'_, MySql>,
        mut data : Vec<(u32, ProductStockModel)>
    ) -> DefaultResult<(), AppError>
    {
        while !data.is_empty() {
            let num_batch = min(data.len(), limit);
            let items_processing = data.split_off(data.len() - num_batch);
            assert!(items_processing.len() > 0);
            let (sql_patt, args) = if cmd == "insert" {
                InsertQtyArg(items_processing).into()
            } else { UpdateQtyArg(items_processing).into() };
            let stmt = tx.prepare(sql_patt.as_str()).await?;
            let query = stmt.query_with(args);
            let exec = tx.deref_mut();
            let resultset = query.execute(exec).await?;
            let num_affected = resultset.rows_affected() as usize;
            if num_affected != num_batch {
                let detail = format!("num_affected, actual:{}, expect:{}",
                                     num_affected, num_batch );
                return Err(AppError { code: AppErrorCode::DataCorruption,
                    detail: Some(detail) });
            }
        } // end of loop
        Ok(())
    } // end of fn _save_base_qty
} // end of impl StockMariaDbRepo
