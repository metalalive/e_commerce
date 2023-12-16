use std::collections::HashMap;
use std::collections::hash_map::RandomState;
use std::env;
use std::boxed::Box;

use http_body::Limited;
// use tower_http::follow_redirect::policy::Limited;
use hyper::Body as HyperBody;
use tokio::runtime::Builder as RuntimeBuilder;
use tower::ServiceBuilder;

use order::{AppConfig, AppSharedState};
use order::constant::EXPECTED_ENV_VAR_LABELS;
use order::confidentiality::{self, AbstractConfidentiality};
use order::logging::{AppLogContext, AppLogLevel, app_log_event};
use order::network::{net_server_listener, app_web_service, middleware};
use order::api::web::route_table;
use tower_http::cors::CorsLayer;

type AppFinalHttpBody = Limited<HyperBody>; // HyperBody;

async fn start_server (shr_state:AppSharedState)
{
    let log_ctx_p = shr_state.log_context().clone();
    let cfg = shr_state.config().clone();
    let routes = route_table::<AppFinalHttpBody>();
    let listener = &cfg.api_server.listen;
    let (service, num_applied) = app_web_service::<AppFinalHttpBody>(listener, routes, shr_state);
    if num_applied == 0 {
        app_log_event!(log_ctx_p, AppLogLevel::ERROR,
                "no route created, web API server failed to start");
        return;
    }
    let result = net_server_listener(listener.host.clone(), listener.port);
    match result {
        Ok(b) => {
            let ratelm = middleware::rate_limit(listener.max_connections);
            let reqlm = middleware::req_body_limit(cfg.api_server.limit_req_body_in_bytes);
            let co  = match middleware::cors(cfg.basepath.system.clone() +"/"+ listener.cors.as_str())
            {
                Ok(v) => v,
                Err(e) => {
                    app_log_event!(log_ctx_p, AppLogLevel::ERROR,
                                   "cors layer init error, detail: {:?}", e);
                    CorsLayer::new()
                }
            };
            let middlewares1 = ServiceBuilder::new()
                .layer(reqlm)
                .layer(co);
            let service = service.layer(middlewares1);
            let middlewares2 = ServiceBuilder::new()
                .layer(ratelm) // rate-limit not allowed to clone
                .service(service.into_make_service());
            let sr = b.serve(middlewares2);
            let _ = sr.await;
            app_log_event!(log_ctx_p, AppLogLevel::WARNING, "API server terminating ");
        },
        Err(e) => {
            app_log_event!(log_ctx_p, AppLogLevel::ERROR,
                    "API server failed to start, {} ", e);
        }
    }
} // end of fn start_server

fn start_async_runtime (cfg:AppConfig, confidential:Box<dyn AbstractConfidentiality>)
{
    let log_ctx = AppLogContext::new(&cfg.basepath, &cfg.api_server.logging);
    let shr_state = AppSharedState::new(cfg, log_ctx, confidential);
    let cfg = shr_state.config();
    let log_ctx  = shr_state.log_context().clone();
    let log_ctx2 = log_ctx.clone();
    let stack_nbytes:usize = (cfg.api_server.stack_sz_kb as usize) << 10;
    let result = RuntimeBuilder::new_multi_thread()
        .worker_threads(cfg.api_server.num_workers as usize)
        .on_thread_start(move || {
            // this `Fn()` closure will be invoked several times by new thread,
            // depending on number of work threads in the application, all variables
            // moved into this closure have to be clonable.
            let log_cpy = log_ctx.clone();
            app_log_event!(log_cpy, AppLogLevel::INFO, "[API server] worker started");
        })
        .on_thread_stop(move || {
            let log_cpy = log_ctx2.clone();
            app_log_event!(log_cpy, AppLogLevel::INFO, "[API server] worker terminating");
        })
        .thread_stack_size(stack_nbytes)
        .thread_name("web-api-worker")
        // manage low-level I/O drivers used by network types
        .enable_io()
        // rate limiter in crate `tower` requires the timer in the runtime builder
        .enable_time()
        .build();
    match result {
        Ok(rt) => { // new worker threads spawned
            rt.block_on(async move {
                start_server(shr_state).await;
            }); // runtime started
        },
        Err(e) => {
            let log_ctx_p = shr_state.log_context();
            app_log_event!(log_ctx_p, AppLogLevel::ERROR,
                   "async runtime failed to build, {} ", e);
        }
    };
} // end of start_async_runtime


fn main() {
    let iter = env::vars().filter(
        |(k,_v)| { EXPECTED_ENV_VAR_LABELS.contains(&k.as_str()) }
    );
    let arg_map: HashMap<String, String, RandomState> = HashMap::from_iter(iter);
    match AppConfig::new(arg_map) {
        Ok(cfg) => match confidentiality::build_context(&cfg) {
            Ok(confidential) => start_async_runtime(cfg, confidential),
            Err(e) => {
                println!("app failed to init confidentiality handler, error code: {} ", e);
            }
        },
        Err(e) => {
            println!("app failed to configure, error code: {} ", e);
        }
    };
} // end of main

