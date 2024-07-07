use std::boxed::Box;
use std::collections::HashMap;
use std::result::Result;
use std::sync::Arc;

use async_trait::async_trait;
use rust_decimal::Decimal;

use ecommerce_common::api::dto::CurrencyDto;
use ecommerce_common::error::AppErrorCode;

use crate::datastore::{AbstInMemoryDStore, AppInMemFetchKeys, AppInMemFetchedSingleTable};
use crate::error::AppError;
use crate::model::{CurrencyModel, CurrencyModelSet};
use crate::repository::AbsCurrencyRepo;

const TABLE_LABEL: &str = "currency_exchange";

struct FetchArgs(AppInMemFetchKeys);
struct UpdateArgs(AppInMemFetchedSingleTable);

pub struct CurrencyInMemRepo {
    dstore: Arc<Box<dyn AbstInMemoryDStore>>,
}

impl TryFrom<CurrencyModelSet> for UpdateArgs {
    type Error = AppError;
    fn try_from(value: CurrencyModelSet) -> Result<Self, Self::Error> {
        let result = value
            .exchange_rates
            .iter()
            .find(|m| matches!(m.name, CurrencyDto::Unknown))
            .map(|invalid| AppError {
                code: AppErrorCode::InvalidInput,
                detail: Some(format!(
                    "unacceptable-currency: {}",
                    invalid.name.to_string()
                )),
            });
        if let Some(e) = result {
            return Err(e);
        }
        let c_iter = value.exchange_rates.into_iter().map(|m| {
            let key = m.name.to_string();
            let row = vec![m.rate.to_string()];
            (key, row)
        });
        let inner = HashMap::from_iter(c_iter);
        Ok(Self(inner))
    }
}

impl From<Vec<CurrencyDto>> for FetchArgs {
    fn from(value: Vec<CurrencyDto>) -> Self {
        let rows = value.into_iter().map(|c| c.to_string()).collect();
        let inner = HashMap::from([(TABLE_LABEL.to_string(), rows)]);
        Self(inner)
    }
}

impl TryFrom<AppInMemFetchedSingleTable> for CurrencyModelSet {
    type Error = AppError;
    fn try_from(value: AppInMemFetchedSingleTable) -> Result<Self, Self::Error> {
        let mut errs_detail = Vec::new();
        let exchange_rates = value
            .into_iter()
            .filter_map(|(key, row)| {
                let name = CurrencyDto::from(&key);
                if matches!(name, CurrencyDto::Unknown) {
                    errs_detail.push(format!("unknown-currency-label: {}", key));
                    return None;
                }
                if let Some(ra) = row.first() {
                    Decimal::from_str_radix(ra.as_str(), 10)
                        .map_err(|e| {
                            let detail =
                                format!("decimal-decode-fail, saved: {}, reason: {:?}", ra, e);
                            errs_detail.push(detail);
                            e
                        })
                        .ok()
                        .map(|rate| (name, rate))
                } else {
                    errs_detail.push(format!("missing-currency-rate: {}", key));
                    None
                }
            })
            .map(|(name, rate)| CurrencyModel { name, rate })
            .collect();
        if errs_detail.is_empty() {
            // Note the base currency is always USD in this project
            Ok(CurrencyModelSet {
                base: CurrencyDto::USD,
                exchange_rates,
            })
        } else {
            Err(AppError {
                code: AppErrorCode::DataCorruption,
                detail: Some(errs_detail.remove(0)),
            })
        }
    } // end of fn try-from
} // end of impl CurrencyModelSet

#[async_trait]
impl AbsCurrencyRepo for CurrencyInMemRepo {
    async fn fetch(&self, chosen: Vec<CurrencyDto>) -> Result<CurrencyModelSet, AppError> {
        let data = FetchArgs::from(chosen).0;
        let mut resultset = self.dstore.fetch(data).await?;
        let raw = resultset.remove(TABLE_LABEL).ok_or(AppError {
            code: AppErrorCode::DataTableNotExist,
            detail: Some(TABLE_LABEL.to_string()),
        })?;
        CurrencyModelSet::try_from(raw)
    }

    async fn save(&self, ms: CurrencyModelSet) -> Result<(), AppError> {
        let rows = UpdateArgs::try_from(ms)?.0;
        let data = HashMap::from([(TABLE_LABEL.to_string(), rows)]);
        let _num_saved = self.dstore.save(data).await?;
        Ok(())
    }
} // end of impl CurrencyInMemRepo

impl CurrencyInMemRepo {
    pub async fn new(dstore: Arc<Box<dyn AbstInMemoryDStore>>) -> Result<Self, AppError> {
        dstore.create_table(TABLE_LABEL).await?;
        Ok(Self { dstore })
    }
} // end of impl CurrencyInMemRepo
