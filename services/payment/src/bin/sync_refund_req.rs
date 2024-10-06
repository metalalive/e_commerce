use std::collections::HashMap;
use std::env;
use std::result::Result;

use payment::adapter::repository::app_repo_refund;
use tokio::runtime::Builder;

use ecommerce_common::config::{AppCfgHardLimit, AppCfgInitArgs, AppConfig};
use ecommerce_common::constant::env_vars::EXPECTED_LABELS;
use ecommerce_common::logging::{app_log_event, AppLogLevel};

use payment::usecase::SyncRefundReqUseCase;
use payment::{hard_limit, AppSharedState};

#[rustfmt::skip]
async fn start_sync(shr_state:AppSharedState) -> Result<(), ()> {
    let logctx = shr_state.log_context();
    let repo = app_repo_refund(shr_state.datastore())
        .await.map_err(|e| {
            app_log_event!(logctx, AppLogLevel::ERROR, "{:?}", e);
        })?;
    let rpc_ctx = shr_state.rpc_context();
    SyncRefundReqUseCase::execute(repo, rpc_ctx)
        .await
        .map_err(|e| {
            app_log_event!(logctx, AppLogLevel::ERROR, "{:?}", e);
        })
}

fn init_config() -> Result<AppConfig, ()> {
    let iter = env::vars().filter(|(k, _v)| EXPECTED_LABELS.contains(&k.as_str()));
    let env_var_map = HashMap::from_iter(iter);
    let limit = AppCfgHardLimit {
        nitems_per_inmem_table: 0,
        num_db_conns: 10,
        seconds_db_idle: hard_limit::MAX_SECONDS_DB_IDLE,
    };
    let args = AppCfgInitArgs { env_var_map, limit };
    AppConfig::new(args).map_err(|e| {
        println!(
            "[ERROR] config failure, code:{:?}, detail:{:?}",
            e.code, e.detail
        );
    })
}

fn main() -> Result<(), ()> {
    let cfg = init_config()?;
    let shr_state = AppSharedState::new(cfg).map_err(|e| {
        println!("[ERROR] shared state init failure, {:?}", e);
    })?;
    let cfg = shr_state.config();
    let logctx = shr_state.log_context();
    let stack_nbytes = (cfg.api_server.stack_sz_kb as usize) << 10;
    let runtime = Builder::new_current_thread()
        .worker_threads(1)
        .thread_stack_size(stack_nbytes)
        .thread_name("sync-refund-req")
        .enable_time()
        .enable_io()
        .build()
        .map_err(|e| {
            app_log_event!(logctx, AppLogLevel::ERROR, "{:?}", e);
        })?;
    runtime.block_on(async move { start_sync(shr_state).await })
} // end of fn main
