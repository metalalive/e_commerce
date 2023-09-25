use std::boxed::Box;
use std::sync::Arc;
use std::result::Result as DefaultResult;

use crate::AppDataStoreContext;
use crate::datastore::AbstInMemoryDStore;
use crate::error::{AppError, AppErrorCode};

use super::AbsOrderRepo;


pub struct OrderInMemRepo {
    datastore:Arc<Box<dyn AbstInMemoryDStore>>
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
}

impl OrderInMemRepo {
    fn build(ds:Arc<AppDataStoreContext>) -> DefaultResult<Self, AppError>
    {
        if let Some(ds) = &ds.in_mem {
            let obj = Self{datastore:ds.clone()};
            Ok(obj)
        } else {
            Err(AppError {code:AppErrorCode::MissingDataStore,
                detail: Some(format!("in-memory"))}  )
        }
    }
}
