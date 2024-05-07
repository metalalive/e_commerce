mod adapter;
mod auth;
mod confidentiality;
mod config;
mod logging;
pub(crate) mod model;
mod network;
mod repository;
mod rpc;
mod usecase;

use std::env;
use std::result::Result as DefaultResult;

use ecommerce_common::constant::env_vars::{SERVICE_BASEPATH, SYS_BASEPATH};

use order::confidentiality::AbstractConfidentiality;
use order::error::AppError;
use order::logging::AppLogContext;
use order::{AppBasepathCfg, AppConfig, AppSharedState};

pub(crate) const EXAMPLE_REL_PATH: &'static str = "/tests/unit/examples/";

pub(crate) fn ut_setup_share_state(
    cfg_fname: &str,
    cfdntl: Box<dyn AbstractConfidentiality>,
) -> AppSharedState {
    let service_basepath = env::var(SERVICE_BASEPATH).unwrap();
    let sys_basepath = env::var(SYS_BASEPATH).unwrap();
    let fullpath = service_basepath.clone() + EXAMPLE_REL_PATH + cfg_fname;
    let cfg = AppConfig {
        api_server: AppConfig::parse_from_file(fullpath).unwrap(),
        basepath: AppBasepathCfg {
            system: sys_basepath,
            service: service_basepath,
        },
    };
    let logctx = AppLogContext::new(&cfg.basepath, &cfg.api_server.logging);
    AppSharedState::new(cfg, logctx, cfdntl)
}

struct MockConfidential {}
impl AbstractConfidentiality for MockConfidential {
    fn try_get_payload(&self, _id: &str) -> DefaultResult<String, AppError> {
        Ok("unit-test".to_string())
    }
}
