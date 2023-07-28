use std::sync::Arc;

pub mod api;
pub mod error;
pub mod logging;
pub mod network;
pub mod constant;
pub mod usecase;

mod config;
pub use config::{
    AppConfig, ApiServerCfg, ApiServerListenCfg, ApiServerRouteCfg,
    AppLoggingCfg, AppLogHandlerCfg, AppLoggerCfg, AppBasepathCfg,
    AppRpcCfg, AppRpcTypeCfg
};

mod rpc;
use rpc::build_context as build_rpc_context;
pub use rpc::{
    AbstractRpcContext, AbstractRpcHandler, AppRpcPublishedResult,
    AppRpcReplyResult, AppRpcPublishProperty, AppRpcReplyProperty
};

type WebApiPath = String;
type WebApiHdlrLabel = & 'static str;
type AppLogAlias = Arc<String>;

pub struct AppSharedState {
    _cfg: Arc<AppConfig>,
    _log: Arc<logging::AppLogContext>,
    _rpc: Arc<Box<dyn AbstractRpcContext>>
}

impl AppSharedState {
    pub fn new(cfg:AppConfig, log:logging::AppLogContext) -> Self
    {
        let _rpc_ctx = build_rpc_context(&cfg.api_server.rpc)
            .unwrap();
        Self{_cfg:Arc::new(cfg), _log:Arc::new(log),
            _rpc:Arc::new(_rpc_ctx) }
    }

    pub fn config(&self) -> &Arc<AppConfig>
    { &self._cfg }

    pub fn log_context(&self) -> &Arc<logging::AppLogContext>
    { &self._log }
    
    pub fn rpc(&self) -> Arc<Box<dyn AbstractRpcContext>>
    { self._rpc.clone() }
}

impl Clone for AppSharedState {
    fn clone(&self) -> Self {
        Self{
            _cfg: self._cfg.clone(),
            _log: self._log.clone(),
            _rpc: self._rpc.clone()
        }
    }
}

// #[derive(Clone)]
// pub struct AppSharedState2 {
//     cn1: Arc<logging::AppLogContext>,
//     tvb: f32,
// }

