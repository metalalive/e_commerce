pub(super) mod cart;
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
use std::vec::IntoIter;

use crate::error::{AppError, AppErrorCode};

const DATETIME_FORMAT: &'static str = "%Y-%m-%d %H:%M:%S.%6f";
const OID_BYTE_LENGTH: usize = 16;

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
                format!("no-row"),
            ),
            Error::ColumnIndexOutOfBounds { index, len } => (
                AppErrorCode::InvalidInput,
                format!("req-idx:{}, limit:{}", index, len),
            ),
            Error::PoolTimedOut => (AppErrorCode::DatabaseServerBusy, format!("no-conn-avail")),
            Error::PoolClosed => (AppErrorCode::Unknown, format!("pool-closed")),
            Error::WorkerCrashed => (
                AppErrorCode::Unknown,
                format!("low-level-db-worker-crashed"),
            ),
            _others => (
                AppErrorCode::Unknown,
                format!("internal-implementation-issue"),
            ),
        };
        Self {
            code,
            detail: Some(detail),
        }
    } // end of fn from
} // end of impl AppError

/*
* - size of order-id has to match database schema
* - In mariaDB, the BINARY column are right-padded with number of zero octets (0x0)
    to fill the length og declared binary column, this struct ensures any given hex
    string can be converted to correct binary format to database server.
* */
struct OidBytes([u8; OID_BYTE_LENGTH]);

impl<'a> TryFrom<&'a str> for OidBytes {
    type Error = AppError;
    fn try_from(value: &'a str) -> DefaultResult<Self, Self::Error> {
        if value.len() <= (OID_BYTE_LENGTH * 2) {
            let iter = hex_to_octet_iter(value)?;
            let mut dst = [0; OID_BYTE_LENGTH];
            let mut d_iter = dst.iter_mut();
            iter.map(|r| {
                let addr = d_iter.next().unwrap();
                let c = r.unwrap();
                *addr = c;
            })
            .count();
            let num_rotate = OID_BYTE_LENGTH - (value.len() >> 1);
            dst.rotate_right(num_rotate);
            Ok(OidBytes(dst))
        } else {
            let detail = format!("size-not-fit: {value}");
            Err(AppError {
                code: AppErrorCode::InvalidInput,
                detail: Some(detail),
            })
        }
    }
}
impl OidBytes {
    fn as_column(&self) -> Vec<u8> {
        self.0.to_vec()
    }
    fn to_app_oid(row: &MySqlRow, idx: usize) -> DefaultResult<String, AppError> {
        let raw = row.try_get::<Vec<u8>, usize>(idx)?;
        if raw.len() != OID_BYTE_LENGTH {
            let detail = format!("fetched-id-len: {}", raw.len());
            Err(AppError {
                code: AppErrorCode::DataCorruption,
                detail: Some(detail),
            })
        } else {
            let mut padded = true;
            let out = raw
                .into_iter()
                .filter_map(|b| {
                    if b != 0 {
                        padded = false;
                    }
                    if padded {
                        None
                    } else {
                        Some(format!("{:02x}", b))
                    }
                })
                .collect();
            Ok(out)
        }
    }
}

fn hex_to_octet_iter(src: &str) -> DefaultResult<IntoIter<Result<u8, String>>, AppError> {
    if src.len() % 2 == 0 {
        let results = (0..src.len())
            .step_by(2)
            .map(|idx| {
                if let Some(hx) = src.get(idx..idx + 2) {
                    match u8::from_str_radix(hx, 16) {
                        Ok(num) => Ok(num),
                        Err(_e) => Err(format!("parse-char-at-idx: {hx} , {idx}")),
                    }
                } else {
                    Err(format!("no-chars-at-idx: {idx}"))
                }
            })
            .collect::<Vec<_>>();
        let error = results.iter().find_map(|r| r.as_ref().err());
        if let Some(d) = error {
            Err(AppError {
                code: AppErrorCode::InvalidInput,
                detail: Some(d.clone()),
            })
        } else {
            let out = results.into_iter();
            Ok(out)
        } // cannot convert to u8 array using try-from method,  the size of given
          // char vector might not be the same as OID_BYTE_LENGTH
    } else {
        let detail = format!("not-hex-string: {src}");
        Err(AppError {
            code: AppErrorCode::InvalidInput,
            detail: Some(detail),
        })
    }
} // end of fn hex_to_octet_iter

#[test]
fn verify_hex_to_oidbytes() {
    let OidBytes(actual) = OidBytes::try_from("800EFF41").unwrap();
    let expect = [0x80, 0x0E, 0xFF, 0x41, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
    assert_eq!(actual, expect);
    let OidBytes(actual) = OidBytes::try_from("6D1405982C0EF7").unwrap();
    let expect = [
        0x6D, 0x14, 0x05, 0x98, 0x2C, 0x0E, 0xF7, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    ];
    assert_eq!(actual, expect);
    let OidBytes(actual) = OidBytes::try_from("0902900390049005a004a005a006a007").unwrap();
    let expect = [
        0x09, 0x02, 0x90, 0x03, 0x90, 0x04, 0x90, 0x05, 0xa0, 0x04, 0xa0, 0x05, 0xa0, 0x06, 0xa0,
        0x07,
    ];
    assert_eq!(actual, expect);
    let result = OidBytes::try_from("ec0902900390049005a004a005a006a007");
    assert!(result.is_err());
    if let Err(e) = result {
        assert_eq!(e.code, AppErrorCode::InvalidInput);
    }
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
