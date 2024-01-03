pub(super) mod product_policy;
pub(super) mod product_price;

use std::io::ErrorKind;
use sqlx::error::Error;

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

