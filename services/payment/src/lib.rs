pub mod adapter;
pub mod api;
mod auth;
pub mod model;
pub mod network;
pub mod usecase;

use std::result::Result;
use std::sync::Arc;

use ecommerce_common::confidentiality::{self, AbstractConfidentiality};
use ecommerce_common::config::AppConfig;
use ecommerce_common::error::{AppConfidentialityError, AppErrorCode};
use ecommerce_common::logging::AppLogContext;

use crate::adapter::cache::{app_cache_order_sync_lock, AbstractOrderSyncLockCache};
use crate::adapter::datastore::{AppDStoreError, AppDataStoreContext};
use crate::adapter::processor::{
    app_processor_context, AbstractPaymentProcessor, AppProcessorError,
};
use crate::adapter::rpc;
pub use crate::auth::{
    validate_jwt, AbstractAuthKeystore, AppAuthClaimPermission, AppAuthClaimQuota, AppAuthKeystore,
    AppAuthPermissionCode, AppAuthQuotaMatCode, AppAuthedClaim, AppKeystoreRefreshResult,
    AuthJwtError, AuthKeystoreError,
};

pub mod app_meta {
    pub const LABAL: &str = "payment";
    pub const RESOURCE_QUOTA_AP_CODE: u8 = 7;
}
pub mod hard_limit {
    pub const MAX_DB_CONNECTIONS: u32 = 1800u32;
    pub const MAX_SECONDS_DB_IDLE: u16 = 360u16;
    pub const SECONDS_ORDERLINE_DISCARD_MARGIN: u16 = 22u16;
    pub const CREATE_CHARGE_SECONDS_INTERVAL: u16 = 5u16;
}

pub struct AppSharedState {
    _config: Arc<AppConfig>,
    _log_ctx: Arc<AppLogContext>,
    _dstore: Arc<AppDataStoreContext>,
    _processors: Arc<Box<dyn AbstractPaymentProcessor>>,
    _rpc_ctx: Arc<Box<dyn rpc::AbstractRpcContext>>,
    _ordersync_lockset: Arc<Box<dyn AbstractOrderSyncLockCache>>,
    _auth_keys: Arc<Box<dyn AbstractAuthKeystore<Error = AuthKeystoreError>>>,
}

#[derive(Debug)]
pub enum ShrStateInitProgress {
    Confidentiality(AppErrorCode, String),
    DataStore,
    RpcContext,
    ExternalProcessor,
    AuthKeyStore(AuthKeystoreError),
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
        //println!("[ERROR] share-state, init rpc context : {:?}", _value);
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
impl From<AuthKeystoreError> for ShrStateInitError {
    fn from(detail: AuthKeystoreError) -> Self {
        Self {
            progress: ShrStateInitProgress::AuthKeyStore(detail),
        }
    }
}
impl From<AppConfidentialityError> for ShrStateInitError {
    fn from(value: AppConfidentialityError) -> Self {
        Self {
            progress: ShrStateInitProgress::Confidentiality(value.code, value.detail),
        }
    }
}

impl AppSharedState {
    pub fn new(cfg: AppConfig) -> Result<Self, ShrStateInitError> {
        let logctx = {
            let lc = AppLogContext::new(&cfg.basepath, &cfg.api_server.logging);
            Arc::new(lc)
        };
        let cfdntl: Arc<Box<dyn AbstractConfidentiality>> = {
            let c = confidentiality::build_context(&cfg)?;
            Arc::new(c)
        };
        let rpc_ctx = rpc::build_context(&cfg.api_server.rpc, cfdntl.clone(), logctx.clone())?;
        let dstore = AppDataStoreContext::new(&cfg.api_server.data_store, cfdntl, logctx.clone())?;
        let _processors = app_processor_context(logctx.clone())?;
        let ordersync_lockset = app_cache_order_sync_lock();
        let auth_keys = AppAuthKeystore::try_create(&cfg.api_server.auth)?;
        Ok(Self {
            _config: Arc::new(cfg),
            _log_ctx: logctx,
            _ordersync_lockset: Arc::new(ordersync_lockset),
            _dstore: Arc::new(dstore),
            _rpc_ctx: Arc::new(rpc_ctx),
            _processors: Arc::new(_processors),
            _auth_keys: Arc::new(Box::new(auth_keys)),
        })
    } // end of fn new

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
    pub fn auth_keystore(&self) -> Arc<Box<dyn AbstractAuthKeystore<Error = AuthKeystoreError>>> {
        self._auth_keys.clone()
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
            _auth_keys: self._auth_keys.clone(),
        }
    }
}
