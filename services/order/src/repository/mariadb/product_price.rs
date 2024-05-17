use std::cmp::min;
use std::collections::HashMap;
use std::result::Result as DefaultResult;
use std::sync::Arc;

use async_trait::async_trait;
use chrono::{FixedOffset, NaiveDateTime};
use sqlx::mysql::{MySqlArguments, MySqlRow};
use sqlx::{Acquire, Arguments, Executor, IntoArguments, MySql, Row, Statement, Transaction};

use ecommerce_common::constant::ProductType;
use ecommerce_common::error::AppErrorCode;
use ecommerce_common::model::BaseProductIdentity;

use crate::api::rpc::dto::ProductPriceDeleteDto;
use crate::datastore::AppMariaDbStore;
use crate::error::AppError;
use crate::model::{ProductPriceModel, ProductPriceModelSet};
use crate::repository::AbsProductPriceRepo;

use super::DATETIME_FORMAT;

struct InsertArg(u32, Vec<ProductPriceModel>);
struct UpdateArg(u32, Vec<ProductPriceModel>);
struct FetchOneArg(u32, Vec<(ProductType, u64)>);
struct FetchManyArg(Vec<BaseProductIdentity>);
struct DeleteSomeArg(u32, ProductPriceDeleteDto);
struct DeleteAllArg(u32);

impl InsertArg {
    fn sql_pattern(num_batch: usize) -> String {
        const ITEM: &str = "(?,?,?,?,?,?,?,?)";
        const DELIMITER: &str = ",";
        let items = (0..num_batch).map(|_| ITEM).collect::<Vec<_>>();
        format!("INSERT INTO `product_price`(`store_id`,`product_type`,`product_id`,`price`,`start_after`,`end_before`, `start_tz_utc`, `end_tz_utc`) VALUES {}"
                , items.join(DELIMITER) )
    }
}
impl<'q> IntoArguments<'q, MySql> for InsertArg {
    fn into_arguments(self) -> <MySql as sqlx::database::HasArguments<'q>>::Arguments {
        let mut out = MySqlArguments::default();
        let (store_id, items) = (self.0, self.1);
        items
            .into_iter()
            .map(|item| {
                let (p_id, p_typ, price, start_after, end_before) = (
                    item.product_id,
                    item.product_type,
                    item.price,
                    item.start_after,
                    item.end_before,
                );
                let prod_typ_num: u8 = p_typ.into();
                let tz = start_after.fixed_offset().timezone();
                let start_tz_utc = tz.local_minus_utc() / 60;
                let tz = end_before.fixed_offset().timezone();
                let end_tz_utc = tz.local_minus_utc() / 60;
                out.add(store_id);
                out.add(prod_typ_num.to_string());
                out.add(p_id);
                out.add(price);
                out.add(format!("{}", start_after.format(DATETIME_FORMAT)));
                out.add(format!("{}", end_before.format(DATETIME_FORMAT)));
                out.add(start_tz_utc as i16);
                out.add(end_tz_utc as i16);
            })
            .count();
        out
    }
} // impl IntoArguments for InsertArg
impl From<InsertArg> for (String, MySqlArguments) {
    fn from(value: InsertArg) -> (String, MySqlArguments) {
        (
            InsertArg::sql_pattern(value.1.len()),
            value.into_arguments(),
        )
    }
}

impl UpdateArg {
    fn sql_pattern(num_batch: usize) -> String {
        let case_ops = (0..num_batch)
            .map(|_| "WHEN (`product_type`=? AND `product_id`=?) THEN ? ")
            .collect::<Vec<_>>()
            .join("");
        let pid_cmps = (0..num_batch)
            .map(|_| "(`product_type`=? AND `product_id`=?)")
            .collect::<Vec<_>>()
            .join(" OR ");
        format!("UPDATE `product_price` SET `price` = CASE {} ELSE `price` END, `start_after` = CASE {} ELSE `start_after` END, `end_before` = CASE {} ELSE `end_before` END, `start_tz_utc` = CASE {} ELSE `start_tz_utc` END, `end_tz_utc` = CASE {} ELSE `end_tz_utc` END  WHERE store_id = ? AND ({})" 
                , case_ops, case_ops, case_ops, case_ops, case_ops, pid_cmps)
    }
}
impl<'q> IntoArguments<'q, MySql> for UpdateArg {
    fn into_arguments(self) -> <MySql as sqlx::database::HasArguments<'q>>::Arguments {
        let mut out = MySqlArguments::default();
        let (store_id, items) = (self.0, self.1);
        items
            .iter()
            .map(|item| {
                let (p_id, p_typ, price) = (item.product_id, item.product_type.clone(), item.price);
                let prod_typ_num: u8 = p_typ.into();
                out.add(prod_typ_num.to_string());
                out.add(p_id);
                out.add(price);
            })
            .count();
        items
            .iter()
            .map(|item| {
                let (p_id, p_typ, start_after) =
                    (item.product_id, item.product_type.clone(), item.start_after);
                let prod_typ_num: u8 = p_typ.into();
                out.add(prod_typ_num.to_string());
                out.add(p_id);
                out.add(start_after.format(DATETIME_FORMAT).to_string());
            })
            .count();
        items
            .iter()
            .map(|item| {
                let (p_id, p_typ, end_before) =
                    (item.product_id, item.product_type.clone(), item.end_before);
                let prod_typ_num: u8 = p_typ.into();
                out.add(prod_typ_num.to_string());
                out.add(p_id);
                out.add(end_before.format(DATETIME_FORMAT).to_string());
            })
            .count();
        items
            .iter()
            .map(|item| {
                let (p_id, p_typ, start_after) =
                    (item.product_id, item.product_type.clone(), item.start_after);
                let prod_typ_num: u8 = p_typ.into();
                let start_tz_utc = start_after.fixed_offset().timezone().local_minus_utc() / 60;
                out.add(prod_typ_num.to_string());
                out.add(p_id);
                out.add(start_tz_utc as i16);
            })
            .count();
        items
            .iter()
            .map(|item| {
                let (p_id, p_typ, end_before) =
                    (item.product_id, item.product_type.clone(), item.end_before);
                let prod_typ_num: u8 = p_typ.into();
                let end_tz_utc = end_before.fixed_offset().timezone().local_minus_utc() / 60;
                out.add(prod_typ_num.to_string());
                out.add(p_id);
                out.add(end_tz_utc as i16);
            })
            .count();
        out.add(store_id);
        items
            .into_iter()
            .map(|item| {
                let (p_id, p_typ) = (item.product_id, item.product_type);
                let prod_typ_num: u8 = p_typ.into();
                out.add(prod_typ_num.to_string());
                out.add(p_id);
            })
            .count();
        out
    } // end of fn into_arguments
} // end of impl IntoArguments for UpdateArg
impl From<UpdateArg> for (String, MySqlArguments) {
    fn from(value: UpdateArg) -> (String, MySqlArguments) {
        (
            UpdateArg::sql_pattern(value.1.len()),
            value.into_arguments(),
        )
    }
}

impl FetchOneArg {
    fn sql_pattern(num_batch: usize) -> String {
        let col_seq = "`store_id`,`product_type`,`product_id`,`price`,`start_after`,\
                       `end_before`,`start_tz_utc`,`end_tz_utc`";
        let pid_cmps = (0..num_batch)
            .map(|_| "(`product_type`=? AND `product_id`=?)")
            .collect::<Vec<_>>()
            .join("OR");
        format!(
            "SELECT {} FROM `product_price` WHERE `store_id`=? AND ({})",
            col_seq, pid_cmps
        )
    }
}
impl<'q> IntoArguments<'q, MySql> for FetchOneArg {
    fn into_arguments(self) -> <MySql as sqlx::database::HasArguments<'q>>::Arguments {
        let mut out = MySqlArguments::default();
        let (store_id, items) = (self.0, self.1);
        out.add(store_id);
        items
            .into_iter()
            .map(|(product_type, product_id)| {
                let prod_typ_num: u8 = product_type.into();
                out.add(prod_typ_num.to_string());
                out.add(product_id);
            })
            .count();
        out
    }
}
impl From<FetchOneArg> for (String, MySqlArguments) {
    fn from(value: FetchOneArg) -> (String, MySqlArguments) {
        let num_batch = value.1.len();
        assert!(num_batch > 0);
        (FetchOneArg::sql_pattern(num_batch), value.into_arguments())
    }
}

impl FetchManyArg {
    fn sql_pattern(num_batch: usize) -> String {
        let col_seq = "`store_id`,`product_type`,`product_id`,`price`,`start_after`,\
                       `end_before`,`start_tz_utc`,`end_tz_utc`";
        let pid_cmps = (0..num_batch)
            .map(|_| "(`store_id`=? AND `product_type`=? AND `product_id`=?)")
            .collect::<Vec<_>>()
            .join(" OR ");
        format!("SELECT {col_seq} FROM `product_price` WHERE {}", pid_cmps)
    }
}
impl<'q> IntoArguments<'q, MySql> for FetchManyArg {
    fn into_arguments(self) -> <MySql as sqlx::database::HasArguments<'q>>::Arguments {
        let mut out = MySqlArguments::default();
        let pids = self.0;
        pids.into_iter()
            .map(|id_| {
                let (store_id, prod_type, prod_id) =
                    (id_.store_id, id_.product_type, id_.product_id);
                let prod_typ_num: u8 = prod_type.into();
                out.add(store_id);
                out.add(prod_typ_num.to_string());
                out.add(prod_id);
            })
            .count();
        out
    }
}
impl From<FetchManyArg> for (String, MySqlArguments) {
    fn from(value: FetchManyArg) -> (String, MySqlArguments) {
        let num_batch = value.0.len();
        assert!(num_batch > 0);
        (FetchManyArg::sql_pattern(num_batch), value.into_arguments())
    }
}

impl DeleteSomeArg {
    fn sql_pattern(num_items: usize, num_pkgs: usize) -> String {
        let items_ph = (0..num_items).map(|_| "?").collect::<Vec<_>>().join(",");
        let pkgs_ph = (0..num_pkgs).map(|_| "?").collect::<Vec<_>>().join(",");
        format!("DELETE FROM `product_price` WHERE `store_id`=? AND ((`product_type`=? AND `product_id` IN ({})) OR (`product_type`=? AND `product_id` IN ({}))  )", items_ph, pkgs_ph)
    }
}
impl<'q> IntoArguments<'q, MySql> for DeleteSomeArg {
    fn into_arguments(self) -> <MySql as sqlx::database::HasArguments<'q>>::Arguments {
        let mut out = MySqlArguments::default();
        let (store_id, data) = (self.0, self.1);
        out.add(store_id);
        let item_typ: u8 = data.item_type.into();
        out.add(item_typ.to_string());
        data.items
            .unwrap()
            .iter()
            .map(|product_id| {
                out.add(*product_id);
            })
            .count();
        let pkg_typ: u8 = data.pkg_type.into();
        out.add(pkg_typ.to_string());
        data.pkgs
            .unwrap()
            .iter()
            .map(|product_id| {
                out.add(*product_id);
            })
            .count();
        out
    }
}
impl TryInto<(String, MySqlArguments)> for DeleteSomeArg {
    type Error = AppError;

    fn try_into(self) -> DefaultResult<(String, MySqlArguments), Self::Error> {
        let empty = vec![];
        let items_r = self.1.items.as_ref().unwrap_or(&empty);
        let pkgs_r = self.1.pkgs.as_ref().unwrap_or(&empty);
        if items_r.is_empty() && pkgs_r.is_empty() {
            Err(AppError {
                code: AppErrorCode::EmptyInputData,
                detail: Some("delete-product-price".to_string()),
            })
        } else {
            Ok((
                Self::sql_pattern(items_r.len(), pkgs_r.len()),
                self.into_arguments(),
            ))
        }
    }
}

impl From<DeleteAllArg> for (String, MySqlArguments) {
    fn from(value: DeleteAllArg) -> (String, MySqlArguments) {
        let sql_patt = "DELETE FROM `product_price` WHERE `store_id`=?";
        let mut args = MySqlArguments::default();
        let store_id = value.0;
        args.add(store_id);
        (sql_patt.to_string(), args)
    }
}

impl TryFrom<MySqlRow> for ProductPriceModel {
    type Error = AppError;
    fn try_from(value: MySqlRow) -> DefaultResult<Self, Self::Error> {
        let product_type = value.try_get::<&str, usize>(1)?.parse::<ProductType>()?;
        let product_id = value.try_get::<u64, usize>(2)?;
        let price = value.try_get::<u32, usize>(3)?;
        let start_after = value.try_get::<NaiveDateTime, usize>(4)?;
        let end_before = value.try_get::<NaiveDateTime, usize>(5)?;
        let start_tz_utc = value.try_get::<i16, usize>(6)?;
        let end_tz_utc = value.try_get::<i16, usize>(7)?;
        //let start_after_naive = start_after.clone();
        let start_after = {
            let num_secs = (start_tz_utc as i32) * 60;
            let tz = FixedOffset::east_opt(num_secs).unwrap();
            start_after.and_local_timezone(tz).unwrap()
        };
        let end_before = {
            let num_secs = (end_tz_utc as i32) * 60;
            let tz = FixedOffset::east_opt(num_secs).unwrap();
            // Do NOT use DateTime::from_naive_utc_and_offset()
            end_before.and_local_timezone(tz).unwrap()
        };
        //println!("[DEBUG] product-id : {}, start_after naive: {:?}, final:{:?}",
        //        product_id, start_after_naive, start_after);
        Ok(Self {
            product_type,
            product_id,
            price,
            start_after,
            end_before,
            is_create: false,
        })
    } // end of fn try-from
} // end of impl try-from for ProductPriceModel

impl TryFrom<Vec<MySqlRow>> for ProductPriceModelSet {
    type Error = AppError;
    fn try_from(value: Vec<MySqlRow>) -> DefaultResult<Self, Self::Error> {
        if value.is_empty() {
            Ok(Self {
                store_id: 0,
                items: vec![],
            })
        } else {
            let mut errors = vec![];
            let first_row = value.first().unwrap();
            let store_id = first_row.try_get::<u32, usize>(0)?;
            let items = value
                .into_iter()
                .map(|v| {
                    let store_id_dup = v.try_get::<u32, usize>(0)?;
                    if store_id_dup == store_id {
                        Ok(v)
                    } else {
                        let detail =
                            format!("inconsistency, store-id: {}, {}", store_id, store_id_dup);
                        Err(AppError {
                            code: AppErrorCode::DataCorruption,
                            detail: Some(detail),
                        })
                    }
                })
                .filter_map(|v| {
                    let result = match v {
                        Ok(row) => ProductPriceModel::try_from(row),
                        Err(e) => Err(e),
                    };
                    match result {
                        Ok(m) => Some(m),
                        Err(e) => {
                            errors.push(e);
                            None
                        }
                    }
                })
                .collect::<Vec<_>>();
            if errors.is_empty() {
                Ok(Self { store_id, items })
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
        }
    } // end of fn try-from
} // end of impl try-from for ProductPriceModelSet

pub struct ProductPriceMariaDbRepo {
    db: Arc<AppMariaDbStore>,
}
impl ProductPriceMariaDbRepo {
    pub fn new(dbs: &Vec<Arc<AppMariaDbStore>>) -> DefaultResult<Self, AppError> {
        if dbs.is_empty() {
            let e = AppError {
                code: AppErrorCode::MissingDataStore,
                detail: Some("mariadb".to_string()),
            };
            Err(e)
        } else {
            // TODO, currently this repo always grabs the first db pool,
            // will need to figure out how to balance loading when the app data grows
            let db = dbs.first().unwrap().clone();
            Ok(Self { db })
        }
    }

    async fn _save(
        store_id: u32,
        cmd: &str,
        limit: usize,
        tx: &mut Transaction<'_, MySql>,
        mut prices: Vec<ProductPriceModel>,
    ) -> DefaultResult<(), AppError> {
        while !prices.is_empty() {
            let num_batch = min(prices.len(), limit);
            let expect_num_affected = num_batch;
            let items_processing = prices.split_off(prices.len() - num_batch);
            assert!(!items_processing.is_empty());
            let (sql_patt, args) = if cmd == "insert" {
                InsertArg(store_id, items_processing).into()
            } else {
                // update
                UpdateArg(store_id, items_processing).into()
            };
            let stmt = tx.prepare(sql_patt.as_str()).await?;
            let exec = &mut **tx;
            let query = stmt.query_with(args);
            let resultset = query.execute(exec).await?;
            let num_affected = resultset.rows_affected() as usize;
            if num_affected != expect_num_affected {
                let detail = format!(
                    "num_affected, actual:{}, expect:{}",
                    num_affected, expect_num_affected
                );
                return Err(AppError {
                    code: AppErrorCode::DataCorruption,
                    detail: Some(detail),
                });
            }
        } // end of loop
        Ok(())
    } // end of fn _save
    async fn _fetch_common(
        &self,
        sql_patt: String,
        args: MySqlArguments,
    ) -> DefaultResult<Vec<MySqlRow>, AppError> {
        let mut conn = self.db.acquire().await?;
        let stmt = conn.prepare(sql_patt.as_str()).await?;
        let query = stmt.query_with(args);
        let exec = conn.as_mut();
        let rows = query.fetch_all(exec).await?;
        Ok(rows)
    }
    async fn _delete_common(
        &self,
        sql_patt: String,
        args: MySqlArguments,
    ) -> DefaultResult<(), AppError> {
        let mut conn = self.db.acquire().await?;
        let stmt = conn.prepare(sql_patt.as_str()).await?;
        let query = stmt.query_with(args);
        let exec = conn.as_mut();
        let _resultset = query.execute(exec).await?;
        Ok(()) // TODO, logging result
    }
} // end of impl ProductPriceMariaDbRepo

#[async_trait]
impl AbsProductPriceRepo for ProductPriceMariaDbRepo {
    async fn delete_all(&self, store_id: u32) -> DefaultResult<(), AppError> {
        let (sql_patt, args) = DeleteAllArg(store_id).into();
        self._delete_common(sql_patt, args).await?;
        Ok(())
    }
    async fn delete(
        &self,
        store_id: u32,
        ids: ProductPriceDeleteDto,
    ) -> DefaultResult<(), AppError> {
        let (sql_patt, args) = DeleteSomeArg(store_id, ids).try_into()?;
        self._delete_common(sql_patt, args).await?;
        Ok(())
    }
    async fn fetch(
        &self,
        store_id: u32,
        ids: Vec<(ProductType, u64)>,
    ) -> DefaultResult<ProductPriceModelSet, AppError> {
        let out = if ids.is_empty() {
            ProductPriceModelSet {
                store_id,
                items: vec![],
            }
        } else {
            let (sql_patt, args) = FetchOneArg(store_id, ids).into();
            let rows = self._fetch_common(sql_patt, args).await?;
            let mut o = ProductPriceModelSet::try_from(rows)?;
            if o.store_id == 0 && o.items.is_empty() {
                o.store_id = store_id;
            }
            o
        };
        Ok(out)
    }
    async fn fetch_many(
        &self,
        ids: Vec<(u32, ProductType, u64)>,
    ) -> DefaultResult<Vec<ProductPriceModelSet>, AppError> {
        if ids.is_empty() {
            return Ok(vec![]);
        }
        let ids = ids
            .into_iter()
            .map(|(store_id, product_type, product_id)| BaseProductIdentity {
                store_id,
                product_type,
                product_id,
            })
            .collect::<Vec<_>>();
        let (sql_patt, args) = FetchManyArg(ids).into();
        let rows = self._fetch_common(sql_patt, args).await?;
        let mut errors: Vec<AppError> = Vec::new();
        let mut map: HashMap<u32, ProductPriceModelSet> = HashMap::new();
        let num_fetched = rows.len();
        let num_decoded = rows
            .into_iter()
            .map(|row| {
                let store_id = row.try_get::<u32, usize>(0)?;
                let m = ProductPriceModel::try_from(row)?;
                Ok((store_id, m))
            })
            .filter_map(|r| match r {
                Ok(v) => Some(v),
                Err(e) => {
                    errors.push(e);
                    None
                }
            })
            .map(|(store_id, m)| {
                if let Some(mset) = map.get_mut(&store_id) {
                    mset.items.push(m);
                } else {
                    let mset = ProductPriceModelSet {
                        store_id,
                        items: vec![m],
                    };
                    let old = map.insert(store_id, mset);
                    assert!(old.is_none());
                }
            })
            .count();
        if errors.is_empty() {
            assert_eq!(num_fetched, num_decoded);
            let out = map.into_values().collect::<Vec<_>>();
            Ok(out)
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
    } // end of fn fetch_many
    async fn save(&self, mset: ProductPriceModelSet) -> DefaultResult<(), AppError> {
        let (store_id, items) = (mset.store_id, mset.items);
        let (mut prices_add, mut prices_modify) = (vec![], vec![]);
        items
            .into_iter()
            .map(|p| {
                if p.is_create {
                    prices_add.push(p);
                } else {
                    prices_modify.push(p)
                }
            })
            .count(); // TODO, swtich to feature `drain-filter` when it becomes stable
        let mut conn = self.db.acquire().await?;
        let mut tx = conn.begin().await?;
        Self::_save(store_id, "update", 16, &mut tx, prices_modify).await?;
        Self::_save(store_id, "insert", 8, &mut tx, prices_add).await?;
        tx.commit().await?;
        Ok(())
    }
} // end of impl ProductPriceMariaDbRepo
