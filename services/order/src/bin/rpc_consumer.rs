use std::boxed::Box;
use std::collections::HashMap;
use std::env;
use std::future::Future;
use std::pin::Pin;
use std::result::Result as DefaultResult;

use ecommerce_common::constant::env_vars::EXPECTED_LABELS;
use ecommerce_common::logging::{app_log_event, AppLogContext, AppLogLevel};
use tokio::runtime::Builder as RuntimeBuilder;

use order::api::rpc::route_to_handler;
use order::confidentiality::{self, AbstractConfidentiality};
use order::constant::hard_limit;
use order::error::AppError;
use order::{AppCfgHardLimit, AppCfgInitArgs, AppConfig, AppRpcClientReqProperty, AppSharedState};

fn route_handler_wrapper(
    req: AppRpcClientReqProperty,
    shr_state: AppSharedState,
) -> Pin<Box<dyn Future<Output = DefaultResult<Vec<u8>, AppError>> + Send>> {
    Pin::from(Box::new(
        async move { route_to_handler(req, shr_state).await },
    ))
}

async fn start_rpc_worker(shr_state: AppSharedState) {
    let logctx_p = shr_state.log_context().clone();
    let rctx = shr_state.rpc();
    let result = rctx.server_start(shr_state, route_handler_wrapper).await;
    if let Err(e) = result {
        app_log_event!(logctx_p, AppLogLevel::ERROR, "error: {:?}", e);
    }
    //TODO, signal handler to break from the loop ..
}

fn start_async_runtime(cfg: AppConfig, cfdntl: Box<dyn AbstractConfidentiality>) {
    let log_ctx = AppLogContext::new(&cfg.basepath, &cfg.api_server.logging);
    let shr_state = AppSharedState::new(cfg, log_ctx, cfdntl);
    let cfg = shr_state.config();
    let stack_nbytes: usize = (cfg.api_server.stack_sz_kb as usize) << 10;
    let result = RuntimeBuilder::new_multi_thread()
        .worker_threads(cfg.api_server.num_workers as usize)
        .thread_stack_size(stack_nbytes)
        .thread_name("rpc-api-consumer")
        // manage low-level I/O drivers used by network types
        .enable_io()
        .enable_time()
        .build();
    match result {
        Ok(rt) => {
            // new worker threads spawned
            rt.block_on(async move {
                start_rpc_worker(shr_state).await;
            }); // runtime started
        }
        Err(e) => {
            let log_ctx_p = shr_state.log_context();
            app_log_event!(
                log_ctx_p,
                AppLogLevel::ERROR,
                "async runtime failed to build, {} ",
                e
            );
        }
    };
} // end of start_async_runtime

fn main() {
    let iter = env::vars().filter(|(k, _v)| EXPECTED_LABELS.contains(&k.as_str()));
    let args = AppCfgInitArgs {
        limit: AppCfgHardLimit {
            nitems_per_inmem_table: hard_limit::MAX_ITEMS_STORED_PER_MODEL,
            num_db_conns: hard_limit::MAX_DB_CONNECTIONS,
            seconds_db_idle: hard_limit::MAX_SECONDS_DB_IDLE,
        },
        env_var_map: HashMap::from_iter(iter),
    };
    match AppConfig::new(args) {
        Ok(cfg) => match confidentiality::build_context(&cfg) {
            Ok(cfdntl) => {
                start_async_runtime(cfg, cfdntl);
            }
            Err(e) => {
                println!(
                    "app failed to init confidentiality handler, error code: {} ",
                    e
                );
            }
        },
        Err(e) => {
            println!(
                "app failed to configure, error code: {} ",
                AppError::from(e)
            );
        }
    };
} // end of main
