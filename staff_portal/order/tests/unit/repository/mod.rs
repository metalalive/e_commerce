mod product_policy;
mod product_price;
mod oorder;

use std::sync::Arc;
use std::boxed::Box;

use order::error::{AppErrorCode, AppError};
use order::{AppDataStoreContext, AppInMemoryDbCfg};
use order::datastore::{
    AbstInMemoryDStore, AppInMemUpdateData, AppInMemFetchKeys, AppInMemDstoreLock,
    AppInMemFetchedData, AppInMemDeleteInfo, AbsDStoreFilterKeyOp
};

fn in_mem_ds_ctx_setup<T: AbstInMemoryDStore + 'static> (max_items:u32)
    -> Arc<AppDataStoreContext>
{
    let d = AppInMemoryDbCfg { alias:format!("utest") , max_items };
    let obj = T::new(&d);
    let obj:Box<dyn AbstInMemoryDStore> = Box::new(obj);
    let inmem_ds = Arc::new(obj);
    Arc::new(AppDataStoreContext{ sql_dbs:None,
        in_mem:Some(inmem_ds) })
}
struct MockInMemDeadDataStore {}

impl AbstInMemoryDStore for MockInMemDeadDataStore {
    fn new(_cfg:&AppInMemoryDbCfg) -> Self where Self:Sized
    { Self{} }
    fn fetch(&self, _info: AppInMemFetchKeys) -> Result<AppInMemFetchedData, AppError> {
        Err(AppError { code: AppErrorCode::AcquireLockFailure, detail:Some(format!("utest")) }) 
    }
    fn fetch_acquire(&self, _info:AppInMemFetchKeys)
            -> Result<(AppInMemFetchedData, AppInMemDstoreLock), AppError>
    { 
        Err(AppError { code: AppErrorCode::AcquireLockFailure, detail:Some(format!("utest")) }) 
    }
    fn delete(&self, _info:AppInMemDeleteInfo) -> Result<usize, AppError> {
        Err(AppError { code: AppErrorCode::NotImplemented, detail:Some(format!("utest")) })
    }
    fn create_table (&self, _label:&str) -> Result<(), AppError> {
        Ok(())
    }
    fn save(&self, _data:AppInMemUpdateData) -> Result<usize, AppError> {
        Err(AppError { code: AppErrorCode::DataTableNotExist, detail:Some(format!("utest")) })
    }
    fn save_release(&self, _data:AppInMemUpdateData, _lock: AppInMemDstoreLock)
            -> Result<usize, AppError>
    {
        Err(AppError { code: AppErrorCode::NotImplemented, detail:Some(format!("utest")) })
    }
    fn filter_keys(&self, _tbl_label:String, _op:&dyn AbsDStoreFilterKeyOp)
        -> Result<Vec<String>, AppError>
    {
        Err(AppError { code: AppErrorCode::NotImplemented, detail:Some(format!("utest")) })
    }
}

