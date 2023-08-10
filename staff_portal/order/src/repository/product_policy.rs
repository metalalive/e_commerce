use std::convert::Into;
use std::collections::HashMap;
use std::sync::Arc;
use std::boxed::Box;
use async_trait::async_trait;

use crate::AppDataStoreContext;
use crate::datastore::AbstInMemoryDStore;
use crate::model::{ProductPolicyModelSet, ProductPolicyModel};
use crate::error::{AppError, AppErrorCode};
use super::AbstProductPolicyRepo;

const TABLE_LABEL: &'static str = "product_policy";

enum InMemColIdx {UserId, AutoCancel, Warranty, AsyncStockChk, TotNumColumns}

impl Into<usize> for InMemColIdx {
    fn into(self) -> usize {
        match self {
            Self::UserId => 0,
            Self::AutoCancel => 1,
            Self::Warranty => 2,
            Self::AsyncStockChk => 3,
            Self::TotNumColumns => 4
        }
    }
}

pub struct ProductPolicyInMemRepo
{
    datastore: Arc<Box<dyn AbstInMemoryDStore>>
}

#[async_trait]
impl AbstProductPolicyRepo for ProductPolicyInMemRepo
{
    fn new(ds:Arc<AppDataStoreContext>) -> Result<Box<dyn AbstProductPolicyRepo>, AppError>
        where Self:Sized
    {
        if let Some(m)= &ds.in_mem {
            m.create_table(TABLE_LABEL) ? ;
            let obj = Self{datastore: m.clone()};
            Ok(Box::new(obj))
        } else { // TODO, logging more detail ?
            let obj = AppError { code: AppErrorCode::MissingDataStore,
                detail: Some(format!("in-memory")) };
            Err(obj)
        }
    }

    async fn fetch(&self, usr_id:u32, ids:Vec<u64>) -> Result<ProductPolicyModelSet, AppError>
    {
        let info = {
            let mut h = HashMap::new();
            let v = ids.iter().map(u64::to_string).collect();
            h.insert(TABLE_LABEL.to_string(), v);
            h
        };
        let result_raw = self.datastore.fetch(info)?;
        let filtered = if let Some(d) = result_raw.get(TABLE_LABEL)
        { // raw strings to model instances
            d.into_iter() .filter_map(|(pid,row)| {
                let saved_uid:u32 = row.get::<usize>(InMemColIdx::UserId.into())
                    .unwrap() .parse() .unwrap();
                if saved_uid == usr_id {
                    let product_id = pid.parse().unwrap();
                    let auto_cancel_secs = row.get::<usize>(InMemColIdx::AutoCancel.into())
                        .unwrap().parse().unwrap();
                    let warranty_hours = row.get::<usize>(InMemColIdx::Warranty.into())
                        .unwrap().parse().unwrap();
                    let async_stock_chk = row.get::<usize>(InMemColIdx::AsyncStockChk.into())
                        .unwrap().parse().unwrap();
                    Some(ProductPolicyModel {
                        product_id,   auto_cancel_secs,  warranty_hours,
                        async_stock_chk, usr_id:saved_uid,  is_create:false
                    })
                } else {None}
            }) .collect()
        } else { Vec::new() };
        Ok(ProductPolicyModelSet {policies:filtered})
    } // end of fn fetch
   

    async fn save(&self, ppset:ProductPolicyModelSet) -> Result<(), AppError>
    {
        if ppset.policies.is_empty() {
            return Err(AppError {code: AppErrorCode::EmptyInputData,
                detail:Some(format!("save ProductPolicyModel"))  });
        }
        let data = {
            let mut h = HashMap::new();
            let table_data = {
                let kv_pairs = ppset.policies.iter().map(|m|{
                    let mut row:Vec<String> = Vec::with_capacity(InMemColIdx::TotNumColumns.into());
                    row.insert(InMemColIdx::UserId.into(), m.usr_id.to_string());
                    row.insert(InMemColIdx::AutoCancel.into(), m.auto_cancel_secs.to_string());
                    row.insert(InMemColIdx::Warranty.into(), m.warranty_hours.to_string());
                    row.insert(InMemColIdx::AsyncStockChk.into(), m.async_stock_chk.to_string());
                    (m.product_id.to_string(), row)
                });
                HashMap::from_iter(kv_pairs)
            };
            h.insert(TABLE_LABEL.to_string(), table_data);
            h
        };
        let _num_saved = self.datastore.save(data)?;
        Ok(())
    } // end of fn save
} // end of impl AbstProductPolicyRepo

