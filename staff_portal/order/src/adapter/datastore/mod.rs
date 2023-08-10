mod in_mem;
mod sql_db;

use std::boxed::Box;

pub use in_mem::{
    AbstInMemoryDStore, AppInMemUpdateData, AppInMemDeleteInfo,
    AppInMemFetchKeys, AppInMemFetchedData, AppInMemoryDStore
};
pub use sql_db::AppSqlDbStore;

use crate::config::AppDataStoreCfg;

pub(crate) fn build_context(cfg:&Vec<AppDataStoreCfg>)
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
                    let item = AppSqlDbStore::new(&d);
                    lst.push(item);
                }
            }
        }
    }
    (inmem, sqldb)
}
