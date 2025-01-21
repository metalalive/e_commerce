mod in_mem;
#[cfg(feature = "mariadb")]
mod sql_db;

use std::boxed::Box;
use std::result::Result as DefaultResult;
use std::sync::Arc;

use ecommerce_common::confidentiality::AbstractConfidentiality;
use ecommerce_common::config::{AppDataStoreCfg, AppDbServerCfg};
#[cfg(not(feature = "mariadb"))]
use ecommerce_common::error::AppErrorCode;
use ecommerce_common::logging::{app_log_event, AppLogContext, AppLogLevel};

use crate::error::AppError;
pub use in_mem::{
    AbsDStoreFilterKeyOp, AbstInMemoryDStore, AppInMemDeleteInfo, AppInMemDstoreLock,
    AppInMemFetchKeys, AppInMemFetchedData, AppInMemFetchedSingleRow, AppInMemFetchedSingleTable,
    AppInMemUpdateData, AppInMemoryDStore,
};
#[cfg(feature = "mariadb")]
pub use sql_db::AppMariaDbStore;

#[cfg(not(feature = "mariadb"))]
pub struct AppMariaDbStore {}

#[cfg(not(feature = "mariadb"))]
impl AppMariaDbStore {
    pub fn try_build(
        cfg: &AppDbServerCfg,
        _confidential: Arc<Box<dyn AbstractConfidentiality>>,
        _logctx: Arc<AppLogContext>,
    ) -> DefaultResult<Self, AppError> {
        let detail = format!(
            "sql-db, type:{:?}, alias:{}",
            cfg.srv_type,
            cfg.alias.as_str()
        );
        Err(AppError {
            code: AppErrorCode::FeatureDisabled,
            detail: Some(detail),
        })
    }
} // end of impl AppMariaDbStore

#[allow(clippy::type_complexity)]
pub(crate) fn build_context(
    logctx: Arc<AppLogContext>,
    cfg: &Vec<AppDataStoreCfg>,
    confidential: Arc<Box<dyn AbstractConfidentiality>>,
) -> DefaultResult<
    (
        Option<Box<dyn AbstInMemoryDStore>>,
        Option<Vec<AppMariaDbStore>>,
    ),
    Vec<AppError>,
> {
    let mut inmem = None;
    let mut sqldb = None;
    let mut errors = Vec::new();
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
                            errors.push(e);
                        }
                    }
                }
            }
        }
    }
    if errors.is_empty() {
        Ok((inmem, sqldb))
    } else {
        Err(errors)
    }
}
