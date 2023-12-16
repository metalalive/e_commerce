use std::net::ToSocketAddrs;
use std::io::ErrorKind;
use std::result::Result as DefaultResult;

use axum::Router;
use hyper::server::Server as HyperServer;
use hyper::server::conn::AddrIncoming;
use hyper::server::Builder as HyperSrvBuilder;
use http_body::Body as HttpBody;

use crate::{WebApiRouteCfg, WebApiListenCfg, AppSharedState};
use crate::error::{AppError, AppErrorCode};
use crate::api::web::{ApiRouteType, ApiRouteTableType};

pub type WebServiceRoute<HB> = Router<(), HB>;

// Due to the issues #1110 and discussion #1818 in Axum v0.6.x,
// the type of final router depends on number of middleware layers
// which are added to the router and wrap the original http body,
// the type parameter `B` has to match that at compile time

pub fn app_web_service<HB>(cfg: &WebApiListenCfg, rtable: ApiRouteTableType<HB>,
           shr_state:AppSharedState) -> (WebServiceRoute<HB>, u16)
    where HB: HttpBody + Send + 'static
{ // the type parameters for shared state and http body should be explicitly annotated,
  // this function creates a router first then specify type of the shared state later
  // at the end of the same function.
    let mut router:Router<AppSharedState, HB> = Router::new();
    let iterator = cfg.routes.iter();
    let filt_fn = |&item:&&WebApiRouteCfg| -> bool {
        let hdlr_label = item.handler.as_str();
        rtable.contains_key(hdlr_label)
    };
    let filtered = iterator.filter(filt_fn);
    let mut num_applied:u16 = 0;
    for item in filtered {
        let hdlr_label = item.handler.as_str();
        if let Some(route) = rtable.get(hdlr_label) {
            let route_cpy:ApiRouteType<HB> = route.clone();
            router = router.route(item.path.as_str(), route_cpy);
            num_applied += 1u16;
        } // 2 different paths might linked to the same handler
    }
    let router = if num_applied > 0 {
        let api_ver_path = String::from("/") + &cfg.api_version;
        Router::new().nest(api_ver_path.as_str(), router)
    } else { router };
    // DO NOT specify state type at here, Axum converts a router to a service
    // ONLY when the type parameter `S` in `Router` becomes empty tuple `()`,
    // it is counter-intuitive that the `S` means `state type that is missing
    // in the router`.
    ////let router = router.with_state::<AppSharedState>(shr_state); // will cause error
    let router = router.with_state(shr_state);
    // let service = IntoMakeService{svc:router}; // prohibit
    (router, num_applied)
} // end of fn app_web_service


pub mod middleware {
    use std::fs::File;
    use std::str::FromStr;
    use std::time::Duration;

    use serde::Deserialize;
    use tower::limit::RateLimitLayer;
    use tower_http::cors::CorsLayer;
    use tower_http::limit::RequestBodyLimitLayer;
    use axum::http;

    use super::{DefaultResult, AppError, AppErrorCode};

    #[derive(Deserialize)]
    struct CorsAllowedOrigin {
        order: String
    }

    #[allow(non_snake_case)]
    #[derive(Deserialize)]
    struct CorsConfig {
        ALLOWED_ORIGIN: CorsAllowedOrigin,
        ALLOWED_METHODS: Vec<String>,
        ALLOWED_HEADERS: Vec<String>,
        ALLOW_CREDENTIALS: bool,
        PREFLIGHT_MAX_AGE: u64
    }

    pub fn rate_limit(max_conn: u32) -> RateLimitLayer {
        let num = max_conn as u64;
        let period = Duration::from_secs(1);
        RateLimitLayer::new(num, period)
    }

    pub fn cors(cfg_path:String) -> DefaultResult<CorsLayer, AppError> 
    {
        match File::open(cfg_path) {
            Ok(f) => match serde_json::from_reader::<File, CorsConfig>(f) {
                Ok(val) => {
                    let methods = val.ALLOWED_METHODS.iter().filter_map(
                        |m| match http::Method::from_bytes(m.as_bytes()) {
                                Ok(ms) => Some(ms),
                                Err(_e) => None,
                            }
                    ).collect::<Vec<http::Method>>();
                    if val.ALLOWED_METHODS.len() > methods.len() {
                        let detail = format!("invalid-allowed-method");
                        return Err(AppError { detail: Some(detail),
                                code: AppErrorCode::InvalidInput });
                    }
                    let headers = val.ALLOWED_HEADERS.iter().filter_map(
                        |h| match http::HeaderName::from_str(h.as_str()) {
                                Ok(hs) => Some(hs), Err(_e) => None
                            }
                    ).collect::<Vec<http::HeaderName>>();
                    if !headers.contains(&http::header::AUTHORIZATION) ||
                        !headers.contains(&http::header::CONTENT_TYPE) ||
                        !headers.contains(&http::header::ACCEPT) {
                        let detail = format!("invalid-allowed-header");
                        return Err(AppError { detail: Some(detail),
                                code: AppErrorCode::InvalidInput }); 
                    }
                    let origin = val.ALLOWED_ORIGIN.order.parse::<http::HeaderValue>().unwrap();
                    let co = CorsLayer::new().allow_origin(origin)
                        .allow_methods(methods).allow_headers(headers)
                        .allow_credentials(val.ALLOW_CREDENTIALS)
                        .max_age(Duration::from_secs(val.PREFLIGHT_MAX_AGE));
                    Ok(co)
                },
                Err(e) => Err(AppError { detail: Some(e.to_string()),
                    code: AppErrorCode::InvalidJsonFormat }),
            },
            Err(e) => Err(AppError { detail:Some(e.to_string()),
                code:AppErrorCode::IOerror(e.kind())  }),
        } // end of file open
    } // end of fn cors_middleware

    pub fn req_body_limit(limit:usize) -> RequestBodyLimitLayer {
        let reqlm = RequestBodyLimitLayer::new(limit);
        reqlm
    }
} // end of module middleware


pub fn net_server_listener(mut domain_host:String, port:u16)
    -> DefaultResult<HyperSrvBuilder<AddrIncoming>, AppError>
{
    if !domain_host.contains(":") {
        domain_host += &":0";
    }
    match domain_host.to_socket_addrs() {
        Ok(mut iterator) => loop {
            match iterator.next() {
                Some(mut addr) => {
                    addr.set_port(port);
                    match HyperServer::try_bind(&addr) {
                        Ok(b) => break Ok(b),
                        Err(_) => {}
                    }
                },
                None => break Err(AppError{
                        detail:Some("failed to bound with all IPs".to_string()),
                        code:AppErrorCode::IOerror(ErrorKind::AddrInUse)
                    })
            }
        }, // end of loop
        Err(e) => Err(AppError{
                      detail:Some(e.to_string() + ", domain_host:" + &domain_host),
                      code:AppErrorCode::IOerror(ErrorKind::AddrNotAvailable)
                  }) // IP not found after domain name resolution
    }
} // end of fn net_server_listener

