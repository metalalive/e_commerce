use std::result::Result;
use std::sync::Arc;
use std::vec::Vec;

use async_trait::async_trait;
use rust_decimal::Decimal;
use sqlx::database::HasArguments;
use sqlx::mysql::{MySqlArguments, MySqlRow};
use sqlx::{Acquire, Arguments, Executor, IntoArguments, MySql, Row, Statement};

use ecommerce_common::api::dto::CurrencyDto;
use ecommerce_common::error::AppErrorCode;

use super::run_query_once;
use crate::datastore::AppMariaDbStore;
use crate::error::AppError;
use crate::model::{CurrencyModel, CurrencyModelSet};
use crate::repository::AbsCurrencyRepo;

// the 2 internal constants should be consistent with database schema
const PRECISION_WHOLE_NUMBER: u32 = 8;
const PRECISION_FRACTIONAL: u32 = 4;

struct UpdateArgs(CurrencyModelSet);
struct FetchArgs(Vec<CurrencyDto>);

impl UpdateArgs {
    fn sql_pattern(num: usize) -> String {
        let cond_write = (0..num)
            .map(|_| "WHEN `name`=? THEN ? ")
            .collect::<Vec<_>>()
            .join("");
        let chosen_labels = (0..num).map(|_| "?").collect::<Vec<_>>().join(",");
        format!(
            "UPDATE `currency_exchange` SET `rate` = CASE {cond_write} ELSE \
                `rate` END WHERE `name` IN ({chosen_labels})"
        )
    }
}
impl<'q> IntoArguments<'q, MySql> for UpdateArgs {
    fn into_arguments(self) -> <MySql as HasArguments<'q>>::Arguments {
        let CurrencyModelSet {
            base: _,
            exchange_rates,
        } = self.0;
        let curr_labels = exchange_rates
            .iter()
            .map(|m| m.name.to_string())
            .collect::<Vec<_>>();
        let mut args = MySqlArguments::default();
        exchange_rates
            .into_iter()
            .map(|m| {
                args.add(m.name.to_string());
                args.add(m.rate);
            })
            .count();
        curr_labels
            .into_iter()
            .map(|label| {
                args.add(label);
            })
            .count();
        args
    }
}
impl From<UpdateArgs> for (String, MySqlArguments) {
    fn from(value: UpdateArgs) -> Self {
        let sql_patt = UpdateArgs::sql_pattern(value.0.exchange_rates.len());
        let args = value.into_arguments();
        (sql_patt, args)
    }
}

impl From<FetchArgs> for (String, MySqlArguments) {
    fn from(value: FetchArgs) -> Self {
        let num = value.0.len();
        let chosen_labels = (0..num).map(|_| "?").collect::<Vec<_>>().join(",");
        let sql_patt = format!(
            "SELECT `name`,`rate` FROM `currency_exchange`\
                               WHERE `name` IN ({chosen_labels})"
        );
        let mut args = MySqlArguments::default();
        value
            .0
            .into_iter()
            .map(|m| {
                args.add(m.to_string());
            })
            .count();
        (sql_patt, args)
    }
}

impl TryFrom<MySqlRow> for CurrencyModel {
    type Error = AppError;
    fn try_from(value: MySqlRow) -> Result<Self, Self::Error> {
        let name_serial = value.try_get::<String, usize>(0)?;
        let name = (&name_serial).into();
        if matches!(name, CurrencyDto::Unknown) {
            Err(AppError {
                code: AppErrorCode::DataCorruption,
                detail: Some(format!("invalid-currency-label: {name_serial}")),
            })
        } else {
            let rate = value.try_get::<Decimal, usize>(1)?;
            Ok(Self { name, rate })
        }
    }
}

pub(crate) struct CurrencyMariaDbRepo {
    _db: Arc<AppMariaDbStore>,
}

#[async_trait]
impl AbsCurrencyRepo for CurrencyMariaDbRepo {
    async fn fetch(&self, chosen: Vec<CurrencyDto>) -> Result<CurrencyModelSet, AppError> {
        let (sql_patt, args) = FetchArgs(chosen).into();
        let mut conn = self._db.acquire().await?;
        let stmt = conn.prepare(sql_patt.as_str()).await?;
        let query = stmt.query_with(args);
        let exec = &mut *conn;
        let rows = exec.fetch_all(query).await?;

        let mut errors = Vec::new();
        let exchange_rates = rows
            .into_iter()
            .filter_map(|m| {
                CurrencyModel::try_from(m)
                    .map_err(|e| {
                        errors.push(e);
                        0
                    })
                    .ok()
            })
            .collect::<Vec<_>>();
        if errors.is_empty() {
            Ok(CurrencyModelSet {
                base: CurrencyDto::USD,
                exchange_rates,
            })
        } else {
            Err(errors.remove(0))
        }
    } // end of fn fetch

    async fn save(&self, ms: CurrencyModelSet) -> Result<(), AppError> {
        if !matches!(ms.base, CurrencyDto::USD) {
            return Err(AppError {
                code: AppErrorCode::InvalidInput,
                detail: Some("invalid-base-currency".to_string()),
            });
        }
        Self::check_rate_range(&ms.exchange_rates)?;
        let num_updated = ms.exchange_rates.len();
        let (sql_patt, args) = UpdateArgs(ms).into();
        let mut conn = self._db.acquire().await?;
        let mut tx = conn.begin().await?;
        let _rs = run_query_once(&mut tx, sql_patt, args, Some(num_updated)).await?;
        tx.commit().await?;
        Ok(())
    } // end of fn save
} // end of impl CurrencyMariaDbRepo

impl CurrencyMariaDbRepo {
    pub fn try_build(dstores: &[Arc<AppMariaDbStore>]) -> Result<Self, AppError> {
        let _db = dstores.first().cloned().ok_or(AppError {
            code: AppErrorCode::MissingDataStore,
            detail: Some("mariadb".to_string()),
        })?;
        Ok(Self { _db })
    }
    fn check_rate_range(data: &[CurrencyModel]) -> Result<(), AppError> {
        // TODO, examine whether the code here could be reused elsewhere
        let wholenum_limit = 10i128.pow(PRECISION_WHOLE_NUMBER);
        let msgs = data
            .iter()
            .filter_map(|m| {
                let fractional = m.rate.scale();
                if fractional > PRECISION_FRACTIONAL {
                    Some(("scale".to_string(), m))
                } else {
                    let wholenum = m.rate.trunc().mantissa();
                    if wholenum >= wholenum_limit {
                        Some(("whole-num".to_string(), m))
                    } else {
                        None
                    }
                }
            })
            .map(|(msg, m)| {
                format!(
                    "name:{}, rate:{}, error:{}",
                    m.name.to_string(),
                    m.rate,
                    msg
                )
            })
            .collect::<Vec<_>>();
        if msgs.is_empty() {
            Ok(())
        } else {
            let e = AppError {
                code: AppErrorCode::ExceedingMaxLimit,
                detail: Some(msgs.join(", ")),
            };
            Err(e)
        }
    }
} // end of impl CurrencyMariaDbRepo
