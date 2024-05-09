use actix_web::rt;
use std::collections::HashMap;
use std::env;

use ecommerce_common::config::{AppCfgHardLimit, AppCfgInitArgs, AppConfig};
use ecommerce_common::constant::env_vars::EXPECTED_LABELS;

use payment::api::web::AppRouteTable;
use payment::hard_limit;
use payment::network::{app_web_service, net_server_listener};

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
    let cfgroutes = cfg.api_server.listen.routes.clone();
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
        let route_table = AppRouteTable::default();
        let (app, num_applied) = app_web_service(route_table, cfgroutes.clone());
        if num_applied == 0 {
            // TODO, logging error, actix-web does not consider to handle error
            // returned from this callback
        }
        app
    };
    let ht_srv = net_server_listener(
        app_init,
        cfg.api_server.listen.host.as_str(),
        cfg.api_server.listen.port,
    );
    let runner = rt::System::new();
    let _result = runner.block_on(ht_srv.run());
} // end of fn main
