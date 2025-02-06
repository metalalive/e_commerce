use async_trait::async_trait;
use std::boxed::Box;
use std::collections::HashMap;
use std::convert::Into;
use std::sync::Arc;

use ecommerce_common::error::AppErrorCode;

use super::super::AbstProductPolicyRepo;
use crate::datastore::AbstInMemoryDStore;
use crate::error::AppError;
use crate::model::{ProductPolicyModel, ProductPolicyModelSet};

const TABLE_LABEL: &str = "product_policy";

enum InMemColIdx {
    AutoCancel,
    Warranty,
    MaxNumRsv,
    MinNumRsv,
    TotNumColumns,
}

impl From<InMemColIdx> for usize {
    fn from(value: InMemColIdx) -> usize {
        match value {
            InMemColIdx::AutoCancel => 0,
            InMemColIdx::Warranty => 1,
            InMemColIdx::MaxNumRsv => 2,
            InMemColIdx::MinNumRsv => 3,
            InMemColIdx::TotNumColumns => 4,
        }
    }
}

pub struct ProductPolicyInMemRepo {
    datastore: Arc<Box<dyn AbstInMemoryDStore>>,
}

impl ProductPolicyInMemRepo {
    pub async fn new(m: Arc<Box<dyn AbstInMemoryDStore>>) -> Result<Self, AppError> {
        m.create_table(TABLE_LABEL).await?;
        Ok(Self { datastore: m })
    }
}

#[async_trait]
impl AbstProductPolicyRepo for ProductPolicyInMemRepo {
    async fn fetch(&self, ids: Vec<u64>) -> Result<ProductPolicyModelSet, AppError> {
        let info = {
            let v = ids.iter().map(|prod_id| prod_id.to_string()).collect();
            let items = [(TABLE_LABEL.to_string(), v)];
            HashMap::from(items)
        };
        let result_raw = self.datastore.fetch(info).await?;
        let filtered = if let Some(d) = result_raw.get(TABLE_LABEL) {
            // raw strings to model instances
            d.iter()
                .map(|(key, row)| {
                    let id_elms = key.split('-').collect::<Vec<&str>>();
                    let product_id = id_elms[0].parse().unwrap();
                    let auto_cancel_secs = row
                        .get::<usize>(InMemColIdx::AutoCancel.into())
                        .unwrap()
                        .parse()
                        .unwrap();
                    let warranty_hours = row
                        .get::<usize>(InMemColIdx::Warranty.into())
                        .unwrap()
                        .parse()
                        .unwrap();
                    let max_num_rsv = row
                        .get::<usize>(InMemColIdx::MaxNumRsv.into())
                        .unwrap()
                        .parse()
                        .unwrap();
                    let min_num_rsv = row
                        .get::<usize>(InMemColIdx::MinNumRsv.into())
                        .unwrap()
                        .parse()
                        .unwrap();
                    ProductPolicyModel {
                        product_id,
                        auto_cancel_secs,
                        warranty_hours,
                        max_num_rsv,
                        min_num_rsv,
                        is_create: false,
                    }
                })
                .collect()
        } else {
            Vec::new()
        };
        Ok(ProductPolicyModelSet { policies: filtered })
    } // end of fn fetch

    async fn save(&self, ppset: ProductPolicyModelSet) -> Result<(), AppError> {
        if ppset.policies.is_empty() {
            return Err(AppError {
                code: AppErrorCode::EmptyInputData,
                detail: Some("save ProductPolicyModel".to_string()),
            });
        }
        let data = {
            let mut h = HashMap::new();
            let table_data = {
                let kv_pairs = ppset.policies.iter().map(|m| {
                    let pkey = format!("{}", m.product_id);
                    // manually allocate space in advance, instead of `Vec::with_capacity`
                    let mut row = (0..InMemColIdx::TotNumColumns.into())
                        .map(|_n| String::new())
                        .collect::<Vec<String>>();
                    let _ = [
                        // so the order of columns can be arbitrary
                        (InMemColIdx::MinNumRsv, m.min_num_rsv.to_string()),
                        (InMemColIdx::MaxNumRsv, m.max_num_rsv.to_string()),
                        (InMemColIdx::Warranty, m.warranty_hours.to_string()),
                        (InMemColIdx::AutoCancel, m.auto_cancel_secs.to_string()),
                    ]
                    .into_iter()
                    .map(|(idx, val)| {
                        let idx: usize = idx.into();
                        row[idx] = val;
                    })
                    .collect::<Vec<()>>();
                    (pkey, row)
                });
                HashMap::from_iter(kv_pairs)
            };
            h.insert(TABLE_LABEL.to_string(), table_data);
            h
        };
        let _num_saved = self.datastore.save(data).await?;
        Ok(())
    } // end of fn save
} // end of impl AbstProductPolicyRepo
