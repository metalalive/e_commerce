pub mod adapter;
pub mod api;
pub mod model;
pub mod network;
pub mod usecase;

use std::result::Result;
use std::sync::Arc;

use ecommerce_common::config::AppConfig;
use ecommerce_common::logging::AppLogContext;

use crate::adapter::cache::{app_cache_order_sync_lock, AbstractOrderSyncLockCache};
use crate::adapter::datastore::{AppDStoreError, AppDataStoreContext};
use crate::adapter::processor::{
    app_processor_context, AbstractPaymentProcessor, AppProcessorError,
};
use crate::adapter::rpc;

pub mod hard_limit {
    pub const MAX_DB_CONNECTIONS: u32 = 1800u32;
    pub const MAX_SECONDS_DB_IDLE: u16 = 360u16;
    pub const SECONDS_ORDERLINE_DISCARD_MARGIN: u16 = 22u16;
}

pub struct AppSharedState {
    _config: Arc<AppConfig>,
    _log_ctx: Arc<AppLogContext>,
    _dstore: Arc<AppDataStoreContext>,
    _processors: Arc<Box<dyn AbstractPaymentProcessor>>,
    _rpc_ctx: Arc<Box<dyn rpc::AbstractRpcContext>>,
    _ordersync_lockset: Arc<Box<dyn AbstractOrderSyncLockCache>>,
}

#[derive(Debug)]
pub enum ShrStateInitProgress {
    DataStore,
    RpcContext,
    ExternalProcessor,
}

// TODO,
// - error code with  ecommerce_common::error::AppErrorCode;
#[derive(Debug)]
pub struct ShrStateInitError {
    pub progress: ShrStateInitProgress,
}
impl From<AppDStoreError> for ShrStateInitError {
    fn from(_value: AppDStoreError) -> Self {
        Self {
            progress: ShrStateInitProgress::DataStore,
        }
    }
}
impl From<rpc::AppRpcCtxError> for ShrStateInitError {
    fn from(_value: rpc::AppRpcCtxError) -> Self {
        Self {
            progress: ShrStateInitProgress::RpcContext,
        }
    }
}
impl From<AppProcessorError> for ShrStateInitError {
    fn from(_value: AppProcessorError) -> Self {
        Self {
            progress: ShrStateInitProgress::ExternalProcessor,
        }
    }
}

impl AppSharedState {
    pub fn new(cfg: AppConfig) -> Result<Self, ShrStateInitError> {
        let logctx = {
            let lc = AppLogContext::new(&cfg.basepath, &cfg.api_server.logging);
            Arc::new(lc)
        };
        let _rpc_ctx = {
            let r = rpc::build_context(&cfg.api_server.rpc, logctx.clone())?;
            Arc::new(r)
        };
        let _dstore = {
            let d = AppDataStoreContext::new(&cfg.api_server.data_store, logctx.clone())?;
            Arc::new(d)
        };
        let _processors = {
            let proc = app_processor_context(logctx.clone())?;
            Arc::new(proc)
        };
        let _ordersync_lockset = {
            let ols = app_cache_order_sync_lock();
            Arc::new(ols)
        };
        Ok(Self {
            _config: Arc::new(cfg),
            _log_ctx: logctx,
            _ordersync_lockset,
            _dstore,
            _rpc_ctx,
            _processors,
        })
    }

    pub fn datastore(&self) -> Arc<AppDataStoreContext> {
        self._dstore.clone()
    }
    pub fn processor_context(&self) -> Arc<Box<dyn AbstractPaymentProcessor>> {
        self._processors.clone()
    }
    pub fn rpc_context(&self) -> Arc<Box<dyn rpc::AbstractRpcContext>> {
        self._rpc_ctx.clone()
    }
    pub fn ordersync_lockset(&self) -> Arc<Box<dyn AbstractOrderSyncLockCache>> {
        self._ordersync_lockset.clone()
    }
    pub fn log_context(&self) -> Arc<AppLogContext> {
        self._log_ctx.clone()
    }
    pub fn config(&self) -> Arc<AppConfig> {
        self._config.clone()
    }
} // end of impl AppSharedState

impl Clone for AppSharedState {
    fn clone(&self) -> Self {
        Self {
            _config: self._config.clone(),
            _log_ctx: self._log_ctx.clone(),
            _dstore: self._dstore.clone(),
            _rpc_ctx: self._rpc_ctx.clone(),
            _processors: self._processors.clone(),
            _ordersync_lockset: self._ordersync_lockset.clone(),
        }
    }
}
