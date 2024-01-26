pub(super) mod product_policy;
pub(super) mod product_price;
pub(super) mod stock;
pub(super) mod order;

use std::ops::DerefMut;
use std::u8;
use std::result::Result as DefaultResult;
use std::io::ErrorKind;
use sqlx::{Executor, Transaction, MySql, Statement};
use sqlx::error::Error;
use sqlx::mysql::{MySqlArguments, MySqlQueryResult};

use crate::error::{AppError, AppErrorCode};
    
const DATETIME_FORMAT: &'static str = "%Y-%m-%d %H:%M:%S.%6f";

impl From<Error> for AppError {
    fn from(value: Error) -> Self {
        let (code, detail) = match value {
            Error::Configuration(e) =>
                (AppErrorCode::InvalidInput, e.to_string() + " invalid-db-config"),
            Error::Io(e) =>
                (AppErrorCode::IOerror(e.kind()), e.to_string() + " io-err-mariadb"),
            Error::Database(e) =>
                (AppErrorCode::RemoteDbServerFailure, e.to_string()),
            Error::Protocol(errmsg) =>
                (AppErrorCode::IOerror(ErrorKind::InvalidData), errmsg),
            Error::Decode(e) =>
                (AppErrorCode::DataCorruption, e.to_string()),
            Error::ColumnDecode { index, source } =>
                (AppErrorCode::DataCorruption, source.to_string() +
                 ", when decoding column at idx " + index.as_str()),
            Error::Tls(e) =>
                (AppErrorCode::IOerror(ErrorKind::NotConnected), e.to_string()),
            Error::TypeNotFound { type_name } =>
                (AppErrorCode::InvalidInput, type_name + " wrong-col-typ"),
            Error::ColumnNotFound(col_name) =>
                (AppErrorCode::IOerror(ErrorKind::NotFound) , col_name + "no-col"),
            Error::RowNotFound =>
                (AppErrorCode::IOerror(ErrorKind::NotFound) , format!("no-row")),
            Error::ColumnIndexOutOfBounds { index, len } =>
                (AppErrorCode::InvalidInput, format!("req-idx:{}, limit:{}", index, len)),
            Error::PoolTimedOut =>
                (AppErrorCode::IOerror(ErrorKind::ResourceBusy), format!("no-conn-avail")),
            Error::PoolClosed =>
                (AppErrorCode::Unknown, format!("pool-closed")),
            Error::WorkerCrashed =>
                (AppErrorCode::Unknown, format!("low-level-db-worker-crashed")),
            _others =>
                (AppErrorCode::Unknown, format!("internal-implementation-issue")),
        };
        Self { code , detail: Some(detail) }
    } // end of fn from
} // end of impl AppError

// currently it is only for order-id type casting
fn hex_to_bytes(src:&str) -> DefaultResult<Vec<u8>, AppError> {
    if src.len() % 2 == 0 {
        let results = (0 .. src.len()).step_by(2).map(|idx| {
            if let Some(hx) = src.get(idx .. idx+2) {
                match u8::from_str_radix(hx, 16) {
                    Ok(num) => Ok(num),
                    Err(_e) => Err(format!("parse-char-at-idx: {hx} , {idx}"))
                }
            } else { Err(format!("no-chars-at-idx: {idx}")) }
        }).collect::<Vec<_>>();
        let error = results.iter().find_map(|r| r.as_ref().err());
        if let Some(d) = error {
            Err(AppError {code:AppErrorCode::InvalidInput, detail:Some(d.clone()) })
        } else {
            let out = results.into_iter().map(|r| r.unwrap()).collect();
            Ok(out)
        }
    } else {
        let detail = format!("not-hex-string: {src}");
        Err(AppError { code: AppErrorCode::InvalidInput, detail: Some(detail) })
    }
}

async fn run_query_once(tx: &mut Transaction<'_, MySql>,
                        sql_patt:String,
                        args:MySqlArguments,
                        num_batch:usize )
    -> DefaultResult<MySqlQueryResult, AppError>
{
    let stmt = tx.prepare(sql_patt.as_str()).await?;
    let query = stmt.query_with(args);
    let exec = tx.deref_mut();
    let resultset = query.execute(exec).await?;
    let num_affected = resultset.rows_affected() as usize;
    if num_affected == num_batch {
        Ok(resultset)
    } else { // TODO, logging more detail for debugging
        let detail = format!("num_affected, actual:{}, expect:{}",
                             num_affected, num_batch );
        Err(AppError { code: AppErrorCode::DataCorruption,
            detail: Some(detail) })
    }
}
