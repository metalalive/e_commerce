use std::result::Result;
use std::sync::Arc;

use ecommerce_common::logging::AppLogContext;

pub struct AppProcessorError;
pub struct AppProcessorContext;

impl AppProcessorContext {
    pub fn new(_logctx: Arc<AppLogContext>) -> Result<Self, AppProcessorError> {
        Ok(Self)
    }
}
