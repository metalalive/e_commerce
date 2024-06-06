pub(super) mod charge;
pub(super) mod order;

use std::result::Result;

use chrono::{DateTime, NaiveDate, NaiveDateTime, NaiveTime, SubsecRound, Utc};
use ecommerce_common::error::AppErrorCode;

use super::{AppRepoError, AppRepoErrorDetail, AppRepoErrorFnLabel};

#[allow(non_snake_case)]
fn raw_column_to_datetime(
    val: mysql_async::Value,
    subsec_precision: u16,
) -> Result<DateTime<Utc>, AppRepoError> {
    if let mysql_async::Value::Date(Y, M, D, h, m, s, us) = val {
        let d =
            NaiveDate::from_ymd_opt(Y as i32, M as u32, D as u32).ok_or_else(|| AppRepoError {
                fn_label: AppRepoErrorFnLabel::GetUnpaidOlines,
                code: AppErrorCode::DataCorruption,
                detail: AppRepoErrorDetail::DataRowParse("date-parse-fail".to_string()),
            })?;
        let t =
            NaiveTime::from_hms_micro_opt(h as u32, m as u32, s as u32, us).ok_or_else(|| {
                AppRepoError {
                    fn_label: AppRepoErrorFnLabel::GetUnpaidOlines,
                    code: AppErrorCode::DataCorruption,
                    detail: AppRepoErrorDetail::DataRowParse("time-parse-fail".to_string()),
                }
            })?;
        let out = NaiveDateTime::new(d, t)
            .and_utc()
            .trunc_subsecs(subsec_precision);
        Ok(out)
    } else {
        Err(AppRepoError {
            fn_label: AppRepoErrorFnLabel::GetUnpaidOlines,
            code: AppErrorCode::DataCorruption,
            detail: AppRepoErrorDetail::DataRowParse("datetime-unknown-value-type".to_string()),
        })
    }
}
