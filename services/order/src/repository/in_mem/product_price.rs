use std::sync::Arc;
use std::boxed::Box;
use std::result::Result as DefaultResult;
use std::collections::HashMap;

use async_trait::async_trait;
use chrono::DateTime;

use crate::AppDataStoreContext;
use crate::api::rpc::dto::ProductPriceDeleteDto;
use crate::constant::ProductType;
use crate::datastore::{AbstInMemoryDStore, AppInMemFetchKeys, AbsDStoreFilterKeyOp};
use crate::error::{AppError, AppErrorCode};
use crate::model::{ProductPriceModelSet, ProductPriceModel};
use super::super::AbsProductPriceRepo;

const TABLE_LABEL: &'static str = "product_price";

enum InMemColIdx {Price, SellerId, StartAfter, EndBefore, ProductId, ProductType, TotNumColumns}

impl Into<usize> for InMemColIdx {
    fn into(self) -> usize {
        match self {
            Self::Price => 0,
            Self::StartAfter => 1,
            Self::EndBefore => 2,
            Self::ProductId => 3,
            Self::ProductType => 4,
            Self::SellerId => 5,
            Self::TotNumColumns => 6,
        }
    }
}

struct InnerDStoreFilterKeyOp {pattern_prefix:String}

impl AbsDStoreFilterKeyOp for InnerDStoreFilterKeyOp {
    fn filter(&self, k:&String, _v:&Vec<String>) -> bool {
        if let Some(pos) = k.find(self.pattern_prefix.as_str()) {
            pos == 0
        } else {false}
    }
}
impl InnerDStoreFilterKeyOp {
    fn new(store_id:u32) -> Self {
        let patt = format!("{store_id}-");
        Self { pattern_prefix: patt }
    }
}

pub struct ProductPriceInMemRepo {
    datastore: Arc<Box<dyn AbstInMemoryDStore>>
}

#[async_trait]
impl AbsProductPriceRepo for ProductPriceInMemRepo {
    async fn new(dstore:Arc<AppDataStoreContext>) -> DefaultResult<Box<dyn AbsProductPriceRepo>, AppError>
        where Self:Sized
    {
        match Self::_new(dstore).await {
            Ok(rp) => Ok(Box::new(rp)),
            Err(e) => Err(e)
        }
    }
    async fn delete_all(&self, store_id:u32) -> Result<(), AppError>
    {
        let op = InnerDStoreFilterKeyOp::new(store_id);
        let filtered = self.datastore.filter_keys(TABLE_LABEL.to_string(), &op).await?;
        let mut allkeys = HashMap::new();
        allkeys.insert(TABLE_LABEL.to_string(), filtered);
        self._delete_common(allkeys).await
    }
    
    async fn delete(&self, store_id:u32, ids:ProductPriceDeleteDto) -> Result<(), AppError>
    {
        let _ids = {
            let mut out = vec![];
            if let Some(p) = &ids.pkgs {
                out.extend(p.iter().map(|id| (ids.pkg_type.clone(), id.clone())));
            }
            if let Some(p) = &ids.items {
                out.extend(p.iter().map(|id| (ids.item_type.clone(), id.clone())));
            }
            out
        };
        if _ids.is_empty() {
            Err(AppError { code: AppErrorCode::EmptyInputData,
                detail: Some(format!("deleting-prodcut-price-id")) })
        } else {
            let allkeys = self.gen_id_keys(store_id, _ids);
            let mut h = HashMap::new();
            h.insert(TABLE_LABEL.to_string(), allkeys);
            self._delete_common(h).await
        }
    }

    async fn fetch(&self, store_id:u32, ids:Vec<(ProductType,u64)>) -> Result<ProductPriceModelSet, AppError>
    {
        let allkeys = self.gen_id_keys(store_id, ids);
        let mut info = HashMap::new();
        info.insert(TABLE_LABEL.to_string(), allkeys);
        let items = self._fetch(info).await?;
        let items = items.into_iter().map(|(_seller_id, obj)| obj).collect();
        let obj = ProductPriceModelSet { items, store_id };
        Ok(obj)
    } // end of fn fetch
    
    async fn fetch_many(&self, ids:Vec<(u32,ProductType,u64)>)
        -> DefaultResult<Vec<ProductPriceModelSet>, AppError>
    {
        let info = {
            let allkeys = ids.into_iter().map(|id| {
                let mut r = self.gen_id_keys(id.0, vec![(id.1, id.2)]);
                assert_eq!(r.len(), 1);
                r.remove(0)
            }).collect();
            let mut a = HashMap::new();
            a.insert(TABLE_LABEL.to_string(), allkeys);
            a
        };
        let items = self._fetch(info).await?;
        let mut modelmap = HashMap::new();
        let _ = items.into_iter().map(|(seller_id, model)| {
            let mset = if let Some(m) = modelmap.get_mut(&seller_id) {
                m
            } else {
                let m = ProductPriceModelSet {store_id:seller_id, items:vec![]};
                modelmap.insert(seller_id, m);
                modelmap.get_mut(&seller_id).unwrap()
            };
            mset.items.push(model)
        }).collect::<Vec<()>>();
        let out  = modelmap.into_values().collect();
        Ok(out)
    } // end of fn fetch_many

    async fn save(&self, ppset:ProductPriceModelSet) -> Result<(), AppError>
    {
        if ppset.store_id == 0 || ppset.items.is_empty() {
            return Err(AppError {code: AppErrorCode::EmptyInputData,
                detail:Some(format!("save ProductPriceModel"))  });
        }
        let kv_pairs = ppset.items.iter().map(|m| {
            let store_id_str = ppset.store_id.to_string();
            let prod_typ_num:u8 = m.product_type.clone().into();
            let pkey = format!("{}-{}-{}", store_id_str, prod_typ_num.to_string(),
                    m.product_id.to_string());
            // manually allocate space in advance, instead of `Vec::with_capacity`
            let mut row = (0..InMemColIdx::TotNumColumns.into()).map(
                |_n| String::new())  .collect::<Vec<String>>();
            let _ = [ // so the order of columns can be arbitrary
                (InMemColIdx::SellerId, ppset.store_id.to_string()),
                (InMemColIdx::Price, m.price.to_string()),
                (InMemColIdx::ProductType, prod_typ_num.to_string()),
                (InMemColIdx::ProductId,  m.product_id.to_string()),
                (InMemColIdx::StartAfter, m.start_after.to_rfc3339()),
                (InMemColIdx::EndBefore,  m.end_before.to_rfc3339()),
            ].into_iter().map(|(idx, val)| {
                let idx:usize = idx.into();
                row[idx] = val;
            }).collect::<Vec<()>>();
            (pkey, row)
        });
        let rows = HashMap::from_iter(kv_pairs);
        let mut data = HashMap::new();
        data.insert(TABLE_LABEL.to_string(), rows);
        let _num = self.datastore.save(data).await?;
        Ok(())
    } // end of fn save
} // end of impl ProductPriceInMemRepo

impl ProductPriceInMemRepo {
    pub async fn _new(dstore:Arc<AppDataStoreContext>) -> DefaultResult<Self, AppError>
        where Self:Sized
    {
        if let Some(m) = &dstore.in_mem {
            m.create_table(TABLE_LABEL).await?;
            let obj = Self { datastore: m.clone() };
            Ok(obj)
        } else {
            Err(AppError {code:AppErrorCode::MissingDataStore,
                detail: Some(format!("in-memory"))}  )
        }
    }
    fn gen_id_keys(&self, store_id:u32, ids:Vec<(ProductType,u64)>) -> Vec<String>
    {
        ids.into_iter().map(|(_typ, _id)| {
            let typnum:u8 = _typ.into();
            format!("{store_id}-{}-{}", typnum.to_string(), _id.to_string())
        }).collect()
    }

    async fn _fetch(&self, ids:HashMap<String, Vec<String>>) ->  Result<Vec<(u32,ProductPriceModel)>, AppError>
    {
        let result_raw = self.datastore.fetch(ids).await?;
        let out = if let Some(t) = result_raw.get(TABLE_LABEL)
        { // TODO, reliability check
            t.into_iter().map(|(_key, row)| {
                let prod_typ_num:u8 = row.get::<usize>(InMemColIdx::ProductType.into())
                    .unwrap().parse().unwrap() ;
                let product_type = ProductType::from(prod_typ_num);
                let product_id = row.get::<usize>(InMemColIdx::ProductId.into())
                    .unwrap().parse().unwrap();
                let seller_id = row.get::<usize>(InMemColIdx::SellerId.into())
                    .unwrap().parse().unwrap();
                let price = row.get::<usize>(InMemColIdx::Price.into())
                    .unwrap().parse().unwrap();
                let start_after = row.get::<usize>(InMemColIdx::StartAfter.into()).unwrap();
                let end_before  = row.get::<usize>(InMemColIdx::EndBefore.into()).unwrap();
                let start_after = DateTime::parse_from_rfc3339(start_after).unwrap().into();
                let end_before  = DateTime::parse_from_rfc3339(end_before).unwrap().into();
                let obj = ProductPriceModel {product_type, product_id, price,
                    start_after, end_before, is_create:false};
                (seller_id, obj)
            }).collect()
        } else { Vec::new() };
        Ok(out)
    } // end of fn _fetch
    
    async fn _delete_common(&self, keys:AppInMemFetchKeys) -> Result<(), AppError>
    {
        let _num_del = self.datastore.delete(keys).await?;
        Ok(())
    }
} // end of impl ProductPriceInMemRepo
