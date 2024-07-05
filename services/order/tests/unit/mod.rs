mod adapter;
mod auth;
pub(crate) mod model;
mod network;
mod repository;
mod rpc;
mod usecase;

use std::env;
use std::result::Result as DefaultResult;

use ecommerce_common::confidentiality::AbstractConfidentiality;
use ecommerce_common::constant::env_vars::{SERVICE_BASEPATH, SYS_BASEPATH};
use ecommerce_common::error::AppConfidentialityError;
use ecommerce_common::logging::AppLogContext;

use order::constant::hard_limit;
use order::{AppBasepathCfg, AppCfgHardLimit, AppConfig, AppSharedState};

pub(crate) const EXAMPLE_REL_PATH: &'static str = "/tests/unit/examples/";

pub(crate) fn ut_setup_share_state(
    cfg_fname: &str,
    cfdntl: Box<dyn AbstractConfidentiality>,
) -> AppSharedState {
    let service_basepath = env::var(SERVICE_BASEPATH).unwrap();
    let sys_basepath = env::var(SYS_BASEPATH).unwrap();
    let fullpath = service_basepath.clone() + EXAMPLE_REL_PATH + cfg_fname;
    let limit = AppCfgHardLimit {
        nitems_per_inmem_table: hard_limit::MAX_ITEMS_STORED_PER_MODEL,
        num_db_conns: hard_limit::MAX_DB_CONNECTIONS,
        seconds_db_idle: hard_limit::MAX_SECONDS_DB_IDLE,
    };
    let cfg = AppConfig {
        api_server: AppConfig::parse_from_file(fullpath, limit).unwrap(),
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
    fn try_get_payload(&self, path: &str) -> DefaultResult<String, AppConfidentialityError> {
        let d = match path {
            "backend_apps/secret_key/staff/OpenExchangeRates" => "\"unit-test\"",
            _others => "unit-test",
        };
        Ok(d.to_string())
    }
}
