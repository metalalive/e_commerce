use std::sync::Arc;
use std::boxed::Box;
use async_trait::async_trait;

use crate::AppDataStoreContext;
use crate::datastore::AppInMemoryDStore;
use crate::model::ProductPolicyModelSet;
use crate::error::{AppError, AppErrorCode};
use super::AbstProductPolicyRepo;

const TABLE_LABEL: &'static str = "product_policy";

pub struct ProductPolicyInMemRepo
{
    datastore: Arc<AppInMemoryDStore>
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

    async fn fetch(&self, usr_id:u32, _ids:Vec<u64>) -> Result<ProductPolicyModelSet, AppError>
    {
        let result_set = ProductPolicyModelSet {usr_id, policies:Vec::new()};
        Ok(result_set)
    }
    
    async fn save(&self, _updated:ProductPolicyModelSet) -> Result<(), AppError>
    {
        Ok(())
    }
} // end of impl AbstProductPolicyRepo

