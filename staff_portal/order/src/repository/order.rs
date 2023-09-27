use std::boxed::Box;
use std::sync::Arc;
use std::result::Result as DefaultResult;

use async_trait::async_trait;

use crate::AppDataStoreContext;
use crate::datastore::AbstInMemoryDStore;
use crate::error::{AppError, AppErrorCode};
use crate::model::StockLevelModelSet;

use super::{AbsOrderRepo, AbsOrderStockRepo};

struct StockLvlInMemRepo {
    datastore: Arc<Box<dyn AbstInMemoryDStore>>,
}
pub struct OrderInMemRepo {
    _stock: Arc<Box<dyn AbsOrderStockRepo>>
}

#[async_trait]
impl AbsOrderStockRepo for StockLvlInMemRepo {
    async fn fetch(&self, pids:Vec<(u32,u8,u64)>) -> DefaultResult<StockLevelModelSet, AppError>
    { Ok(StockLevelModelSet {}) }
    
    async fn save(&self, slset:StockLevelModelSet) -> DefaultResult<(), AppError>
    { Ok(()) }
}

impl AbsOrderRepo for OrderInMemRepo {
    fn new(ds:Arc<AppDataStoreContext>) -> DefaultResult<Box<dyn AbsOrderRepo>, AppError>
        where Self:Sized
    {
        match Self::build(ds) {
            Ok(obj) => Ok(Box::new(obj)),
            Err(e) => Err(e)
        }
    }
    fn stock(&self) -> Arc<Box<dyn AbsOrderStockRepo>>
    { self._stock.clone() }
} // end of impl AbsOrderRepo

impl OrderInMemRepo {
    fn build(ds:Arc<AppDataStoreContext>) -> DefaultResult<Self, AppError>
    {
        if let Some(ds) = &ds.in_mem {
            let _stock = StockLvlInMemRepo {datastore: ds.clone()};
            let obj = Self{_stock:Arc::new(Box::new(_stock))};
            Ok(obj)
        } else {
            Err(AppError {code:AppErrorCode::MissingDataStore,
                detail: Some(format!("in-memory"))}  )
        }
    }
}
