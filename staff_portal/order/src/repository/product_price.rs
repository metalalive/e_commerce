use std::sync::Arc;
use std::boxed::Box;
use std::result::Result as DefaultResult;

use async_trait::async_trait;

use crate::AppDataStoreContext;
use crate::api::rpc::dto::ProductPriceDeleteDto;
use crate::error::{AppError, AppErrorCode};
use crate::model::ProductPriceModelSet;
use super::AbsProductPriceRepo;

const TABLE_LABEL: &'static str = "product_price";

pub struct ProductPriceInMemRepo {
    datastore: Arc<AppDataStoreContext>
}

#[async_trait]
impl AbsProductPriceRepo for ProductPriceInMemRepo {
    fn new(dstore:Arc<AppDataStoreContext>) -> DefaultResult<Box<dyn AbsProductPriceRepo>, AppError>
        where Self:Sized
    {
        match Self::_new(dstore) {
            Ok(rp) => Ok(Box::new(rp)),
            Err(e) => Err(e)
        }
    }
    async fn delete_all(&self, store_id:u32) -> Result<(), AppError>
    { Ok(()) }
    async fn delete(&self, store_id:u32, ids:ProductPriceDeleteDto) -> Result<(), AppError>
    { Ok(()) }
    async fn fetch(&self, store_id:u32, ids:Vec<(u8,u64)>) -> Result<ProductPriceModelSet, AppError>
    {
        let items = Vec::new();
        let obj = ProductPriceModelSet {items:items};
        Ok(obj)
    }
    async fn save(&self, updated:ProductPriceModelSet) -> Result<(), AppError>
    { Ok(()) }
} // end of impl ProductPriceInMemRepo

impl ProductPriceInMemRepo {
    pub fn _new(dstore:Arc<AppDataStoreContext>) -> DefaultResult<Self, AppError>
        where Self:Sized
    {
        if let Some(m) = &dstore.in_mem {
            m.create_table(TABLE_LABEL)?;
            let obj = Self { datastore: dstore.clone() };
            Ok(obj)
        } else {
            Err(AppError {code:AppErrorCode::MissingDataStore,
                detail: Some(format!("in-memory"))}  )
        }
    }
} // end of impl ProductPriceInMemRepo
