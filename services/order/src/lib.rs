use std::sync::atomic::{AtomicBool, AtomicU32};
use std::sync::Arc;

use uuid::{Builder, NoContext, Timestamp, Uuid};

pub mod api;
pub mod constant;
pub mod error;
pub mod model;
pub mod network;
pub mod repository;
pub mod usecase;

use ecommerce_common::confidentiality::AbstractConfidentiality;
pub use ecommerce_common::config::{
    ApiServerCfg, AppAuthCfg, AppBasepathCfg, AppCfgHardLimit, AppCfgInitArgs, AppConfidentialCfg,
    AppConfig, AppInMemoryDbCfg, AppLogHandlerCfg, AppLoggerCfg, AppLoggingCfg, AppRpcAmqpCfg,
    AppRpcCfg, WebApiListenCfg, WebApiRouteCfg,
};
use ecommerce_common::logging::AppLogContext;

mod auth;
pub use auth::{
    AbstractAuthKeystore, AppAuthClaimPermission, AppAuthClaimQuota, AppAuthKeystore,
    AppAuthPermissionCode, AppAuthQuotaMatCode, AppAuthedClaim, AppJwtAuthentication,
    AppKeystoreRefreshResult,
};

mod rpc;
pub use rpc::{
    AbsRpcClientCtx, AbsRpcServerCtx, AbstractRpcClient, AbstractRpcContext,
    AppRpcClientReqProperty, AppRpcReply, AppRpcRouteHdlrFn,
};

mod adapter;
pub use adapter::datastore;
use adapter::thirdparty::{app_currency_context, AbstractCurrencyExchange};

type WebApiHdlrLabel = &'static str;

pub struct AppDataStoreContext {
    pub in_mem: Option<Arc<Box<dyn datastore::AbstInMemoryDStore>>>,
    pub sql_dbs: Option<Vec<Arc<datastore::AppMariaDbStore>>>,
} // TODO, rename sql_dbs

// global state shared by all threads
pub struct AppSharedState {
    _cfg: Arc<AppConfig>,
    _log: Arc<AppLogContext>,
    _rpc: Arc<Box<dyn AbstractRpcContext>>,
    dstore: Arc<AppDataStoreContext>,
    _auth_keys: Arc<Box<dyn AbstractAuthKeystore>>,
    _currency_ex: Arc<Box<dyn AbstractCurrencyExchange>>,
    _shutdown: Arc<AtomicBool>,
    _num_reqs_processing: Arc<AtomicU32>,
}

impl AppSharedState {
    pub fn new(
        cfg: AppConfig,
        log: AppLogContext,
        confidential: Box<dyn AbstractConfidentiality>,
    ) -> Self {
        // TODO
        // - should return error
        // - confidential argument to arc-box pointer
        let confidential = Arc::new(confidential);
        let log = Arc::new(log);
        let _rpc_ctx =
            rpc::build_context(&cfg.api_server.rpc, log.clone(), confidential.clone()).unwrap();
        let (in_mem, sql_dbs) = datastore::build_context(
            log.clone(),
            &cfg.api_server.data_store,
            confidential.clone(),
        )
        .unwrap();
        let in_mem = in_mem.map(Arc::new);
        let sql_dbs = sql_dbs.map(|m| m.into_iter().map(Arc::new).collect());
        let ds_ctx = Arc::new(AppDataStoreContext { in_mem, sql_dbs });
        let auth_keys = AppAuthKeystore::new(&cfg.api_server.auth);
        let currency_ex = app_currency_context(
            &cfg.basepath,
            &cfg.api_server.third_parties,
            confidential,
            log.clone(),
        )
        .unwrap();
        Self {
            _cfg: Arc::new(cfg),
            _log: log,
            _rpc: Arc::new(_rpc_ctx),
            dstore: ds_ctx,
            _auth_keys: Arc::new(Box::new(auth_keys)),
            _currency_ex: Arc::new(currency_ex),
            _shutdown: Arc::new(AtomicBool::new(false)),
            _num_reqs_processing: Arc::new(AtomicU32::new(0)),
        }
    } // end of fn new

    pub fn config(&self) -> &Arc<AppConfig> {
        &self._cfg
    }

    pub fn log_context(&self) -> &Arc<AppLogContext> {
        &self._log
    }

    pub fn rpc(&self) -> Arc<Box<dyn AbstractRpcContext>> {
        self._rpc.clone()
    }

    pub fn datastore(&self) -> Arc<AppDataStoreContext> {
        self.dstore.clone()
    }

    pub fn auth_keystore(&self) -> Arc<Box<dyn AbstractAuthKeystore>> {
        self._auth_keys.clone()
    }

    pub fn currency(&self) -> Arc<Box<dyn AbstractCurrencyExchange>> {
        self._currency_ex.clone()
    }

    pub fn shutdown(&self) -> Arc<AtomicBool> {
        self._shutdown.clone()
    }

    /// return atomic field which represents current number of processing requests
    pub fn num_requests(&self) -> Arc<AtomicU32> {
        self._num_reqs_processing.clone()
    }
} // end of impl AppSharedState

impl Clone for AppSharedState {
    fn clone(&self) -> Self {
        Self {
            _cfg: self._cfg.clone(),
            _log: self._log.clone(),
            _rpc: self._rpc.clone(),
            dstore: self.dstore.clone(),
            _auth_keys: self._auth_keys.clone(),
            _currency_ex: self._currency_ex.clone(),
            _shutdown: self._shutdown.clone(),
            _num_reqs_processing: self._num_reqs_processing.clone(),
        }
    }
}

fn generate_custom_uid(machine_code: u8) -> Uuid {
    // UUIDv7 is for single-node application. This app needs to consider
    // scalability of multi-node environment, UUIDv8 can be utilized cuz it
    // allows custom ID layout, so few bits of the ID can be assigned to
    // represent each machine/node ID,  rest of that should be timestamp with
    // random byte sequence
    let ts_ctx = NoContext;
    let (secs, nano) = Timestamp::now(ts_ctx).to_unix();
    let millis = (secs * 1000).saturating_add((nano as u64) / 1_000_000);
    let mut node_id = rand::random::<[u8; 10]>();
    node_id[0] = machine_code;
    let builder = Builder::from_unix_timestamp_millis(millis, &node_id);
    builder.into_uuid()
}
