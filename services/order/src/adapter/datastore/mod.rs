mod in_mem;
mod sql_db;

use std::boxed::Box;
use std::sync::Arc;

pub use in_mem::{
    AbstInMemoryDStore, AppInMemUpdateData, AppInMemDeleteInfo, AbsDStoreFilterKeyOp,
    AppInMemFetchKeys, AppInMemFetchedData, AppInMemoryDStore, AppInMemDstoreLock,
    AppInMemFetchedSingleTable, AppInMemFetchedSingleRow
};
pub use sql_db::AppMariaDbStore;

use crate::config::AppDataStoreCfg;
use crate::confidentiality::AbstractConfidentiality;
use crate::logging::{AppLogContext, AppLogLevel, app_log_event};

pub(crate) fn build_context(logctx:Arc<AppLogContext>, cfg:&Vec<AppDataStoreCfg>,
                            confidential:Arc<Box<dyn AbstractConfidentiality>> )
    -> (Option<Box<dyn AbstInMemoryDStore>>, Option<Vec<AppMariaDbStore>>)
{
    let mut inmem = None;
    let mut sqldb = None;
    for c in cfg {
        match c {
            AppDataStoreCfg::InMemory(d) => {
                let item:Box<dyn AbstInMemoryDStore> = Box::new(AppInMemoryDStore::new(&d));
                inmem = Some(item);
            },
            AppDataStoreCfg::DbServer(d) => {
                if sqldb.is_none() {
                    sqldb = Some(Vec::new());
                }
                if let Some(lst) = &mut sqldb {
                    match AppMariaDbStore::try_build(&d, confidential.clone())
                    {
                        Ok(item) => {lst.push(item);} ,
                        Err(e) => {
                            app_log_event!(logctx, AppLogLevel::ERROR, "{:?}", e);
                        }
                    }
                    
                }
            }
        }
    }
    (inmem, sqldb)
}
