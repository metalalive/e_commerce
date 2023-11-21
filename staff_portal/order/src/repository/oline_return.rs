use std::boxed::Box;
use std::sync::Arc;
use std::result::Result as DefaultResult;

use async_trait::async_trait;

use crate::AppDataStoreContext;
use crate::datastore::AbstInMemoryDStore;
use crate::error::{AppError, AppErrorCode};
use crate::model::{OrderLineIdentity, OrderReturnModel};
use super::AbsOrderReturnRepo;

mod _oline_return {
    pub(super) const TABLE_LABEL:&'static str = "order_line_return";
}

pub struct OrderReturnInMemRepo {
    datastore: Arc<Box<dyn AbstInMemoryDStore>>,
}

#[async_trait]
impl AbsOrderReturnRepo for OrderReturnInMemRepo
{
    async fn new(ds:Arc<AppDataStoreContext>) -> DefaultResult<Box<dyn AbsOrderReturnRepo>, AppError>
        where Self: Sized
    {
        let obj = Self::build(ds).await ? ;
        Ok(Box::new(obj))
    }
    async fn fetch_by_pid(&self, _oid:&str, _pids:Vec<OrderLineIdentity>)
        -> DefaultResult<Vec<OrderReturnModel>, AppError>
    {
        Ok(vec![])
    }
    async fn create(&self, _oid:&str, reqs:Vec<OrderReturnModel>) -> DefaultResult<usize, AppError>
    {
        Ok(reqs.len())
    }
} // end of OrderReturnInMemRepo

impl OrderReturnInMemRepo {
    pub async fn build(ds:Arc<AppDataStoreContext>) -> DefaultResult<Self, AppError>
    {
        if let Some(m) = ds.in_mem.as_ref() {
            m.create_table(_oline_return::TABLE_LABEL).await?;
            Ok(Self {datastore:m.clone()}) 
        } else {
            Err(AppError {code:AppErrorCode::MissingDataStore,
                detail: Some(format!("in-memory"))}  )
        }
    }
}
