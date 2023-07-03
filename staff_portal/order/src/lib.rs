use std::sync::Arc;

pub mod api;
pub mod error;
pub mod logging;
pub mod network;

mod config;
pub use config::{
    AppConfig, ApiServerCfg, ApiServerListenCfg, ApiServerRouteCfg,
    AppLoggingCfg, AppLogHandlerCfg,
    AppLoggerCfg, AppBasepathCfg
};

mod constant;
pub(crate) use constant as AppConst;

type WebApiPath = String;
type WebApiHdlrLabel = & 'static str;
type AppLogAlias = Arc<String>;

pub struct AppSharedState {
    _cfg: Arc<AppConfig>,
    _log: Arc<logging::AppLogContext>,
}

impl AppSharedState {
    pub fn new(cfg:AppConfig, log:logging::AppLogContext) -> Self
    { Self{_cfg:Arc::new(cfg), _log:Arc::new(log)} }

    pub fn config(&self) -> &Arc<AppConfig>
    { &self._cfg }

    pub fn log_context(&self) -> &Arc<logging::AppLogContext>
    { &self._log }
}

impl Clone for AppSharedState {
    fn clone(&self) -> Self {
        Self{
            _cfg: self._cfg.clone(),
            _log: self._log.clone()
        }
    }
}

// #[derive(Clone)]
// pub struct AppSharedState2 {
//     cn1: Arc<logging::AppLogContext>,
//     tvb: f32,
// }

