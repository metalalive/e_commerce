use std::cmp::min;
use std::collections::HashMap;
use std::result::Result as DefaultResult;
use std::sync::Arc;

use async_trait::async_trait;
use chrono::{FixedOffset, NaiveDateTime};
use sqlx::database::Database as AbstractDatabase;
use sqlx::mysql::{MySqlArguments, MySqlRow};
use sqlx::{Acquire, Arguments, Executor, IntoArguments, MySql, Row, Statement, Transaction};

use ecommerce_common::api::dto::CurrencyDto;
use ecommerce_common::error::AppErrorCode;
use ecommerce_common::model::BaseProductIdentity;

use crate::api::rpc::dto::ProductPriceDeleteDto;
use crate::datastore::AppMariaDbStore;
use crate::error::AppError;
use crate::model::{ProdAttriPriceModel, ProductPriceModel, ProductPriceModelSet};
use crate::repository::AbsProductPriceRepo;

use super::{run_query_once, DATETIME_FORMAT};

struct InsertProductArg(u32, Vec<ProductPriceModel>);
struct UpdateProductArg(u32, Vec<ProductPriceModel>);
struct InsertUpdateMetaArg(u32, CurrencyDto);
struct FetchProductOneSellerArg(u32, Vec<u64>);
struct FetchProductManySellersArg(Vec<BaseProductIdentity>);
struct FetchMetaOneSellerArg(u32);
struct FetchMetaManySellersArg(Vec<u32>);
struct DeleteSomeArg(u32, ProductPriceDeleteDto);
struct DeleteStoreAllProductsArg(u32);
struct DeleteStoreMetaArg(u32);

impl InsertProductArg {
    fn sql_pattern(num_batch: usize) -> String {
        const ITEM: &str = "(?,?,?,?,?,?,?,?,?)";
        const DELIMITER: &str = ",";
        let items = (0..num_batch).map(|_| ITEM).collect::<Vec<_>>();
        format!("INSERT INTO `product_price`(`store_id`,`product_id`,`price`,`start_after`,`end_before`, \
                 `attr_lastupdate`, `start_tz_utc`, `end_tz_utc`, `attr_map`) VALUES {}"
                , items.join(DELIMITER) )
    }
}
impl<'q> IntoArguments<'q, MySql> for InsertProductArg {
    fn into_arguments(self) -> <MySql as AbstractDatabase>::Arguments<'q> {
        let mut out = MySqlArguments::default();
        let Self(store_id, items) = self;
        items
            .into_iter()
            .map(|item| {
                let attrprices_serial = item.attrs_charge().serialize_map().unwrap();
                let (p_id, baseprice, ts, _) = item.into_parts();
                let [start_after, end_before, attr_lastupdate] = ts;
                let tz = start_after.fixed_offset().timezone();
                let start_tz_utc = tz.local_minus_utc() / 60;
                let tz = end_before.fixed_offset().timezone();
                let end_tz_utc = tz.local_minus_utc() / 60;
                out.add(store_id).unwrap();
                out.add(p_id).unwrap();
                out.add(baseprice).unwrap();
                let t0 = format!("{}", start_after.format(DATETIME_FORMAT));
                let t1 = format!("{}", end_before.format(DATETIME_FORMAT));
                let t2 = format!("{}", attr_lastupdate.to_utc().format(DATETIME_FORMAT));
                out.add(t0).unwrap();
                out.add(t1).unwrap();
                out.add(t2).unwrap();
                out.add(start_tz_utc as i16).unwrap();
                out.add(end_tz_utc as i16).unwrap();
                out.add(attrprices_serial).unwrap();
            })
            .count();
        out
    }
} // impl IntoArguments for InsertProductArg
impl From<InsertProductArg> for (String, MySqlArguments) {
    fn from(value: InsertProductArg) -> (String, MySqlArguments) {
        (
            InsertProductArg::sql_pattern(value.1.len()),
            value.into_arguments(),
        )
    }
}

impl UpdateProductArg {
    fn sql_pattern(num_batch: usize) -> String {
        let case_ops = (0..num_batch)
            .map(|_| "WHEN (`product_id`=?) THEN ? ")
            .collect::<Vec<_>>()
            .join("");
        let pid_cmps = (0..num_batch)
            .map(|_| "(`product_id`=?)")
            .collect::<Vec<_>>()
            .join(" OR ");
        format!(
            "UPDATE `product_price` SET `price` = CASE {} ELSE `price` END, \
            `start_after` = CASE {} ELSE `start_after` END, `end_before` = CASE {} ELSE `end_before` END, \
            `start_tz_utc` = CASE {} ELSE `start_tz_utc` END, `end_tz_utc` = CASE {} ELSE `end_tz_utc` END, \
            `attr_lastupdate` = CASE {} ELSE `attr_lastupdate` END, `attr_map` = CASE {} ELSE `attr_map` END
            WHERE store_id = ? AND ({})" 
            , case_ops, case_ops, case_ops, case_ops, case_ops, case_ops, case_ops, pid_cmps
        )
    }
}
impl<'q> IntoArguments<'q, MySql> for UpdateProductArg {
    fn into_arguments(self) -> <MySql as AbstractDatabase>::Arguments<'q> {
        let mut out = MySqlArguments::default();
        let (store_id, items) = (self.0, self.1);
        items
            .iter()
            .map(|item| {
                out.add(item.product_id()).unwrap();
                out.add(item.base_price()).unwrap();
            })
            .count();
        items
            .iter()
            .map(|item| {
                out.add(item.product_id()).unwrap();
                let t = item.start_after().format(DATETIME_FORMAT).to_string();
                out.add(t).unwrap();
            })
            .count();
        items
            .iter()
            .map(|item| {
                out.add(item.product_id()).unwrap();
                let t = item.end_before().format(DATETIME_FORMAT).to_string();
                out.add(t).unwrap();
            })
            .count();
        items
            .iter()
            .map(|item| {
                out.add(item.product_id()).unwrap();
                let start_tz_utc = item.start_after().timezone().local_minus_utc() / 60;
                out.add(start_tz_utc as i16).unwrap();
            })
            .count();
        items
            .iter()
            .map(|item| {
                out.add(item.product_id()).unwrap();
                let end_tz_utc = item
                    .end_before()
                    .fixed_offset()
                    .timezone()
                    .local_minus_utc()
                    / 60;
                out.add(end_tz_utc as i16).unwrap();
            })
            .count();
        items
            .iter()
            .map(|item| {
                out.add(item.product_id()).unwrap();
                let t = item.attrs_charge().lastupdate().to_utc();
                out.add(t.format(DATETIME_FORMAT).to_string()).unwrap();
            })
            .count();
        items
            .iter()
            .map(|item| {
                out.add(item.product_id()).unwrap();
                let serial = item.attrs_charge().serialize_map().unwrap();
                out.add(serial).unwrap();
            })
            .count();
        out.add(store_id).unwrap();
        items
            .into_iter()
            .map(|item| {
                out.add(item.product_id()).unwrap();
            })
            .count();
        out
    } // end of fn into_arguments
} // end of impl IntoArguments for UpdateProductArg
impl From<UpdateProductArg> for (String, MySqlArguments) {
    fn from(value: UpdateProductArg) -> (String, MySqlArguments) {
        (
            UpdateProductArg::sql_pattern(value.1.len()),
            value.into_arguments(),
        )
    }
}

impl From<InsertUpdateMetaArg> for (String, MySqlArguments) {
    fn from(value: InsertUpdateMetaArg) -> (String, MySqlArguments) {
        let InsertUpdateMetaArg(store_id, currency) = value;
        let sql_patt = "INSERT INTO `seller_price_meta`(`id`,`currency`) VALUES (?,?) \
                        ON DUPLICATE KEY UPDATE `currency`=?"
            .to_string();
        let mut args = MySqlArguments::default();
        args.add(store_id).unwrap();
        args.add(currency.to_string()).unwrap();
        args.add(currency.to_string()).unwrap();
        (sql_patt, args)
    }
}

#[rustfmt::skip]
const SELECT_COLUMN_SEQ: [&'static str ; 8] = [
    "`product_id`", "`price`", "`start_after`", "`end_before`",
    "`start_tz_utc`", "`end_tz_utc`", "`attr_lastupdate`", "`attr_map`",
];

impl FetchProductOneSellerArg {
    fn sql_pattern(num_batch: usize) -> String {
        let pid_cmps = (0..num_batch)
            .map(|_| "(`product_id`=?)")
            .collect::<Vec<_>>()
            .join("OR");
        format!(
            "SELECT {} FROM `product_price` WHERE `store_id`=? AND ({})",
            SELECT_COLUMN_SEQ.join(","),
            pid_cmps
        )
    }
}
impl<'q> IntoArguments<'q, MySql> for FetchProductOneSellerArg {
    fn into_arguments(self) -> <MySql as AbstractDatabase>::Arguments<'q> {
        let mut out = MySqlArguments::default();
        let (store_id, items) = (self.0, self.1);
        out.add(store_id).unwrap();
        items
            .into_iter()
            .map(|product_id| {
                out.add(product_id).unwrap();
            })
            .count();
        out
    }
}
impl From<FetchProductOneSellerArg> for (String, MySqlArguments) {
    fn from(value: FetchProductOneSellerArg) -> (String, MySqlArguments) {
        let num_batch = value.1.len();
        assert!(num_batch > 0);
        (
            FetchProductOneSellerArg::sql_pattern(num_batch),
            value.into_arguments(),
        )
    }
}

impl From<FetchMetaOneSellerArg> for (String, MySqlArguments) {
    fn from(value: FetchMetaOneSellerArg) -> (String, MySqlArguments) {
        let store_id = value.0;
        let sql_patt = "SELECT `id`,`currency` FROM `seller_price_meta` WHERE `id`=?".to_string();
        let mut args = MySqlArguments::default();
        args.add(store_id).unwrap();
        (sql_patt, args)
    }
}

impl FetchProductManySellersArg {
    fn sql_pattern(num_batch: usize) -> String {
        let mut col_seq = SELECT_COLUMN_SEQ.to_vec();
        col_seq.push("`store_id`");
        let col_seq = col_seq.join(",");
        let pid_cmps = (0..num_batch)
            .map(|_| "(`store_id`=? AND `product_id`=?)")
            .collect::<Vec<_>>()
            .join(" OR ");
        format!("SELECT {col_seq} FROM `product_price` WHERE {}", pid_cmps)
    }
    fn seller_id_column_idx() -> usize {
        8usize
    }
}
impl<'q> IntoArguments<'q, MySql> for FetchProductManySellersArg {
    fn into_arguments(self) -> <MySql as AbstractDatabase>::Arguments<'q> {
        let mut out = MySqlArguments::default();
        let pids = self.0;
        pids.into_iter()
            .map(|id_| {
                let (store_id, prod_id) = (id_.store_id, id_.product_id);
                out.add(store_id).unwrap();
                out.add(prod_id).unwrap();
            })
            .count();
        out
    }
}
impl From<FetchProductManySellersArg> for (String, MySqlArguments) {
    fn from(value: FetchProductManySellersArg) -> (String, MySqlArguments) {
        let num_batch = value.0.len();
        assert!(num_batch > 0);
        (
            FetchProductManySellersArg::sql_pattern(num_batch),
            value.into_arguments(),
        )
    }
}

impl FetchMetaManySellersArg {
    fn sql_pattern(num_batch: usize) -> String {
        let col_seq = "`id`,`currency`";
        let pid_cmps = (0..num_batch).map(|_| "?").collect::<Vec<_>>().join(",");
        format!(
            "SELECT {col_seq} FROM `seller_price_meta` WHERE `id` IN ({})",
            pid_cmps
        )
    }
}
impl<'q> IntoArguments<'q, MySql> for FetchMetaManySellersArg {
    fn into_arguments(self) -> <MySql as AbstractDatabase>::Arguments<'q> {
        let mut out = MySqlArguments::default();
        self.0
            .into_iter()
            .map(|store_id| {
                out.add(store_id).unwrap();
            })
            .count();
        out
    }
}
impl From<FetchMetaManySellersArg> for (String, MySqlArguments) {
    fn from(value: FetchMetaManySellersArg) -> (String, MySqlArguments) {
        let num_batch = value.0.len();
        assert!(num_batch > 0);
        (
            FetchMetaManySellersArg::sql_pattern(num_batch),
            value.into_arguments(),
        )
    }
}

impl DeleteSomeArg {
    fn sql_pattern(num_items: usize) -> String {
        let items_ph = (0..num_items).map(|_| "?").collect::<Vec<_>>().join(",");
        format!(
            "DELETE FROM `product_price` WHERE `store_id`=? AND (`product_id` IN ({}))",
            items_ph
        )
    }
}
impl<'q> IntoArguments<'q, MySql> for DeleteSomeArg {
    fn into_arguments(self) -> <MySql as AbstractDatabase>::Arguments<'q> {
        let mut out = MySqlArguments::default();
        let (store_id, data) = (self.0, self.1);
        out.add(store_id).unwrap();
        data.items
            .unwrap()
            .iter()
            .map(|product_id| {
                out.add(*product_id).unwrap();
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
        if items_r.is_empty() {
            Err(AppError {
                code: AppErrorCode::EmptyInputData,
                detail: Some("delete-product-price".to_string()),
            })
        } else {
            Ok((Self::sql_pattern(items_r.len()), self.into_arguments()))
        }
    }
}

impl From<DeleteStoreAllProductsArg> for (String, MySqlArguments) {
    fn from(value: DeleteStoreAllProductsArg) -> (String, MySqlArguments) {
        let sql_patt = "DELETE FROM `product_price` WHERE `store_id`=?";
        let mut args = MySqlArguments::default();
        let store_id = value.0;
        args.add(store_id).unwrap();
        (sql_patt.to_string(), args)
    }
}

impl From<DeleteStoreMetaArg> for (String, MySqlArguments) {
    fn from(value: DeleteStoreMetaArg) -> (String, MySqlArguments) {
        let sql_patt = "DELETE FROM `seller_price_meta` WHERE `id`=?";
        let mut args = MySqlArguments::default();
        let store_id = value.0;
        args.add(store_id).unwrap();
        (sql_patt.to_string(), args)
    }
}

impl TryFrom<MySqlRow> for ProductPriceModel {
    type Error = AppError;
    fn try_from(value: MySqlRow) -> DefaultResult<Self, Self::Error> {
        // TODO, discard fetching `store-id` column, the index of subsequent
        // columns should be decremented by one
        let product_id = value.try_get::<u64, usize>(0)?;
        let price = value.try_get::<u32, usize>(1)?;
        let start_after = value.try_get::<NaiveDateTime, usize>(2)?;
        let end_before = value.try_get::<NaiveDateTime, usize>(3)?;
        let start_tz_utc = value.try_get::<i16, usize>(4)?;
        let end_tz_utc = value.try_get::<i16, usize>(5)?;
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
        let attr_lastupdate = {
            let raw = value.try_get::<NaiveDateTime, usize>(6)?;
            let utc_tz = FixedOffset::east_opt(0).unwrap();
            raw.and_local_timezone(utc_tz).unwrap()
        };
        let attrprice = {
            let raw = value.try_get::<&[u8], usize>(7)?;
            let serial = std::str::from_utf8(raw).map_err(|e| AppError {
                code: AppErrorCode::DataCorruption,
                detail: Some(format!("cvt-prod-attr-price: {}", e.to_string())),
            })?;
            ProdAttriPriceModel::deserialize_map(serial)?
        };
        //println!("[DEBUG] product-id : {}, start_after naive: {:?}, final:{:?}",
        //        product_id, start_after_naive, start_after);
        let ts = [start_after, end_before, attr_lastupdate];
        let arg = (product_id, price, ts, attrprice);
        Ok(Self::from(arg))
    } // end of fn try-from
} // end of impl try-from for ProductPriceModel

impl TryFrom<MySqlRow> for ProductPriceModelSet {
    type Error = AppError;
    fn try_from(value: MySqlRow) -> DefaultResult<Self, Self::Error> {
        let store_id = value.try_get::<u32, usize>(0)?;
        let raw_currency = value.try_get::<&[u8], usize>(1)?;
        let raw_currency = std::str::from_utf8(raw_currency)
            .map_err(|e| AppError {
                code: AppErrorCode::DataCorruption,
                detail: Some(e.to_string()),
            })?
            .to_string();
        let currency = CurrencyDto::from(&raw_currency);
        if matches!(currency, CurrencyDto::Unknown) {
            return Err(AppError {
                code: AppErrorCode::DataCorruption,
                detail: Some(format!("invalid-currency: {raw_currency}")),
            });
        }
        Ok(Self {
            store_id,
            currency,
            items: Vec::new(),
        })
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
                InsertProductArg(store_id, items_processing).into()
            } else {
                UpdateProductArg(store_id, items_processing).into()
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

    fn _merge_err_data_corruption(errors: Vec<AppError>) -> AppError {
        let detail = errors
            .into_iter()
            .map(|e| e.to_string())
            .collect::<Vec<_>>()
            .join(", ");
        AppError {
            code: AppErrorCode::DataCorruption,
            detail: Some(detail),
        }
    }
} // end of impl ProductPriceMariaDbRepo

#[async_trait]
impl AbsProductPriceRepo for ProductPriceMariaDbRepo {
    async fn delete_all(&self, store_id: u32) -> DefaultResult<(), AppError> {
        let (sql_patt, args) = DeleteStoreAllProductsArg(store_id).into();
        self._delete_common(sql_patt, args).await?;
        let (sql_patt, args) = DeleteStoreMetaArg(store_id).into();
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
        ids: Vec<u64>,
    ) -> DefaultResult<ProductPriceModelSet, AppError> {
        if ids.is_empty() {
            return Err(AppError {
                code: AppErrorCode::ProductNotExist,
                detail: Some("missing-product-id".to_string()),
            });
        }
        let (sql_patt, args) = FetchMetaOneSellerArg(store_id).into();
        let mut rows = self._fetch_common(sql_patt, args).await?;
        if rows.is_empty() {
            return Err(AppError {
                code: AppErrorCode::ProductNotExist,
                detail: Some("missing-store".to_string()),
            });
        }
        let mut o = ProductPriceModelSet::try_from(rows.remove(0))?;
        let (sql_patt, args) = FetchProductOneSellerArg(store_id, ids).into();
        let rows = self._fetch_common(sql_patt, args).await?;
        let mut errors = vec![];
        o.items = rows
            .into_iter()
            .filter_map(|row| {
                ProductPriceModel::try_from(row)
                    .map_err(|e| {
                        errors.push(e);
                        0
                    })
                    .ok()
            })
            .collect::<Vec<_>>();
        if errors.is_empty() {
            Ok(o)
        } else {
            Err(Self::_merge_err_data_corruption(errors))
        }
    } // end of fn fetch

    async fn fetch_many(
        &self,
        ids: Vec<(u32, u64)>,
    ) -> DefaultResult<Vec<ProductPriceModelSet>, AppError> {
        if ids.is_empty() {
            return Err(AppError {
                code: AppErrorCode::ProductNotExist,
                detail: Some("missing-ids".to_string()),
            });
        }
        let mut errors: Vec<AppError> = Vec::new();
        let mut map: HashMap<u32, ProductPriceModelSet> = {
            let sids = ids
                .iter()
                .map(|(store_id, _)| *store_id)
                .collect::<Vec<_>>();
            let (sql_patt, args) = FetchMetaManySellersArg(sids).into();
            let rows = self._fetch_common(sql_patt, args).await?;
            let ppset_iter = rows
                .into_iter()
                .filter_map(|row| {
                    ProductPriceModelSet::try_from(row)
                        .map_err(|e| {
                            errors.push(e);
                            0
                        })
                        .ok()
                })
                .map(|v| (v.store_id, v));
            HashMap::from_iter(ppset_iter)
        };
        if !errors.is_empty() {
            return Err(Self::_merge_err_data_corruption(errors));
        }
        let pids = ids
            .into_iter()
            .map(|(store_id, product_id)| BaseProductIdentity {
                store_id,
                product_id,
            })
            .collect::<Vec<_>>();
        let (sql_patt, args) = FetchProductManySellersArg(pids).into();
        let rows = self._fetch_common(sql_patt, args).await?;
        let num_fetched = rows.len();
        let decoded = rows
            .into_iter()
            .map(|row| {
                let idx = FetchProductManySellersArg::seller_id_column_idx();
                let store_id = row.try_get::<u32, usize>(idx)?;
                let m = ProductPriceModel::try_from(row)?;
                Ok((store_id, m))
            })
            .filter_map(|r| {
                r.map_err(|e| {
                    errors.push(e);
                    0
                })
                .ok()
            })
            .collect::<Vec<_>>(); // to avoid mutable borrow twice

        let num_decoded = decoded
            .into_iter()
            .map(|(store_id, m)| {
                if let Some(mset) = map.get_mut(&store_id) {
                    mset.items.push(m);
                } else {
                    let e = AppError {
                        code: AppErrorCode::DataCorruption,
                        detail: Some(format!("store-missing-meta, id:{store_id}")),
                    };
                    errors.push(e);
                }
            })
            .count();
        if errors.is_empty() {
            assert_eq!(num_fetched, num_decoded);
            let out = map.into_values().collect::<Vec<_>>();
            Ok(out)
        } else {
            Err(Self::_merge_err_data_corruption(errors))
        }
    } // end of fn fetch_many

    async fn save(&self, mset: ProductPriceModelSet) -> DefaultResult<(), AppError> {
        let ProductPriceModelSet {
            store_id,
            items,
            currency,
        } = mset;
        let (ms_add, ms_modify) = ProductPriceModel::split_by_update_state(items);
        let mut conn = self.db.acquire().await?;
        let mut tx = conn.begin().await?;
        {
            let (sql_patt, args) = InsertUpdateMetaArg(store_id, currency).into();
            let resultset = run_query_once(&mut tx, sql_patt, args, None).await?;
            let num_affected = resultset.rows_affected() as usize;
            assert!(num_affected == 1 || num_affected == 2);
        }
        Self::_save(store_id, "update", 16, &mut tx, ms_modify).await?;
        Self::_save(store_id, "insert", 8, &mut tx, ms_add).await?;
        tx.commit().await?;
        Ok(())
    }
} // end of impl ProductPriceMariaDbRepo
