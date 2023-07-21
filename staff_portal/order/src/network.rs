use std::net::{SocketAddr, ToSocketAddrs};
use std::io::ErrorKind;
use std::result::Result as DefaultResult;

use axum::{self, Router};
use axum::routing::IntoMakeService;
use hyper::server::conn::AddrIncoming;

use crate::{ApiServerRouteCfg, ApiServerListenCfg, AppSharedState};
use crate::error::{AppError, AppErrorCode};
use crate::api::web::{ApiRouteType, ApiRouteTableType};

pub type WebApiServer = Router<()>;
type _WebServerResult = axum::Server<AddrIncoming, IntoMakeService<Router>>;

pub fn generate_web_service(cfg: &ApiServerListenCfg, rtable: ApiRouteTableType,
                            shr_state:AppSharedState)
    -> (u16, WebApiServer)
{ // state type should be explicitly annotated, since this application creates a
  // router first then specify the state later in different scope.
    let mut router:Router<AppSharedState> = Router::new();
    let iterator = cfg.routes.iter();
    let filt_fn = |&item:&&ApiServerRouteCfg| -> bool {
        let hdlr_label = item.handler.as_str();
        rtable.contains_key(hdlr_label)
    };
    let filtered = iterator.filter(filt_fn);
    let mut num_applied:u16 = 0;
    for item in filtered {
        let hdlr_label = item.handler.as_str();
        if let Some(route) = rtable.get(hdlr_label) {
            let route_cpy:ApiRouteType = route.clone();
            router = router.route(item.path.as_str(), route_cpy);
            num_applied += 1u16;
        } // 2 different paths might linked to the same handler
    }
    let router = if num_applied > 0 {
        let api_ver_path = String::from("/") + &cfg.api_version;
        Router::new().nest(api_ver_path.as_str(), router)
    } else { router };
    // DO NOT specify state type at here, Axum converts a router to a
    // service ONLY when the generic type `S` in `Router` is NOT specified,
    // it is counter-intuitive that the `S` means `state type that is missing
    // in the router`.
    ////let router = router.with_state::<AppSharedState>(shr_state);
    let router = router.with_state(shr_state);
    (num_applied, router)
} // end of generate_web_service


pub fn start_web_service (_host:String, port:u16, router:Router)
    -> DefaultResult<_WebServerResult, AppError>
{
    let mut domain_host = _host;
    if !domain_host.contains(":") {
        domain_host += &":0";
    }
    match domain_host.to_socket_addrs() {
        Ok(mut iterator) => loop {
            match iterator.next() {
                Some(a) => {
                    let mut addr:SocketAddr = a;
                    addr.set_port(port);
                    match axum::Server::try_bind(&addr) {
                        Ok(b) => {
                            //let service = IntoMakeService{svc:router}; // prohibit
                            let service = router.into_make_service();
                            let server = b.serve(service);
                            break Ok(server)
                        },
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
} // end of  start_web_service

