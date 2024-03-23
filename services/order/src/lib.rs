use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32};

use uuid::{Uuid, Builder, Timestamp, NoContext};

pub mod api;
pub mod error;
pub mod logging;
pub mod network;
pub mod constant;
pub mod usecase;
pub mod repository;
pub mod model;
pub mod confidentiality;

mod config;
pub use config::{
    AppConfig, ApiServerCfg, WebApiListenCfg, WebApiRouteCfg, AppLoggingCfg,
    AppLogHandlerCfg, AppLoggerCfg, AppBasepathCfg, AppRpcCfg, AppRpcAmqpCfg,
    AppInMemoryDbCfg, AppConfidentialCfg, AppAuthCfg
};

mod auth;
pub use auth::{
    AbstractAuthKeystore, AppAuthKeystore, AppJwtAuthentication, AppKeystoreRefreshResult,
    AppAuthedClaim, AppAuthClaimQuota, AppAuthClaimPermission, AppAuthQuotaMatCode,
    AppAuthPermissionCode
};

mod rpc;
pub use rpc::{
    AbstractRpcContext, AbsRpcServerCtx, AbsRpcClientCtx,  AbstractRpcClient,
    AppRpcRouteHdlrFn, AppRpcReply, AppRpcClientReqProperty
};

mod adapter;
pub use adapter::datastore;

use confidentiality::AbstractConfidentiality;

type WebApiPath = String;
type WebApiHdlrLabel = & 'static str;
type AppLogAlias = Arc<String>;

pub struct AppDataStoreContext {
    pub in_mem: Option<Arc<Box<dyn datastore::AbstInMemoryDStore>>>,
    pub sql_dbs: Option<Vec<Arc<datastore::AppMariaDbStore>>>
} // TODO, rename sql_dbs

// global state shared by all threads
pub struct AppSharedState {
    _cfg: Arc<AppConfig>,
    _log: Arc<logging::AppLogContext>,
    _rpc: Arc<Box<dyn AbstractRpcContext>>,
    dstore: Arc<AppDataStoreContext>,
    _auth_keys: Arc<Box<dyn AbstractAuthKeystore>>,
    _shutdown: Arc<AtomicBool>,
    _num_reqs_processing : Arc<AtomicU32>,
}

impl AppSharedState {
    pub fn new(cfg:AppConfig, log:logging::AppLogContext,
               confidential:Box<dyn AbstractConfidentiality>)
        -> Self
    { // TODO
      // - should return error
      // - confidential argument to arc-box pointer
        let confidential = Arc::new(confidential);
        let log = Arc::new(log);
        let _rpc_ctx = rpc::build_context(&cfg.api_server.rpc,
                            log.clone(), confidential.clone()).unwrap();
        let (in_mem, sql_dbs) = datastore::build_context(log.clone(),
                                &cfg.api_server.data_store, confidential);
        let in_mem = if let Some(m) = in_mem { Some(Arc::new(m)) } else {None};
        let sql_dbs = if let Some(m) = sql_dbs {
            Some(m.into_iter().map(Arc::new).collect())
        } else {None};
        let ds_ctx = Arc::new(AppDataStoreContext {in_mem, sql_dbs});
        let auth_keys = AppAuthKeystore::new(&cfg.api_server.auth);
        Self{_cfg:Arc::new(cfg), _log:log, _rpc:Arc::new(_rpc_ctx),
             dstore: ds_ctx, _auth_keys: Arc::new(Box::new(auth_keys)),
             _shutdown: Arc::new(AtomicBool::new(false)),
             _num_reqs_processing: Arc::new(AtomicU32::new(0))
        }
    } // end of fn new

    pub fn config(&self) -> &Arc<AppConfig>
    { &self._cfg }

    pub fn log_context(&self) -> &Arc<logging::AppLogContext>
    { &self._log }
    
    pub fn rpc(&self) -> Arc<Box<dyn AbstractRpcContext>>
    { self._rpc.clone() }

    pub fn datastore(&self) -> Arc<AppDataStoreContext>
    { self.dstore.clone() }

    pub fn auth_keystore(&self) -> Arc<Box<dyn AbstractAuthKeystore>>
    { self._auth_keys.clone() }

    pub fn shutdown(&self) -> Arc<AtomicBool>
    { self._shutdown.clone() }
    
    /// return atomic field which represents current number of processing requests
    pub fn num_requests(&self) -> Arc<AtomicU32>
    { self._num_reqs_processing.clone() }
} // end of impl AppSharedState

impl Clone for AppSharedState {
    fn clone(&self) -> Self {
        Self {
            _cfg: self._cfg.clone(),   _log: self._log.clone(),
            _rpc: self._rpc.clone(),   dstore: self.dstore.clone(),
            _auth_keys: self._auth_keys.clone(),
            _shutdown: self._shutdown.clone(),
            _num_reqs_processing: self._num_reqs_processing.clone(),
        }
    }
}

fn generate_custom_uid(machine_code:u8) -> Uuid
{
    // UUIDv7 is for single-node application. This app needs to consider
    // scalability of multi-node environment, UUIDv8 can be utilized cuz it
    // allows custom ID layout, so few bits of the ID can be assigned to
    // represent each machine/node ID,  rest of that should be timestamp with
    // random byte sequence
    let ts_ctx = NoContext;
    let (secs, nano) = Timestamp::now(ts_ctx).to_unix();
    let millis = (secs * 1000).saturating_add((nano as u64) / 1_000_000);
    let mut node_id = rand::random::<[u8;10]>();
    node_id[0] = machine_code;
    let builder = Builder::from_unix_timestamp_millis(millis, &node_id);
    builder.into_uuid()
}
