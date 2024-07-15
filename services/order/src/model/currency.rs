use std::collections::HashMap;
use std::result::Result;

use rust_decimal::Decimal;

use ecommerce_common::api::dto::{
    CurrencyDto, CurrencySnapshotDto, OrderCurrencySnapshotDto, OrderSellerCurrencyDto,
};
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

pub struct OrderCurrencyModel {
    // save locked rate for both parties of buyer and sellers
    // Note in this project the base currency is always USD
    pub buyer: CurrencyModel,
    pub sellers: HashMap<u32, CurrencyModel>,
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

impl TryFrom<(CurrencyModelSet, CurrencyDto, Vec<(u32, CurrencyDto)>)> for OrderCurrencyModel {
    type Error = Vec<AppError>;
    fn try_from(
        value: (CurrencyModelSet, CurrencyDto, Vec<(u32, CurrencyDto)>),
    ) -> Result<Self, Self::Error> {
        let (exrate_avail, label_buyer, label_sellers) = value;
        let buyer = exrate_avail
            .find(&label_buyer)
            .map(|v| v.clone())
            .map_err(|mut e| {
                if let Some(msg) = &mut e.detail {
                    *msg += ", buyer";
                }
                vec![e]
            })?;
        let mut errors = Vec::new();
        let seller_iter = label_sellers.into_iter().filter_map(|(seller_id, label)| {
            exrate_avail
                .find(&label)
                .map(|v| (seller_id, v.clone()))
                .map_err(|mut e| {
                    if let Some(msg) = &mut e.detail {
                        *msg += ", seller:";
                        *msg += seller_id.to_string().as_str();
                    }
                    errors.push(e);
                })
                .ok()
        });
        let sellers = HashMap::from_iter(seller_iter);
        if errors.is_empty() {
            Ok(Self { buyer, sellers })
        } else {
            Err(errors)
        }
    } // end of fn try-from
} // end of impl OrderCurrencyModel

impl From<OrderCurrencyModel> for OrderCurrencySnapshotDto {
    fn from(value: OrderCurrencyModel) -> Self {
        let OrderCurrencyModel { buyer, sellers } = value;
        let mut snapshot = sellers
            .values()
            .map(CurrencySnapshotDto::from)
            .collect::<Vec<_>>();
        let exist = sellers.values().any(|v| v.name == buyer.name);
        if !exist {
            let item = CurrencySnapshotDto::from(&buyer);
            snapshot.push(item);
        }
        let sellers = sellers
            .into_iter()
            .map(|(seller_id, v)| OrderSellerCurrencyDto {
                seller_id,
                currency: v.name,
            })
            .collect::<Vec<_>>();
        Self {
            snapshot,
            sellers,
            buyer: buyer.name,
        }
    } // end of fn from
} // end of impl OrderCurrencySnapshotDto

impl OrderCurrencyModel {
    pub fn to_buyer_rate(&self, seller_id: u32) -> Result<CurrencyModel, AppError> {
        let c0 = &self.buyer;
        let c1 = self.sellers.get(&seller_id).ok_or(AppError {
            code: AppErrorCode::InvalidInput,
            detail: Some("rate-not-found".to_string()),
        })?;
        let newrate = c0.rate / c1.rate;
        Ok(CurrencyModel {
            name: self.buyer.name.clone(),
            rate: newrate,
        })
    }
}
