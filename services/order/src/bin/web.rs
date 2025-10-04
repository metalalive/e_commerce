use std::boxed::Box;
use std::collections::HashMap;
use std::env;
use std::result::Result;
use std::sync::atomic::Ordering;

use ecommerce_common::constant::env_vars::EXPECTED_LABELS;
use tokio::runtime::Builder as RuntimeBuilder;
use tokio::signal::unix::{signal, SignalKind};
use tower::ServiceBuilder;
use tower_http::auth::AsyncRequireAuthorizationLayer;

use ecommerce_common::confidentiality::{self, AbstractConfidentiality};
use ecommerce_common::config::{AppCfgHardLimit, AppCfgInitArgs, AppConfig};
use ecommerce_common::logging::{app_log_event, AppLogContext, AppLogLevel};

use order::api::web::route_table;
use order::constant::hard_limit;
use order::error::AppError;
use order::network::{app_web_service, middleware, net_listener};
use order::{AppJwtAuthentication, AppSharedState};

async fn start_server(shr_state: AppSharedState) -> Result<(), String> {
    let log_ctx_p = shr_state.log_context().clone();
    let cfg = shr_state.config().clone();
    let shutdown_flag = shr_state.shutdown();
    let num_reqs_cnt = shr_state.num_requests();
    let keystore = shr_state.auth_keystore();
    let routes = route_table();
    let listenercfg = &cfg.api_server.listen;
    let (leaf_router, num_applied) = app_web_service(listenercfg, routes, shr_state);
    if num_applied == 0 {
        return Err("API-server-start-failure, no-route-created".to_string());
    }
    let listener = net_listener(listenercfg.host.clone(), listenercfg.port)
        .await
        .map_err(|e| format!("API-server-start-failure, {e}"))?;
    let sh_detect =
        middleware::ShutdownDetectionLayer::new(shutdown_flag.clone(), num_reqs_cnt.clone());
    let ratelm = middleware::rate_limit(listenercfg.max_connections);
    let reqlm = middleware::req_body_limit(cfg.api_server.limit_req_body_in_bytes);
    let authm = {
        let jwtauth = AppJwtAuthentication::new(keystore, Some(log_ctx_p.clone()));
        AsyncRequireAuthorizationLayer::new(jwtauth)
    };
    let cors_cfg_fullpath = cfg.basepath.system.clone() + "/" + listenercfg.cors.as_str();
    let co = middleware::cors(cors_cfg_fullpath)
        .map_err(|e| format!("cors layer init error, detail: {:?}", e))?;
    // pack layer of services which can be cloned for each inbound connection.
    let per_conn_service = leaf_router
        .layer(authm)
        .layer(co)
        .layer(reqlm)
        .layer(sh_detect)
        .into_make_service();
    // add server-wide service layers which are not allowed to be cloned
    let final_service = ServiceBuilder::new()
        .layer(ratelm)
        // TODO, FIXME
        // Axum Router assigns itself to Response type parameter of a tower service
        // trait. any custom middleware will hit type-mismatch error [E0271] if :
        // (1) it modifies the response type
        // (2) adding itself  to the service builder at here
        //
        // Figure out how this happened, is the error resolved in latest version ?
        .service(per_conn_service);

    let srv = axum::serve(listener, final_service);

    let log_ctx_shutdown = log_ctx_p.clone();
    let graceful = srv.with_graceful_shutdown(async move {
        let mut shutdown_signal = signal(SignalKind::terminate()).unwrap();
        shutdown_signal.recv().await;
        shutdown_flag.store(true, Ordering::Relaxed);
        for _ in 0..10 {
            let num_reqs_rest = num_reqs_cnt.load(Ordering::Relaxed);
            app_log_event!(
                log_ctx_shutdown,
                AppLogLevel::DEBUG,
                "num_reqs_rest : {num_reqs_rest}"
            );
            if num_reqs_rest == 0 {
                break;
            } else {
                let period = std::time::Duration::new(5, 0);
                tokio::time::sleep(period).await;
            }
        } // TODO, improve the code at here
    });
    let result = graceful.await;
    app_log_event!(log_ctx_p, AppLogLevel::INFO, "API-server-terminating");
    result.map_err(|e| e.to_string())
} // end of fn start_server

async fn start_jwks_refresh(shr_state: AppSharedState) {
    let log_ctx = shr_state.log_context().clone();
    let keystore = shr_state.auth_keystore();
    let period_secs = keystore.update_period().num_seconds() as u64;
    let mut shutdown_signal = signal(SignalKind::terminate()).unwrap();
    loop {
        let period = match keystore.refresh().await {
            Ok(stats) => {
                app_log_event!(
                    log_ctx,
                    AppLogLevel::DEBUG,
                    "JWK set refreshed, \
                               period-to-next-op:{}, num-added:{}, num-discarded:{}",
                    stats.period_next_op.num_minutes(),
                    stats.num_added,
                    stats.num_discarded
                );
                match stats.period_next_op.to_std() {
                    Ok(p) => p,
                    Err(e) => {
                        app_log_event!(log_ctx, AppLogLevel::WARNING, "period-error:{:?} ", e);
                        std::time::Duration::new(period_secs, 0)
                    }
                }
            }
            Err(e) => {
                app_log_event!(log_ctx, AppLogLevel::ERROR, "jwks-refresh-failure:{:?} ", e);
                std::time::Duration::new(300, 0)
            }
        };
        tokio::select! {
            _ = tokio::time::sleep(period) => { },
            _ = shutdown_signal.recv()  => { break; },
        }
    } // end of loop
    app_log_event!(log_ctx, AppLogLevel::INFO, "JWKS-refresh-terminating");
} // end of fn start_jwks_refresh

fn start_async_runtime(cfg: AppConfig, confidential: Box<dyn AbstractConfidentiality>) {
    let log_ctx = AppLogContext::new(&cfg.basepath, &cfg.api_server.logging);
    let shr_state = AppSharedState::new(cfg, log_ctx, confidential);
    let cfg = shr_state.config();
    let log_ctx = shr_state.log_context().clone();
    let log_ctx2 = log_ctx.clone();
    let stack_nbytes: usize = (cfg.api_server.stack_sz_kb as usize) << 10;
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
            let log_cp = log_ctx2.clone();
            app_log_event!(log_cp, AppLogLevel::INFO, "[API server] worker terminating");
        })
        .thread_stack_size(stack_nbytes)
        .thread_name("web-api-worker")
        // manage low-level I/O drivers used by network types
        .enable_io()
        // rate limiter in crate `tower` requires the timer in the runtime builder
        .enable_time()
        .build();
    let log_ctx_p = shr_state.log_context().clone();
    match result {
        Ok(rt) => {
            // new worker threads spawned
            let r = rt.block_on(async move {
                let task_jwk = start_jwks_refresh(shr_state.clone());
                tokio::task::spawn(task_jwk);
                start_server(shr_state).await
            }); // runtime started
            if let Err(detail) = r {
                app_log_event!(log_ctx_p, AppLogLevel::ERROR, "{detail}");
            }
        }
        Err(e) => {
            app_log_event!(log_ctx_p, AppLogLevel::ERROR, "async-runtime-fail:{e}");
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
            Ok(confidential) => start_async_runtime(cfg, confidential),
            Err(e) => {
                println!("fail-init-confidential-handler:{:?} ", e);
            }
        },
        Err(e) => {
            println!("fail-app-cfg: {}", AppError::from(e));
        }
    };
} // end of main
