use std::env;

use tokio::runtime::Builder as RuntimeBuilder;

use order::{AppConfig, AppSharedState};
use order::logging::{AppLogContext, AppLogLevel, app_log_event};
use order::network::{generate_webapi_route, generate_webapi_server};
use order::api::web::route_table as web_route_table;

async fn start_server (shr_state:AppSharedState)
{
    let log_ctx_p = shr_state.log_context().clone();
    let cfg = shr_state.config().clone();
    let routes = web_route_table();
    let (num_applied, router) = generate_webapi_route(
           &cfg.api_server.listen, routes);
    if num_applied == 0 {
        app_log_event!(log_ctx_p, AppLogLevel::ERROR,
                "no route created, web API server failed to start");
        return;
    }
    match generate_webapi_server(&cfg.api_server, router, shr_state)
    {
        Ok(srv) => {
            app_log_event!(log_ctx_p, AppLogLevel::INFO, "API server starting");
            let _ = srv.await;
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
    let mut _args = env::args();
    _args.next(); // omit path to the executable
    match AppConfig::new(_args) {
        Ok(cfg) => {
            start_async_runtime(cfg);
        },
        Err(e) => {
            println!("app failed to configure, error code: {} ", e);
        }
    };
} // end of main

