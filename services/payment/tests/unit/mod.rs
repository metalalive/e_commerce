mod adapter;
mod auth;
mod model;
mod usecase;

use std::collections::HashMap;
use std::env;
use std::sync::OnceLock;

use ecommerce_common::config::{AppCfgHardLimit, AppCfgInitArgs, AppConfig};
use ecommerce_common::constant::env_vars::{CFG_FILEPATH, EXPECTED_LABELS};
use payment::AppSharedState;

pub(crate) const EXAMPLE_REL_PATH: &'static str = "/tests/unit/examples/";

fn ut_setup_config(cfg_filename: &str) -> AppConfig {
    let iter = env::vars().filter(|(k, _v)| EXPECTED_LABELS.contains(&k.as_str()));
    let mut env_var_map = HashMap::from_iter(iter);
    let _old = env_var_map.insert(
        CFG_FILEPATH.to_string(),
        EXAMPLE_REL_PATH.to_string() + cfg_filename,
    );
    let limit = AppCfgHardLimit {
        nitems_per_inmem_table: 0,
        num_db_conns: 10,
        seconds_db_idle: 60,
    };
    let args = AppCfgInitArgs { env_var_map, limit };
    AppConfig::new(args).unwrap()
}

fn ut_setup_sharestate(cfg_filename: &str) -> &'static AppSharedState {
    static GUARD_SHR_STATE: OnceLock<AppSharedState> = OnceLock::new();
    GUARD_SHR_STATE.get_or_init(|| {
        let cfg = ut_setup_config(cfg_filename);
        AppSharedState::new(cfg).unwrap()
    })
}
