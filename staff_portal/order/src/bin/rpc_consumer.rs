use std::env;
use std::boxed::Box;
use std::collections::HashMap;
use std::collections::hash_map::RandomState;

use tokio::runtime::Builder as RuntimeBuilder;
use tokio::task;

use order::{AppConfig, AppSharedState,  AppRpcClientReqProperty, AppRpcReply};
use order::confidentiality::{self, AbstractConfidentiality};
use order::constant::EXPECTED_ENV_VAR_LABELS;
use order::logging::{AppLogContext, AppLogLevel, app_log_event};
use order::usecase::rpc_server_process;
use order::api::rpc::route_to_handler;

async fn app_request_handler(req:AppRpcClientReqProperty, shr_state:AppSharedState )
    -> AppRpcReply
{ // handle every single request or error
    let logctx_p = shr_state.log_context().clone();
    let route_bak = req.route.clone();
    let respbody = match route_to_handler(req, shr_state).await
    {
        Ok(raw_resp) => raw_resp,
        Err(e) => {
            app_log_event!(logctx_p, AppLogLevel::ERROR,
                    "[rpc][consumer] failed to handle the request, \
                     route:{}, detail:{}", route_bak, e);
            let pattern = r#" {"status":"error", "detail":""} "#;
            let mut err: serde_json::Value = serde_json::from_str(pattern).unwrap();
            if let Some(m) = err.as_object_mut() {
                let _detail = format!("{}", e);
                m.insert("detail".to_string(), serde_json::Value::String(_detail));
            }
            let msg = err.to_string();
            msg.into_bytes()
        },
    };
    AppRpcReply { body:respbody }
} // end of app_request_handler

async fn start_rpc_worker(shr_state: AppSharedState)
{
    let logctx_p = shr_state.log_context().clone();
    loop {
        let rctx = shr_state.rpc();
        let _joinh = match rpc_server_process(shr_state.clone(),
                     rctx, app_request_handler).await  {
            Ok(tsk) =>  task::spawn(tsk),
            Err(e) => {
                app_log_event!(logctx_p, AppLogLevel::ERROR,
                        "[rpc][consumer] failed to create task, {}", e);
                continue;
            }
        }; // TODO, keep the join handles, abort them once sigterm received
        app_log_event!(logctx_p, AppLogLevel::DEBUG, "[rpc][consumer] main loop running");
    } // TODO, signal handler to break from the loop ..
}


fn start_async_runtime (cfg:AppConfig, cfdntl:Box<dyn AbstractConfidentiality>)
{
    let log_ctx = AppLogContext::new(&cfg.basepath, &cfg.api_server.logging);
    let shr_state = AppSharedState::new(cfg, log_ctx, cfdntl);
    let cfg = shr_state.config();
    let stack_nbytes:usize = (cfg.api_server.stack_sz_kb as usize) << 10;
    let result = RuntimeBuilder::new_multi_thread()
        .worker_threads(cfg.api_server.num_workers as usize)
        .thread_stack_size(stack_nbytes)
        .thread_name("rpc-api-consumer")
        // manage low-level I/O drivers used by network types
        .enable_io()
        .build();
    match result {
        Ok(rt) => { // new worker threads spawned
            rt.block_on(async move {
                start_rpc_worker(shr_state).await;
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
            Ok(cfdntl) => { start_async_runtime(cfg, cfdntl); },
            Err(e) => {
                println!("app failed to init confidentiality handler, error code: {} ", e);
            }
        },
        Err(e) => {
            println!("app failed to configure, error code: {} ", e);
        }
    };
} // end of main

