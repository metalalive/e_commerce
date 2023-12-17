mod auth;
mod config;
mod network;
mod logging;
mod usecase;
mod adapter;
mod repository;
pub(crate) mod model;
mod confidentiality;

use std::env;
use std::result::Result as DefaultResult;

use order::{AppSharedState, AppConfig, AppBasepathCfg};
use order::confidentiality::AbstractConfidentiality;
use order::constant::{ENV_VAR_SERVICE_BASE_PATH, ENV_VAR_SYS_BASE_PATH};
use order::logging::AppLogContext;
use order::error::AppError;

pub(crate) const EXAMPLE_REL_PATH : &'static str = "/tests/unit/examples/";

pub(crate) fn ut_setup_share_state(cfg_fname: &str) -> AppSharedState {
    let service_basepath = env::var(ENV_VAR_SERVICE_BASE_PATH).unwrap();
    let sys_basepath = env::var(ENV_VAR_SYS_BASE_PATH).unwrap(); 
    let fullpath = service_basepath.clone() + EXAMPLE_REL_PATH + cfg_fname;
    let cfg = AppConfig {
        api_server: AppConfig::parse_from_file(fullpath).unwrap(),
        basepath: AppBasepathCfg { system:sys_basepath , service:service_basepath },
    };
    let logctx = AppLogContext::new(&cfg.basepath, &cfg.api_server.logging);
    let cfdntl:Box<dyn AbstractConfidentiality> = Box::new(MockConfidential{});
    AppSharedState::new(cfg, logctx, cfdntl)
}

struct MockConfidential {}
impl AbstractConfidentiality for MockConfidential {
    fn try_get_payload(&self, _id:&str) -> DefaultResult<String, AppError> {
        Ok("unit-test".to_string())
    }
}
