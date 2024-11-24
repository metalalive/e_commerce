use std::time::Duration;

use actix_http::Request;
use actix_service::IntoServiceFactory;
use actix_web::body::MessageBody;
use actix_web::dev::{AppConfig, Response, ServiceFactory, ServiceRequest, ServiceResponse};
use actix_web::web;
use actix_web::{App, HttpServer};

use ecommerce_common::config::{ApiServerCfg, WebApiRouteCfg};

use crate::api::web::AppRouteTable;

/*
 * the original implementation does not intend to let users transfer `App` object
 * around code functions, it is tricky to do so.
 *
 * The test example in the following FAQ demonstrates how to do this :
 * https://github.com/actix/actix-web/wiki/FAQ#how-can-i-return-app-from-a-function--why-is-appentry-private
 *
 * relavant issues (in actix-web github)
 * #780 #1005 #1156 #2039 #2073 #2082 #2301
 *
 * TODO
 * - support multiple versions of route-tables and configurations
 * */
pub fn app_web_service(
    mut route_table: AppRouteTable,
    cfg: Vec<WebApiRouteCfg>,
) -> (
    App<
        impl ServiceFactory<
            ServiceRequest,
            Response = ServiceResponse<impl MessageBody>,
            Error = actix_web::error::Error,
            Config = (),
            InitError = (),
        >,
    >,
    usize,
) {
    let mut num_applied = 0usize;
    let num_applied_p = &mut num_applied;
    let cfg_fn = move |c: &mut web::ServiceConfig| {
        *num_applied_p = cfg
            .into_iter()
            .filter_map(|c| {
                let (path, inner_label) = (c.path, c.handler);
                route_table
                    .entries
                    .remove(inner_label.as_str())
                    .map(|found| (path, found))
            })
            .map(|(path, route_found)| {
                c.route(path.as_str(), route_found);
            })
            .count();
    };
    let path_prefix = format!("/{}", route_table.version.as_str());
    let v_scope = web::scope(path_prefix.as_str()).configure(cfg_fn);
    let app = App::new().service(v_scope);
    (app, num_applied)
}

pub fn net_server_listener<F, I, S, B>(app_init_cb: F, cfg: &ApiServerCfg) -> HttpServer<F, I, S, B>
where
    F: Fn() -> I + Clone + Send + 'static,
    I: IntoServiceFactory<S, Request>,
    S: ServiceFactory<Request, Config = AppConfig> + 'static,
    S::Error: Into<actix_web::error::Error>,
    S::InitError: std::fmt::Debug,
    S::Response: Into<Response<B>>,
    B: MessageBody + 'static,
{
    let domain_host = cfg.listen.host.as_str();
    let port = cfg.listen.port;
    let domain = format!("{domain_host}:{port}");
    let srv = HttpServer::new(app_init_cb).bind(domain).unwrap();
    srv.max_connections(cfg.listen.max_connections as usize)
        .workers(cfg.num_workers as usize)
        .client_request_timeout(Duration::from_secs(61))
        .client_disconnect_timeout(Duration::from_secs(45))
        .shutdown_timeout(70)
}

pub mod middleware {
    use std::fs::File;
    use std::result::Result;
    use std::str::FromStr;

    use actix_cors::Cors;
    use actix_http::header::HeaderName;
    use actix_http::Method;
    use serde::Deserialize;

    use ecommerce_common::error::AppErrorCode;

    #[derive(Deserialize)]
    struct CorsAllowedOrigin {
        payment: String,
    }

    #[allow(non_snake_case)]
    #[derive(Deserialize)]
    struct CorsConfig {
        ALLOWED_ORIGIN: CorsAllowedOrigin,
        ALLOWED_METHODS: Vec<String>,
        ALLOWED_HEADERS: Vec<String>,
        ALLOW_CREDENTIALS: bool,
        PREFLIGHT_MAX_AGE: u64,
    }

    pub fn cors(cfg_path: String) -> Result<Cors, (AppErrorCode, String)> {
        let f =
            File::open(cfg_path).map_err(|e| (AppErrorCode::IOerror(e.kind()), e.to_string()))?;
        let cfg = serde_json::from_reader::<File, CorsConfig>(f)
            .map_err(|e| (AppErrorCode::InvalidJsonFormat, e.to_string()))?;
        let mut errors = Vec::new();
        let mthds = cfg
            .ALLOWED_METHODS
            .iter()
            .filter_map(|m| {
                Method::from_str(m.as_str())
                    .map_err(|e| errors.push(e.to_string()))
                    .ok()
            })
            .collect::<Vec<_>>();
        if !errors.is_empty() {
            let e = errors.remove(0);
            return Err((AppErrorCode::DataCorruption, e));
        }
        let hdrs = cfg
            .ALLOWED_HEADERS
            .iter()
            .filter_map(|h| {
                HeaderName::from_str(h.as_str())
                    .map_err(|e| errors.push(e.to_string()))
                    .ok()
            })
            .collect::<Vec<_>>();
        if !errors.is_empty() {
            let e = errors.remove(0);
            return Err((AppErrorCode::DataCorruption, e));
        }
        let out = Cors::default()
            .allowed_origin(cfg.ALLOWED_ORIGIN.payment.as_str())
            .allowed_headers(hdrs)
            .allowed_methods(mthds)
            .max_age(Some(cfg.PREFLIGHT_MAX_AGE as usize));
        let out = if cfg.ALLOW_CREDENTIALS {
            out.supports_credentials()
        } else {
            out
        };
        Ok(out)
    } // end of fn cors
} // end of middleware
