use std::collections::HashMap;
use std::collections::hash_map::RandomState;
use std::env;

use tokio::runtime::Builder as RuntimeBuilder;

use order::{AppConfig, AppSharedState};
use order::constant::EXPECTED_ENV_VAR_LABELS;
use order::logging::{AppLogContext, AppLogLevel, app_log_event};
use order::network::{generate_web_service, start_web_service};
use order::api::web::route_table as web_route_table;

async fn start_server (shr_state:AppSharedState)
{
    let log_ctx_p = shr_state.log_context().clone();
    let cfg = shr_state.config().clone();
    let routes = web_route_table();
    let listener = &cfg.api_server.listen;
    let (num_applied, srv) = generate_web_service(
            listener, routes, shr_state);
    if num_applied == 0 {
        app_log_event!(log_ctx_p, AppLogLevel::ERROR,
                "no route created, web API server failed to start");
        return;
    }
    let result = start_web_service(
        listener.host.clone(), listener.port, srv );
    match result {
        Ok(sr) => {
            app_log_event!(log_ctx_p, AppLogLevel::INFO, "API server starting");
            let _ = sr.await;
            app_log_event!(log_ctx_p, AppLogLevel::INFO, "API server terminating");
        },
        Err(e) => {
            app_log_event!(log_ctx_p, AppLogLevel::ERROR,
                    "API server failed to start, {} ", e);
        }
    }
}

fn start_async_runtime (cfg:AppConfig)
{
    let log_ctx = AppLogContext::new(&cfg.basepath, &cfg.api_server.logging);
    let shr_state = AppSharedState::new(cfg, log_ctx);
    let cfg = shr_state.config();
    let stack_nbytes:usize = (cfg.api_server.stack_sz_kb as usize) << 10;
    let result = RuntimeBuilder::new_multi_thread()
        .worker_threads(cfg.api_server.num_workers as usize)
        .thread_stack_size(stack_nbytes)
        .thread_name("web-api-worker")
        // manage low-level I/O drivers used by network types
        .enable_io()
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
        Ok(cfg) => {
            start_async_runtime(cfg);
        },
        Err(e) => {
            println!("app failed to configure, error code: {} ", e);
        }
    };
} // end of main

