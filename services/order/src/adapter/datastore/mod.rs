mod in_mem;
mod sql_db;

use std::boxed::Box;
use std::sync::Arc;

use ecommerce_common::confidentiality::AbstractConfidentiality;
use ecommerce_common::config::AppDataStoreCfg;
use ecommerce_common::logging::{app_log_event, AppLogContext, AppLogLevel};

pub use in_mem::{
    AbsDStoreFilterKeyOp, AbstInMemoryDStore, AppInMemDeleteInfo, AppInMemDstoreLock,
    AppInMemFetchKeys, AppInMemFetchedData, AppInMemFetchedSingleRow, AppInMemFetchedSingleTable,
    AppInMemUpdateData, AppInMemoryDStore,
};
pub use sql_db::AppMariaDbStore;

pub(crate) fn build_context(
    logctx: Arc<AppLogContext>,
    cfg: &Vec<AppDataStoreCfg>,
    confidential: Arc<Box<dyn AbstractConfidentiality>>,
) -> (
    Option<Box<dyn AbstInMemoryDStore>>,
    Option<Vec<AppMariaDbStore>>,
) {
    let mut inmem = None;
    let mut sqldb = None;
    for c in cfg {
        match c {
            AppDataStoreCfg::InMemory(d) => {
                let item: Box<dyn AbstInMemoryDStore> = Box::new(AppInMemoryDStore::new(d));
                inmem = Some(item);
            }
            AppDataStoreCfg::DbServer(d) => {
                if sqldb.is_none() {
                    sqldb = Some(Vec::new());
                }
                if let Some(lst) = &mut sqldb {
                    match AppMariaDbStore::try_build(d, confidential.clone(), logctx.clone()) {
                        Ok(item) => {
                            lst.push(item);
                        }
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
