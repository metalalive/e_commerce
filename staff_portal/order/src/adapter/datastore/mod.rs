mod in_mem;
mod sql_db;

pub use in_mem::{
    AppInMemoryDStore, AppInMemUpdateData, AppInMemDeleteInfo,
    AppInMemFetchKeys, AppInMemFetchedData
};
pub use sql_db::AppSqlDbStore;

use crate::config::AppDataStoreCfg;

pub(crate) fn build_context(cfg:&Vec<AppDataStoreCfg>)
    -> (Option<AppInMemoryDStore>, Option<Vec<AppSqlDbStore>>)
{
    let mut inmem = None;
    let mut sqldb = None;
    for c in cfg {
        match c {
            AppDataStoreCfg::InMemory(d) => {
                inmem = Some(AppInMemoryDStore::new(&d));
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
