mod in_mem;
mod sql_db;

use std::boxed::Box;
use std::sync::Arc;

pub use in_mem::{
    AbstInMemoryDStore, AppInMemUpdateData, AppInMemDeleteInfo, AbsDStoreFilterKeyOp,
    AppInMemFetchKeys, AppInMemFetchedData, AppInMemoryDStore, AppInMemDstoreLock,
    AppInMemFetchedSingleTable
};
pub use sql_db::AppSqlDbStore;

use crate::config::AppDataStoreCfg;
use crate::confidentiality::AbstractConfidentiality;

pub(crate) fn build_context(cfg:&Vec<AppDataStoreCfg>,
                            confidential:Arc<Box<dyn AbstractConfidentiality>> )
    -> (Option<Box<dyn AbstInMemoryDStore>>, Option<Vec<AppSqlDbStore>>)
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
                    let item = AppSqlDbStore::new(&d, confidential.clone());
                    lst.push(item);
                }
            }
        }
    }
    (inmem, sqldb)
}
