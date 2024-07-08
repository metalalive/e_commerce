pub(super) mod cart;
pub(super) mod currency;
pub(super) mod oline_return;
pub(super) mod order;
pub(super) mod product_policy;
pub(super) mod product_price;
pub(super) mod stock;

use sqlx::error::Error;
use sqlx::mysql::{MySqlArguments, MySqlQueryResult, MySqlRow};
use sqlx::{Executor, MySql, Row, Statement, Transaction};
use std::io::ErrorKind;
use std::ops::DerefMut;
use std::result::Result as DefaultResult;
use std::u8;

use crate::error::AppError;
use ecommerce_common::adapter::repository::OidBytes;
use ecommerce_common::error::AppErrorCode;

const DATETIME_FORMAT: &str = "%Y-%m-%d %H:%M:%S.%6f";

impl From<Error> for AppError {
    fn from(value: Error) -> Self {
        let (code, detail) = match value {
            Error::Configuration(e) => (
                AppErrorCode::InvalidInput,
                e.to_string() + " invalid-db-config",
            ),
            Error::Io(e) => (
                AppErrorCode::IOerror(e.kind()),
                e.to_string() + " io-err-mariadb",
            ),
            Error::Database(e) => (AppErrorCode::RemoteDbServerFailure, e.to_string()),
            Error::Protocol(errmsg) => (AppErrorCode::IOerror(ErrorKind::InvalidData), errmsg),
            Error::Decode(e) => (AppErrorCode::DataCorruption, e.to_string()),
            Error::ColumnDecode { index, source } => (
                AppErrorCode::DataCorruption,
                source.to_string() + ", when decoding column at idx " + index.as_str(),
            ),
            Error::Tls(e) => (
                AppErrorCode::IOerror(ErrorKind::NotConnected),
                e.to_string(),
            ),
            Error::TypeNotFound { type_name } => {
                (AppErrorCode::InvalidInput, type_name + " wrong-col-typ")
            }
            Error::ColumnNotFound(col_name) => (
                AppErrorCode::IOerror(ErrorKind::NotFound),
                col_name + "no-col",
            ),
            Error::RowNotFound => (
                AppErrorCode::IOerror(ErrorKind::NotFound),
                "no-row".to_string(),
            ),
            Error::ColumnIndexOutOfBounds { index, len } => (
                AppErrorCode::InvalidInput,
                format!("req-idx:{}, limit:{}", index, len),
            ),
            Error::PoolTimedOut => (
                AppErrorCode::DatabaseServerBusy,
                "no-conn-avail".to_string(),
            ),
            Error::PoolClosed => (AppErrorCode::Unknown, "pool-closed".to_string()),
            Error::WorkerCrashed => (
                AppErrorCode::Unknown,
                "low-level-db-worker-crashed".to_string(),
            ),
            _others => (
                AppErrorCode::Unknown,
                "internal-implementation-issue".to_string(),
            ),
        };
        Self {
            code,
            detail: Some(detail),
        }
    } // end of fn from
} // end of impl AppError

fn to_app_oid(row: &MySqlRow, idx: usize) -> DefaultResult<String, AppError> {
    let raw = row.try_get::<Vec<u8>, usize>(idx)?;
    let out = OidBytes::to_app_oid(raw)?;
    Ok(out)
}

async fn run_query_once(
    tx: &mut Transaction<'_, MySql>,
    sql_patt: String,
    args: MySqlArguments,
    maybe_num_batch: Option<usize>,
) -> DefaultResult<MySqlQueryResult, AppError> {
    let stmt = tx.prepare(sql_patt.as_str()).await?;
    let query = stmt.query_with(args);
    let exec = tx.deref_mut();
    let resultset = query.execute(exec).await?;
    if let Some(num_batch) = maybe_num_batch {
        let num_affected = resultset.rows_affected() as usize;
        if num_affected == num_batch {
            Ok(resultset)
        } else {
            // TODO, logging more detail for debugging
            let detail = format!(
                "num_affected, actual:{}, expect:{}",
                num_affected, num_batch
            );
            Err(AppError {
                code: AppErrorCode::DataCorruption,
                detail: Some(detail),
            })
        }
    } else {
        Ok(resultset)
    }
}
