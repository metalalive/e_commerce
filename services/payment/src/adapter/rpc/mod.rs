use std::boxed::Box;
use std::marker::{Send, Sync};
use std::result::Result;
use std::sync::Arc;

use ecommerce_common::config::AppRpcCfg;
use ecommerce_common::logging::AppLogContext;

pub trait AbstractRpcContext: Sync + Send {}

pub(crate) struct AppRpcCtxError;
pub(crate) struct AppDummyRpcContext;

impl AbstractRpcContext for AppDummyRpcContext {}

pub(crate) fn build_context(
    _cfg: &AppRpcCfg,
    _logctx: Arc<AppLogContext>,
) -> Result<Box<dyn AbstractRpcContext>, AppRpcCtxError> {
    let obj = AppDummyRpcContext;
    Ok(Box::new(obj))
}
