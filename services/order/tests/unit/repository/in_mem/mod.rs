pub(super) mod currency;
pub(super) mod oorder;
pub(super) mod product_policy;
mod product_price;

use async_trait::async_trait;
use std::boxed::Box;
use std::sync::Arc;

use ecommerce_common::error::AppErrorCode;

use order::datastore::{
    AbsDStoreFilterKeyOp, AbstInMemoryDStore, AppInMemDeleteInfo, AppInMemDstoreLock,
    AppInMemFetchKeys, AppInMemFetchedData, AppInMemUpdateData,
};
use order::error::AppError;
use order::{AppDataStoreContext, AppInMemoryDbCfg};

fn in_mem_ds_ctx_setup<T: AbstInMemoryDStore + 'static>(
    max_items: u32,
) -> Arc<AppDataStoreContext> {
    let d = AppInMemoryDbCfg {
        alias: format!("utest"),
        max_items,
    };
    let obj = T::new(&d);
    let obj: Box<dyn AbstInMemoryDStore> = Box::new(obj);
    let inmem_ds = Arc::new(obj);
    Arc::new(AppDataStoreContext {
        sql_dbs: None,
        in_mem: Some(inmem_ds),
    })
}
struct MockInMemDeadDataStore {}

#[async_trait]
impl AbstInMemoryDStore for MockInMemDeadDataStore {
    fn new(_cfg: &AppInMemoryDbCfg) -> Self
    where
        Self: Sized,
    {
        Self {}
    }
    async fn fetch(&self, _info: AppInMemFetchKeys) -> Result<AppInMemFetchedData, AppError> {
        Err(AppError {
            code: AppErrorCode::AcquireLockFailure,
            detail: Some(format!("utest")),
        })
    }
    async fn fetch_acquire<'a>(
        &'a self,
        _info: AppInMemFetchKeys,
    ) -> Result<(AppInMemFetchedData, AppInMemDstoreLock<'a>), AppError> {
        Err(AppError {
            code: AppErrorCode::AcquireLockFailure,
            detail: Some(format!("utest")),
        })
    }
    async fn delete(&self, _info: AppInMemDeleteInfo) -> Result<usize, AppError> {
        Err(AppError {
            code: AppErrorCode::NotImplemented,
            detail: Some(format!("utest")),
        })
    }
    async fn create_table(&self, _label: &str) -> Result<(), AppError> {
        Ok(())
    }
    async fn save(&self, _data: AppInMemUpdateData) -> Result<usize, AppError> {
        Err(AppError {
            code: AppErrorCode::DataTableNotExist,
            detail: Some(format!("utest")),
        })
    }
    fn save_release(
        &self,
        _data: AppInMemUpdateData,
        _lock: AppInMemDstoreLock,
    ) -> Result<usize, AppError> {
        Err(AppError {
            code: AppErrorCode::NotImplemented,
            detail: Some(format!("utest")),
        })
    }
    async fn filter_keys(
        &self,
        _tbl_label: String,
        _op: &dyn AbsDStoreFilterKeyOp,
    ) -> Result<Vec<String>, AppError> {
        Err(AppError {
            code: AppErrorCode::NotImplemented,
            detail: Some(format!("utest")),
        })
    }
}
