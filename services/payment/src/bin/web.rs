use std::collections::HashMap;
use std::env;

use actix_web::rt;
use actix_web::web::Data as WebData;

use ecommerce_common::config::{AppCfgHardLimit, AppCfgInitArgs, AppConfig};
use ecommerce_common::constant::env_vars::EXPECTED_LABELS;
use ecommerce_common::logging::{app_log_event, AppLogLevel};

use payment::api::web::AppRouteTable;
use payment::network::{app_web_service, net_server_listener};
use payment::{hard_limit, AppSharedState};

fn main() {
    let cfg = {
        let iter = env::vars().filter(|(k, _v)| EXPECTED_LABELS.contains(&k.as_str()));
        let env_var_map = HashMap::from_iter(iter);
        let limit = AppCfgHardLimit {
            nitems_per_inmem_table: 0,
            num_db_conns: hard_limit::MAX_DB_CONNECTIONS,
            seconds_db_idle: hard_limit::MAX_SECONDS_DB_IDLE,
        };
        let args = AppCfgInitArgs { env_var_map, limit };
        match AppConfig::new(args) {
            Ok(c) => c,
            Err(e) => {
                println!(
                    "[ERROR] config failure, code:{:?}, detail:{:?}",
                    e.code, e.detail
                );
                return;
            }
        }
    };
    let server = (
        cfg.api_server.listen.host.clone(),
        cfg.api_server.listen.port,
    );
    let shr_state = match AppSharedState::new(cfg) {
        Ok(s) => s,
        Err(e) => {
            println!("[ERROR] shared state init failure, {:?}", e);
            return;
        }
    };
    let logctx = shr_state.log_context();
    /*
     * `App` instance is created on each server worker thread (per HTTP reuqest ?)
     * To share the same data between all `App` instances, initialize the data outside
     * the factory closure in  `HttpServer::new(F)` , clone the data you need to move
     * into the closure, by doing so, the function variable is automatically treated
     * as `Fn()` type instead of `FnOnce()` type.
     *
     * https://docs.rs/actix-web/latest/actix_web/struct.App.html#shared-mutable-state
     *
     * */
    let app_init = move || {
        let _state = shr_state.clone();
        let cfg_ref = _state.config();
        let listener_ref = &cfg_ref.api_server.listen;
        let api_ver = listener_ref.api_version.as_str();
        let route_table = AppRouteTable::get(api_ver);
        let cfgroutes = listener_ref.routes.clone();
        let (app, num_applied) = app_web_service(route_table, cfgroutes);
        if num_applied == 0 {
            let logctx = _state.log_context();
            let logctx_p = logctx.as_ref();
            app_log_event!(logctx_p, AppLogLevel::ERROR, "no-route-in-app-router");
        } // log error, actix-web doesn't consider to handle errors from this callback
        app.app_data(WebData::new(_state))
    };
    let ht_srv = net_server_listener(app_init, server.0.as_str(), server.1);
    let runner = rt::System::new();
    if let Err(e) = runner.block_on(ht_srv.run()) {
        let logctx_p = &logctx;
        app_log_event!(logctx_p, AppLogLevel::ERROR, "reason: {:?}", e);
    }
} // end of fn main
