use actix_web::rt;

use payment::api::web::AppRouteTable;
use payment::network::{app_web_service, net_server_listener};

struct MockAppConfig {
    host: String,
    port: u16,
}

fn main() {
    let cfg = MockAppConfig {
        host: "localhost".to_string(),
        port: 8015,
    };
    let cfg_routes = [
        ("/charge/{charge_id}", "create_new_charge"),
        ("/charge/{charge_id}", "refresh_charge_status"),
    ]
    .into_iter()
    .map(|(path, inner_label)| (path.to_string(), inner_label.to_string()))
    .collect::<Vec<_>>();
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
        let (app, num_applied) = app_web_service(route_table, cfg_routes.clone());
        if num_applied == 0 {
            // TODO, logging error, actix-web does not consider to handle error
            // returned from this callback
        }
        app
    };
    let ht_srv = net_server_listener(app_init, cfg.host.as_str(), cfg.port);
    let runner = rt::System::new();
    let _result = runner.block_on(ht_srv.run());
}
