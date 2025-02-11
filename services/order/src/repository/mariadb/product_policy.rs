use std::cmp::min;
use std::result::Result as DefaultResult;
use std::sync::Arc;
use std::vec::Vec;

use async_trait::async_trait;
use sqlx::mysql::{MySqlArguments, MySqlRow};
use sqlx::{Acquire, Arguments, Executor, MySql, Row, Statement, Transaction};

use ecommerce_common::error::AppErrorCode;

use crate::datastore::AppMariaDbStore;
use crate::error::AppError;
use crate::model::{ProductPolicyModel, ProductPolicyModelSet};
use crate::repository::AbstProductPolicyRepo;

pub(crate) struct ProductPolicyMariaDbRepo {
    db: Arc<AppMariaDbStore>,
}

impl ProductPolicyMariaDbRepo {
    pub async fn new(dbs: &Vec<Arc<AppMariaDbStore>>) -> DefaultResult<Self, AppError> {
        if dbs.is_empty() {
            let e = AppError {
                code: AppErrorCode::MissingDataStore,
                detail: Some("mariadb".to_string()),
            };
            Err(e)
        } else {
            let db = dbs.first().unwrap().clone();
            Ok(Self { db })
        } // TODO, currently this repo always grabs the first db pool,
          // will need to figure out how to balance loading when the app data grows
    }

    fn prep_stmt_patt_read(sql_pattern_blocks: (&str, &str, &str), mut num_items: usize) -> String {
        assert!(num_items > 0);
        let (prefix, item, delimiter) = sql_pattern_blocks;
        let mut sql = prefix.to_string() + item;
        num_items -= 1;
        let num_done = (0..num_items)
            .map(|_| {
                sql += delimiter;
                sql += item;
            })
            .count();
        assert_eq!(num_done, num_items);
        sql
    }

    async fn _save(
        &self,
        tx: &mut Transaction<'_, MySql>,
        mut policies: Vec<ProductPolicyModel>,
        args_constructor: fn(Vec<ProductPolicyModel>) -> (String, MySqlArguments),
    ) -> DefaultResult<(), AppError> {
        let limit = 14_usize;
        let params = [];
        while !policies.is_empty() {
            let num_batch = min(policies.len(), limit);
            let expect_num_affected = num_batch;
            let policies_processing = policies.split_off(policies.len() - num_batch);
            assert!(!policies_processing.is_empty());
            let (sql_patt, args) = args_constructor(policies_processing);
            let stmt = tx.prepare_with(sql_patt.as_str(), &params).await?;
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

    fn construct_insert_args(items: Vec<ProductPolicyModel>) -> (String, MySqlArguments) {
        const SQL_PATTERN_BLOCKS: (&str, &str, &str) = (
            "INSERT INTO `product_policy`(`product_id`,`auto_cancel_secs`,\
             `warranty_hours`,`max_num_rsv`,`min_num_rsv`) VALUES ",
            "(?,?,?,?,?)",
            ",",
        );
        let mut args = MySqlArguments::default();
        let num_batch = items
            .into_iter()
            .map(|item| {
                let (prod_id, auto_cancel, warranty, max_rsv, min_rsv) = (
                    item.product_id,
                    item.auto_cancel_secs,
                    item.warranty_hours,
                    item.max_num_rsv,
                    item.min_num_rsv,
                );
                args.add(prod_id).unwrap();
                args.add(auto_cancel).unwrap();
                args.add(warranty).unwrap();
                args.add(max_rsv).unwrap();
                args.add(min_rsv).unwrap();
            })
            .count();
        let sql_patt = Self::prep_stmt_patt_read(SQL_PATTERN_BLOCKS, num_batch);
        (sql_patt, args)
    } // end of fn construct_insert_args

    fn construct_update_args(items: Vec<ProductPolicyModel>) -> (String, MySqlArguments) {
        let mut args = MySqlArguments::default();
        let mut num_batch = items
            .iter()
            .map(|item| {
                let (prod_id, auto_cancel) = (item.product_id, item.auto_cancel_secs);
                args.add(prod_id).unwrap();
                args.add(auto_cancel).unwrap();
            })
            .count();
        items
            .iter()
            .map(|item| {
                let (prod_id, warranty) = (item.product_id, item.warranty_hours);
                args.add(prod_id).unwrap();
                args.add(warranty).unwrap();
            })
            .count();
        items
            .iter()
            .map(|item| {
                let (prod_id, max_rsv) = (item.product_id, item.max_num_rsv);
                args.add(prod_id).unwrap();
                args.add(max_rsv).unwrap();
            })
            .count();
        items
            .iter()
            .map(|item| {
                let (prod_id, min_rsv) = (item.product_id, item.min_num_rsv);
                args.add(prod_id).unwrap();
                args.add(min_rsv).unwrap();
            })
            .count();
        items
            .iter()
            .map(|item| {
                let prod_id = item.product_id;
                args.add(prod_id).unwrap();
            })
            .count();
        let sql_patt = {
            let case_ops = (0..num_batch)
                .map(|_| "WHEN (`product_id`=?) THEN ? ")
                .collect::<Vec<_>>()
                .join("");
            let mut out = format!(
                "UPDATE `product_policy` SET \
                `auto_cancel_secs` = CASE {} ELSE `auto_cancel_secs` END,\
                `warranty_hours` = CASE {} ELSE `warranty_hours` END, \
                `max_num_rsv` = CASE {} ELSE `max_num_rsv` END, \
                `min_num_rsv` = CASE {} ELSE `min_num_rsv` END \
                WHERE ",
                case_ops, case_ops, case_ops, case_ops
            );
            out += "(`product_id`=?)";
            num_batch -= 1;
            (0..num_batch)
                .map(|_| {
                    out += "OR (`product_id`=?)";
                })
                .count();
            out
        };
        (sql_patt, args)
    } // end of fn construct_update_args
} // end of impl ProductPolicyMariaDbRepo

#[async_trait]
impl AbstProductPolicyRepo for ProductPolicyMariaDbRepo {
    async fn fetch(&self, ids: Vec<u64>) -> DefaultResult<ProductPolicyModelSet, AppError> {
        const SQL_PATTERN_BLOCKS: (&str, &str, &str) = (
            "SELECT `product_id`,`auto_cancel_secs`,`warranty_hours`,`max_num_rsv`,`min_num_rsv` FROM `product_policy` WHERE ",
            "(`product_id`=?)", "OR"
        );
        let (limit, mut num_iter) = (16_usize, 0usize);
        let mut _ids = ids;
        // Mysql/mariadb doesn't need to specify type parameters
        let params = [];
        let mut conn = self.db.acquire().await?;
        let num_batch = min(_ids.len(), limit);
        let mut sql = Self::prep_stmt_patt_read(SQL_PATTERN_BLOCKS, num_batch);
        let mut policies = vec![];
        while !_ids.is_empty() {
            let num_batch = min(_ids.len(), limit);
            sql = if num_batch == _ids.len() && num_iter > 0 {
                Self::prep_stmt_patt_read(SQL_PATTERN_BLOCKS, num_batch)
            } else {
                sql
            };
            // `sqlx` internally caches prepared statements that were successfully declared
            // for each connection of a mariaDB server, this app can call the method
            // `prepare_with(...)` several times without worrying about network latency
            let stmt = conn.prepare_with(sql.as_str(), &params).await?;
            let mut args = MySqlArguments::default();
            let _ = (0..num_batch)
                .map(|_| {
                    let prod_id = _ids.remove(0);
                    args.add(prod_id).unwrap();
                })
                .count();
            let exec = conn.as_mut();
            let query = stmt.query_with(args);
            let rows = query.fetch_all(exec).await?;
            let portions = rows
                .into_iter()
                .map(ProductPolicyModel::try_from)
                .collect::<Vec<_>>();
            let errors = portions
                .iter()
                .filter_map(|r| {
                    if let Err(e) = r.as_ref() {
                        Some(e.detail.as_ref().unwrap().to_string())
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>();
            if !errors.is_empty() {
                return Err(AppError {
                    detail: Some(errors.join(",")),
                    code: AppErrorCode::DataCorruption,
                });
            } // TODO, logging error
            policies.extend(portions.into_iter().filter_map(|r| r.ok()));
            num_iter += 1;
        } // end of loop
        Ok(ProductPolicyModelSet { policies })
    } // end of fn fetch

    async fn save(&self, ppset: ProductPolicyModelSet) -> DefaultResult<(), AppError> {
        let (mut policies_add, mut policies_modify) = (vec![], vec![]);
        ppset
            .policies
            .into_iter()
            .map(|p| {
                if p.is_create {
                    policies_add.push(p);
                } else {
                    policies_modify.push(p)
                }
            })
            .count(); // TODO, swtich to feature `drain-filter` when it becomes stable
        let mut conn = self.db.acquire().await?;
        let mut tx = conn.begin().await?;
        self._save(&mut tx, policies_modify, Self::construct_update_args)
            .await?;
        self._save(&mut tx, policies_add, Self::construct_insert_args)
            .await?;
        tx.commit().await?;
        Ok(())
    } // end of fn save
} // end of impl ProductPolicyMariaDbRepo

impl TryFrom<MySqlRow> for ProductPolicyModel {
    type Error = AppError;
    fn try_from(value: MySqlRow) -> DefaultResult<Self, Self::Error> {
        // note, the code here implicitly converts the error type received `sqlx::Error`
        // into the error type `AppError`, on immediately returning the error
        let product_id = value.try_get::<u64, usize>(0)?;
        let auto_cancel_secs = value.try_get::<u32, usize>(1)?;
        let warranty_hours = value.try_get::<u32, usize>(2)?;
        let max_num_rsv = value.try_get::<u16, usize>(3)?;
        let min_num_rsv = value.try_get::<u16, usize>(4)?;
        Ok(Self {
            is_create: false,
            product_id,
            auto_cancel_secs,
            warranty_hours,
            max_num_rsv,
            min_num_rsv,
        })
    } // end of fn try_from
} // end of impl ProductPolicyModel
