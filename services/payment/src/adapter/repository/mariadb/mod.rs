pub(super) mod charge;
mod charge_converter;
pub(super) mod merchant;
mod order_replica;

use std::result::Result;

use chrono::{DateTime, NaiveDate, NaiveDateTime, NaiveTime, SubsecRound, Utc};
use ecommerce_common::error::AppErrorCode;

use super::AppRepoErrorDetail;

const DATETIME_FMT_P0F: &str = "%Y-%m-%d %H:%M:%S";

#[allow(non_snake_case)]
fn raw_column_to_datetime(
    val: mysql_async::Value,
    subsec_precision: u16,
) -> Result<DateTime<Utc>, (AppErrorCode, AppRepoErrorDetail)> {
    let result = if let mysql_async::Value::Date(Y, M, D, h, m, s, us) = val {
        let res_d = NaiveDate::from_ymd_opt(Y as i32, M as u32, D as u32).ok_or("date-parse-fail");
        let res_t = NaiveTime::from_hms_micro_opt(h as u32, m as u32, s as u32, us)
            .ok_or("time-parse-fail");
        match (res_d, res_t) {
            (Ok(d), Ok(t)) => Ok(NaiveDateTime::new(d, t)
                .and_utc()
                .trunc_subsecs(subsec_precision)),
            (Err(e), _) => Err(e),
            (Ok(_), Err(e)) => Err(e),
        }
    } else {
        Err("datetime-unknown-value-type")
    };
    result.map_err(|msg| {
        (
            AppErrorCode::DataCorruption,
            AppRepoErrorDetail::DataRowParse(msg.to_string()),
        )
    })
}
