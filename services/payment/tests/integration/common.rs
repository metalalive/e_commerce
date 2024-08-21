use std::collections::HashMap;
use std::env;

use actix_http::body::MessageBody;
use actix_http::Request;
use actix_web::dev::{Service, ServiceResponse};
use actix_web::error::Error as WebError;
use actix_web::test::init_service;
use actix_web::web::Data as WebData;

use chrono::Local;
use ecommerce_common::config::{AppCfgHardLimit, AppCfgInitArgs, AppConfig};
use ecommerce_common::constant::env_vars::EXPECTED_LABELS;

use payment::api::web::AppRouteTable;
use payment::network::app_web_service;
use payment::{app_meta, AppAuthedClaim, AppSharedState};

#[macro_export] // available at crate level
macro_rules! ItestService {
    () => {
        impl Service<Request, Response = ServiceResponse<impl MessageBody>, Error = WebError>
    }
}

fn setup_config() -> AppConfig {
    let iter = env::vars().filter(|(k, _v)| EXPECTED_LABELS.contains(&k.as_str()));
    let env_var_map = HashMap::from_iter(iter);
    let limit = AppCfgHardLimit {
        nitems_per_inmem_table: 0,
        num_db_conns: 10,
        seconds_db_idle: 60,
    };
    let args = AppCfgInitArgs { env_var_map, limit };
    AppConfig::new(args).unwrap()
}

pub(crate) async fn itest_setup_app_server() -> ItestService!() {
    let cfg = setup_config();
    let listener_ref = &cfg.api_server.listen;
    let api_ver = listener_ref.api_version.as_str();
    let route_table = AppRouteTable::get(api_ver);
    assert_eq!(route_table.entries.len(), 5);
    let cfg_routes = cfg.api_server.listen.routes.clone();
    let (app, num_applied) = app_web_service(route_table, cfg_routes);
    assert_eq!(num_applied, 5);
    let shr_state = AppSharedState::new(cfg).unwrap();
    let app = app.app_data(WebData::new(shr_state));
    init_service(app).await
}

pub(crate) fn itest_setup_auth_claim(profile: u32) -> AppAuthedClaim {
    let now = Local::now().fixed_offset().timestamp();
    AppAuthedClaim {
        profile,
        iat: now,
        exp: now + 60,
        aud: vec![app_meta::LABAL.to_string()],
        perms: Vec::new(),
        quota: Vec::new(),
    }
}
