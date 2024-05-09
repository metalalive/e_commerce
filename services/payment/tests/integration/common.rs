use std::collections::HashMap;
use std::env;

use actix_http::body::MessageBody;
use actix_http::Request;
use actix_web::dev::{Service, ServiceResponse};
use actix_web::error::Error as WebError;
use actix_web::test::init_service;

use ecommerce_common::config::{AppCfgHardLimit, AppCfgInitArgs, AppConfig};
use ecommerce_common::constant::env_vars::EXPECTED_LABELS;

use payment::api::web::AppRouteTable;
use payment::network::app_web_service;

pub(crate) async fn itest_setup_app_server(
) -> impl Service<Request, Response = ServiceResponse<impl MessageBody>, Error = WebError> {
    let cfg = {
        let iter = env::vars().filter(|(k, _v)| EXPECTED_LABELS.contains(&k.as_str()));
        let env_var_map = HashMap::from_iter(iter);
        let limit = AppCfgHardLimit {
            nitems_per_inmem_table: 0,
            num_db_conns: 10,
            seconds_db_idle: 60,
        };
        let args = AppCfgInitArgs { env_var_map, limit };
        AppConfig::new(args).unwrap()
    };
    let route_table = AppRouteTable::default();
    let cfg_routes = cfg.api_server.listen.routes.clone();
    let (app, num_applied) = app_web_service(route_table, cfg_routes);
    assert_eq!(num_applied, 2);
    init_service(app).await
}
