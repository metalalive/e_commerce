use std::sync::Arc;

pub mod api;
pub mod error;
pub mod logging;
pub mod network;
pub mod constant;
pub mod usecase;
pub mod repository;
pub mod model;

mod config;
pub use config::{
    AppConfig, ApiServerCfg, ApiServerListenCfg, ApiServerRouteCfg,
    AppLoggingCfg, AppLogHandlerCfg, AppLoggerCfg, AppBasepathCfg,
    AppRpcCfg, AppRpcTypeCfg, AppInMemoryDbCfg
};

mod rpc;
use rpc::build_context as build_rpc_context;
pub use rpc::{
    AbstractRpcContext, AbstractRpcHandler, AppRpcPublishedResult,
    AppRpcReplyResult, AppRpcPublishProperty, AppRpcReplyProperty
};

mod adapter;
pub use adapter::datastore;

type WebApiPath = String;
type WebApiHdlrLabel = & 'static str;
type AppLogAlias = Arc<String>;

pub struct AppDataStoreContext {
    in_mem: Option<Arc<datastore::AppInMemoryDStore>>,
    sql_dbs: Option<Vec<Arc<datastore::AppSqlDbStore>>>
}

// global state shared by all threads
pub struct AppSharedState {
    _cfg: Arc<AppConfig>,
    _log: Arc<logging::AppLogContext>,
    _rpc: Arc<Box<dyn AbstractRpcContext>>,
    dstore: Arc<AppDataStoreContext> 
}

impl AppSharedState {
    pub fn new(cfg:AppConfig, log:logging::AppLogContext) -> Self
    {
        let _rpc_ctx = build_rpc_context(&cfg.api_server.rpc)
            .unwrap();
        let (in_mem, sql_dbs) = datastore::build_context(&cfg.api_server.data_store);
        let in_mem = if let Some(m) = in_mem { Some(Arc::new(m)) } else {None};
        let sql_dbs = if let Some(m) = sql_dbs {
            Some(m.into_iter().map(Arc::new).collect())
        } else {None};
        let ds_ctx = AppDataStoreContext {in_mem, sql_dbs};
        Self{_cfg:Arc::new(cfg), _log:Arc::new(log), _rpc:Arc::new(_rpc_ctx),
             dstore:Arc::new(ds_ctx)  }
    }

    pub fn config(&self) -> &Arc<AppConfig>
    { &self._cfg }

    pub fn log_context(&self) -> &Arc<logging::AppLogContext>
    { &self._log }
    
    pub fn rpc(&self) -> Arc<Box<dyn AbstractRpcContext>>
    { self._rpc.clone() }

    pub fn datastore(&self) -> Arc<AppDataStoreContext>
    { self.dstore.clone() }
} // end of impl AppSharedState


impl Clone for AppSharedState {
    fn clone(&self) -> Self {
        Self{
            _cfg: self._cfg.clone(),   _log: self._log.clone(),
            _rpc: self._rpc.clone(),   dstore: self.dstore.clone()
        }
    }
}

// #[derive(Clone)]
// pub struct AppSharedState2 {
//     cn1: Arc<logging::AppLogContext>,
//     tvb: f32,
// }

