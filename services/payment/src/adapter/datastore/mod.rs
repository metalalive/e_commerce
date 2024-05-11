use std::result::Result;
use std::sync::Arc;

use ecommerce_common::config::AppDataStoreCfg;
use ecommerce_common::logging::AppLogContext;

pub struct AppDStoreError;
pub struct AppDataStoreContext;

impl AppDataStoreContext {
    pub fn new(
        _cfg: &[AppDataStoreCfg],
        _logctx: Arc<AppLogContext>,
    ) -> Result<Self, AppDStoreError> {
        Ok(Self)
    }
}
