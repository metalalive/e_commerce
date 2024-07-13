use rust_decimal::Decimal;

use ecommerce_common::api::dto::{CurrencyDto, CurrencySnapshotDto};
use ecommerce_common::error::AppErrorCode;

use crate::error::AppError;

// the 2 internal constants should be consistent with database schema
const PRECISION_WHOLE_NUMBER: u32 = 8;
const PRECISION_FRACTIONAL: u32 = 4;

#[derive(Clone)]
pub struct CurrencyModel {
    pub name: CurrencyDto,
    pub rate: Decimal,
}
pub struct CurrencyModelSet {
    pub base: CurrencyDto,
    pub exchange_rates: Vec<CurrencyModel>,
}

impl From<&CurrencyModel> for CurrencySnapshotDto {
    fn from(value: &CurrencyModel) -> Self {
        Self {
            name: value.name.clone(),
            rate: value.rate.to_string(),
        }
    }
}

impl CurrencyModel {
    pub(crate) fn trunc_rate_fraction(&mut self, scale: u32) {
        let new_rate = self.rate.trunc_with_scale(scale);
        self.rate = new_rate;
    }
}

impl CurrencyModelSet {
    pub(crate) fn trunc_rate_fraction(&mut self) {
        self.exchange_rates
            .iter_mut()
            .map(|v| v.trunc_rate_fraction(PRECISION_FRACTIONAL))
            .count();
    }

    pub(crate) fn check_rate_range(&self) -> Result<(), AppError> {
        let wholenum_limit = 10i128.pow(PRECISION_WHOLE_NUMBER);
        let msgs = self
            .exchange_rates
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
    } // end of  fn check_rate_range

    pub(super) fn find(&self, given: &CurrencyDto) -> Result<&CurrencyModel, AppError> {
        self.exchange_rates
            .iter()
            .find(|m| &m.name == given)
            .ok_or(AppError {
                code: AppErrorCode::InvalidInput,
                detail: Some(format!("fail-load-ex-rate, given:{}", given.to_string())),
            })
    }
} // end of impl CurrencyModelSet
