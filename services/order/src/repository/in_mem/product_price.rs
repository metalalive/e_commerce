use std::boxed::Box;
use std::collections::hash_map::RandomState;
use std::collections::HashMap;
use std::result::Result as DefaultResult;
use std::sync::Arc;

use async_trait::async_trait;
use chrono::DateTime;

use ecommerce_common::api::dto::CurrencyDto;
use ecommerce_common::error::AppErrorCode;

use super::super::AbsProductPriceRepo;
use crate::api::rpc::dto::ProductPriceDeleteDto;
use crate::datastore::{
    AbsDStoreFilterKeyOp, AbstInMemoryDStore, AppInMemFetchKeys, AppInMemFetchedSingleTable,
};
use crate::error::AppError;
use crate::model::{ProductPriceModel, ProductPriceModelSet};

const TABLE_LABELS: [&str; 2] = ["store_meta", "product_price"];

enum InMemColIdx {
    Price,
    SellerId,
    StartAfter,
    EndBefore,
    ProductId,
    TotNumColumns,
}

#[allow(clippy::from_over_into)]
impl Into<usize> for InMemColIdx {
    fn into(self) -> usize {
        match self {
            Self::Price => 0,
            Self::StartAfter => 1,
            Self::EndBefore => 2,
            Self::ProductId => 3,
            Self::SellerId => 4,
            Self::TotNumColumns => 5,
        }
    }
}

struct InnerDStoreFilterKeyOp {
    pattern_prefix: String,
}

impl AbsDStoreFilterKeyOp for InnerDStoreFilterKeyOp {
    fn filter(&self, k: &String, _v: &Vec<String>) -> bool {
        if let Some(pos) = k.find(self.pattern_prefix.as_str()) {
            pos == 0
        } else {
            false
        }
    }
}
impl InnerDStoreFilterKeyOp {
    fn new(store_id: u32) -> Self {
        let patt = format!("{store_id}-");
        Self {
            pattern_prefix: patt,
        }
    }
}

struct UpdateMetaArgs(AppInMemFetchedSingleTable);
struct UpdateProductItemArgs(AppInMemFetchedSingleTable);

impl From<(u32, CurrencyDto)> for UpdateMetaArgs {
    fn from(value: (u32, CurrencyDto)) -> Self {
        let (store_id, currency) = value;
        let key = store_id.to_string();
        let row = vec![currency.to_string()];
        let inner = HashMap::from([(key, row)]);
        Self(inner)
    }
}

impl From<(u32, Vec<ProductPriceModel>)> for UpdateProductItemArgs {
    fn from(value: (u32, Vec<ProductPriceModel>)) -> Self {
        let (store_id, items) = value;
        let kv_pairs = items.iter().map(|m| {
            let (store_id, product_id) = (store_id, m.product_id);
            let pkey = format!("{store_id}-{product_id}");
            // manually allocate space in advance, instead of `Vec::with_capacity`
            let mut row = (0..InMemColIdx::TotNumColumns.into())
                .map(|_n| String::new())
                .collect::<Vec<String>>();
            let _ = [
                // so the order of columns can be arbitrary
                (InMemColIdx::SellerId, store_id.to_string()),
                (InMemColIdx::Price, m.price.to_string()),
                (InMemColIdx::ProductId, m.product_id.to_string()),
                (InMemColIdx::StartAfter, m.start_after.to_rfc3339()),
                (InMemColIdx::EndBefore, m.end_before.to_rfc3339()),
            ]
            .into_iter()
            .map(|(idx, val)| {
                let idx: usize = idx.into();
                row[idx] = val;
            })
            .collect::<Vec<()>>();
            (pkey, row)
        });
        let inner = HashMap::from_iter(kv_pairs);
        Self(inner)
    }
} // end of impl UpdateProductItemArgs

pub struct ProductPriceInMemRepo {
    datastore: Arc<Box<dyn AbstInMemoryDStore>>,
}

#[async_trait]
impl AbsProductPriceRepo for ProductPriceInMemRepo {
    async fn delete_all(&self, store_id: u32) -> Result<(), AppError> {
        let op = InnerDStoreFilterKeyOp::new(store_id);
        let filtered = self
            .datastore
            .filter_keys(TABLE_LABELS[1].to_string(), &op)
            .await?;
        let mut allkeys = HashMap::new();
        allkeys.insert(TABLE_LABELS[1].to_string(), filtered);
        allkeys.insert(TABLE_LABELS[0].to_string(), vec![store_id.to_string()]);
        self._delete_common(allkeys).await
    }

    async fn delete(&self, store_id: u32, ids: ProductPriceDeleteDto) -> Result<(), AppError> {
        let _ids = ids.items.clone().unwrap_or_default();
        if _ids.is_empty() {
            Err(AppError {
                code: AppErrorCode::EmptyInputData,
                detail: Some("deleting-prodcut-price-id".to_string()),
            })
        } else {
            let allkeys = self.gen_id_keys(store_id, _ids);
            let mut h = HashMap::new();
            h.insert(TABLE_LABELS[1].to_string(), allkeys);
            self._delete_common(h).await
        }
    }

    async fn fetch(&self, store_id: u32, ids: Vec<u64>) -> Result<ProductPriceModelSet, AppError> {
        let allkeys = self.gen_id_keys(store_id, ids);
        let mut info = HashMap::new();
        info.insert(TABLE_LABELS[0].to_string(), vec![store_id.to_string()]);
        info.insert(TABLE_LABELS[1].to_string(), allkeys);
        let (meta, items) = self._fetch(info).await?;
        let currency = meta.get(&store_id).map(|d| d.0.clone()).ok_or(AppError {
            code: AppErrorCode::ProductNotExist,
            detail: Some("missing-store".to_string()),
        })?;
        let items = items.into_iter().map(|(_seller_id, obj)| obj).collect();
        let obj = ProductPriceModelSet {
            items,
            store_id,
            currency,
        };
        Ok(obj)
    } // end of fn fetch

    async fn fetch_many(
        &self,
        ids: Vec<(u32, u64)>,
    ) -> DefaultResult<Vec<ProductPriceModelSet>, AppError> {
        let info = {
            let allkeys4meta = ids.iter().map(|id| id.0.to_string()).collect();
            let allkeys4item = ids
                .into_iter()
                .map(|id| {
                    let mut r = self.gen_id_keys(id.0, vec![id.1]);
                    assert_eq!(r.len(), 1);
                    r.remove(0)
                })
                .collect();
            let mut a = HashMap::new();
            a.insert(TABLE_LABELS[0].to_string(), allkeys4meta);
            a.insert(TABLE_LABELS[1].to_string(), allkeys4item);
            a
        };
        let (meta, items) = self._fetch(info).await?;
        let mut modelmap = HashMap::new();
        let _ = items
            .into_iter()
            .map(|(seller_id, model)| {
                let mset = if let Some(m) = modelmap.get_mut(&seller_id) {
                    m
                } else {
                    let meta_item = meta.get(&seller_id).unwrap();
                    let m = ProductPriceModelSet {
                        store_id: seller_id,
                        currency: meta_item.0.clone(),
                        items: vec![],
                    };
                    modelmap.insert(seller_id, m);
                    modelmap.get_mut(&seller_id).unwrap()
                };
                mset.items.push(model)
            })
            .collect::<Vec<_>>();
        let out = modelmap.into_values().collect();
        Ok(out)
    } // end of fn fetch_many

    async fn save(&self, ppset: ProductPriceModelSet) -> Result<(), AppError> {
        if ppset.store_id == 0 || ppset.items.is_empty() {
            return Err(AppError {
                code: AppErrorCode::EmptyInputData,
                detail: Some("save ProductPriceModel".to_string()),
            });
        }
        let ProductPriceModelSet {
            store_id,
            currency,
            items,
        } = ppset;
        let mut data = HashMap::new();
        let rows = UpdateProductItemArgs::from((store_id, items)).0;
        data.insert(TABLE_LABELS[1].to_string(), rows);
        let rows = UpdateMetaArgs::from((store_id, currency)).0;
        data.insert(TABLE_LABELS[0].to_string(), rows);
        let _num = self.datastore.save(data).await?;
        Ok(())
    } // end of fn save
} // end of impl ProductPriceInMemRepo

impl ProductPriceInMemRepo {
    pub async fn new(m: Arc<Box<dyn AbstInMemoryDStore>>) -> DefaultResult<Self, AppError> {
        for label in TABLE_LABELS {
            m.create_table(label).await?;
        }
        Ok(Self {
            datastore: m.clone(),
        })
    }
    fn gen_id_keys(&self, store_id: u32, ids: Vec<u64>) -> Vec<String> {
        ids.into_iter()
            .map(|prod_id| format!("{store_id}-{prod_id}"))
            .collect()
    }

    async fn _fetch(
        &self,
        ids: HashMap<String, Vec<String>>,
    ) -> Result<
        (
            HashMap<u32, (CurrencyDto,), RandomState>,
            Vec<(u32, ProductPriceModel)>,
        ),
        AppError,
    > {
        let mut result_raw = self.datastore.fetch(ids).await?;
        let meta_raw = result_raw.remove(TABLE_LABELS[0]).ok_or(AppError {
            code: AppErrorCode::DataTableNotExist,
            detail: Some(TABLE_LABELS[0].to_string()),
        })?;
        let meta_iter = meta_raw.into_iter().map(|(key, row)| {
            let seller_id = key.parse::<u32>().unwrap();
            let currency_raw = row.first().unwrap();
            let currency = CurrencyDto::from(currency_raw);
            (seller_id, (currency,))
        });
        let meta = HashMap::from_iter(meta_iter);
        let pitems = if let Some(t) = result_raw.remove(TABLE_LABELS[1]) {
            // TODO, reliability check
            t.values()
                .map(|row| {
                    let product_id = row
                        .get::<usize>(InMemColIdx::ProductId.into())
                        .unwrap()
                        .parse()
                        .unwrap();
                    let seller_id = row
                        .get::<usize>(InMemColIdx::SellerId.into())
                        .unwrap()
                        .parse()
                        .unwrap();
                    let price = row
                        .get::<usize>(InMemColIdx::Price.into())
                        .unwrap()
                        .parse()
                        .unwrap();
                    let start_after = row.get::<usize>(InMemColIdx::StartAfter.into()).unwrap();
                    let end_before = row.get::<usize>(InMemColIdx::EndBefore.into()).unwrap();
                    let start_after = DateTime::parse_from_rfc3339(start_after).unwrap();
                    let end_before = DateTime::parse_from_rfc3339(end_before).unwrap();
                    let obj = ProductPriceModel {
                        product_id,
                        price,
                        start_after,
                        end_before,
                        is_create: false,
                    };
                    (seller_id, obj)
                })
                .collect()
        } else {
            Vec::new()
        };
        Ok((meta, pitems))
    } // end of fn _fetch

    async fn _delete_common(&self, keys: AppInMemFetchKeys) -> Result<(), AppError> {
        let _num_del = self.datastore.delete(keys).await?;
        Ok(())
    }
} // end of impl ProductPriceInMemRepo
