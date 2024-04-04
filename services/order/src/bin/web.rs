use std::boxed::Box;
use std::collections::hash_map::RandomState;
use std::collections::HashMap;
use std::env;
use std::sync::atomic::Ordering;

use http_body::Limited;
use hyper::Body as HyperBody;
use tokio::runtime::Builder as RuntimeBuilder;
use tokio::signal::unix::{signal, SignalKind};
use tower::ServiceBuilder;
use tower_http::auth::AsyncRequireAuthorizationLayer;
use tower_http::cors::CorsLayer;

use order::api::web::route_table;
use order::confidentiality::{self, AbstractConfidentiality};
use order::constant::EXPECTED_ENV_VAR_LABELS;
use order::logging::{app_log_event, AppLogContext, AppLogLevel};
use order::network::{app_web_service, middleware, net_server_listener};
use order::{AppConfig, AppJwtAuthentication, AppSharedState};

type AppFinalHttpBody = Limited<HyperBody>; // HyperBody;

async fn start_server(shr_state: AppSharedState) {
    let log_ctx_p = shr_state.log_context().clone();
    let cfg = shr_state.config().clone();
    let shutdown_flag = shr_state.shutdown();
    let num_reqs_cnt = shr_state.num_requests();
    let keystore = shr_state.auth_keystore();
    let routes = route_table::<AppFinalHttpBody>();
    let listener = &cfg.api_server.listen;
    let (leaf_service, num_applied) =
        app_web_service::<AppFinalHttpBody>(listener, routes, shr_state);
    if num_applied == 0 {
        app_log_event!(
            log_ctx_p,
            AppLogLevel::ERROR,
            "API-server-start-failure, no-route-created"
        );
        return;
    }
    let result = net_server_listener(listener.host.clone(), listener.port);
    let server_bound = match result {
        Ok(b) => b,
        Err(e) => {
            app_log_event!(
                log_ctx_p,
                AppLogLevel::ERROR,
                "API-server-start-failure, {e}"
            );
            return;
        }
    };
    let sh_detect =
        middleware::ShutdownDetectionLayer::new(shutdown_flag.clone(), num_reqs_cnt.clone());
    let ratelm = middleware::rate_limit(listener.max_connections);
    let reqlm = middleware::req_body_limit(cfg.api_server.limit_req_body_in_bytes);
    let authm = {
        let jwtauth = AppJwtAuthentication::new(keystore, Some(log_ctx_p.clone()));
        AsyncRequireAuthorizationLayer::new(jwtauth)
    };
    let cors_cfg_fullpath = cfg.basepath.system.clone() + "/" + listener.cors.as_str();
    let co = match middleware::cors(cors_cfg_fullpath) {
        Ok(v) => v,
        Err(e) => {
            app_log_event!(
                log_ctx_p,
                AppLogLevel::ERROR,
                "cors layer init error, detail: {:?}",
                e
            );
            CorsLayer::new()
        }
    };
    let middlewares_cloneable = ServiceBuilder::new()
        .layer(sh_detect)
        .layer(reqlm)
        .layer(co)
        .layer(authm);
    let merged_service = leaf_service.layer(middlewares_cloneable);
    let final_service = ServiceBuilder::new()
        // add middlewares which are not allowed to be cloned
        .layer(ratelm)
        // TODO, FIXME
        // Axum Router assigns itself to Response type parameter of a tower service
        // trait. any custom middleware will hit type-mismatch error [E0271] if :
        // (1) it modifies the response type
        // (2) adding itself  to the service builder at here
        //
        // Figure out how this happened, is the error resolved in latest version ?
        .service(merged_service.into_make_service());
    let srv = server_bound.serve(final_service);
    let log_ctx_shutdown = log_ctx_p.clone();
    let graceful = srv.with_graceful_shutdown(async move {
        let mut shutdown_signal = signal(SignalKind::terminate()).unwrap();
        shutdown_signal.recv().await;
        shutdown_flag.store(true, Ordering::Relaxed);
        for _ in 0..6 {
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
    let _ = graceful.await;
    app_log_event!(log_ctx_p, AppLogLevel::INFO, "API-server-terminating");
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
                        app_log_event!(
                            log_ctx,
                            AppLogLevel::WARNING,
                            "return period error, reason: {:?} ",
                            e
                        );
                        std::time::Duration::new(period_secs, 0)
                    }
                }
            }
            Err(e) => {
                app_log_event!(
                    log_ctx,
                    AppLogLevel::ERROR,
                    "refresh failure JWK set, reason: {:?} ",
                    e
                );
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
            let log_cpy = log_ctx2.clone();
            app_log_event!(
                log_cpy,
                AppLogLevel::INFO,
                "[API server] worker terminating"
            );
        })
        .thread_stack_size(stack_nbytes)
        .thread_name("web-api-worker")
        // manage low-level I/O drivers used by network types
        .enable_io()
        // rate limiter in crate `tower` requires the timer in the runtime builder
        .enable_time()
        .build();
    match result {
        Ok(rt) => {
            // new worker threads spawned
            rt.block_on(async move {
                let task_jwk = start_jwks_refresh(shr_state.clone());
                tokio::task::spawn(task_jwk);
                start_server(shr_state).await;
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
    let iter = env::vars().filter(|(k, _v)| EXPECTED_ENV_VAR_LABELS.contains(&k.as_str()));
    let arg_map: HashMap<String, String, RandomState> = HashMap::from_iter(iter);
    match AppConfig::new(arg_map) {
        Ok(cfg) => match confidentiality::build_context(&cfg) {
            Ok(confidential) => start_async_runtime(cfg, confidential),
            Err(e) => {
                println!(
                    "app failed to init confidentiality handler, error code: {} ",
                    e
                );
            }
        },
        Err(e) => {
            println!("app failed to configure, error code: {} ", e);
        }
    };
} // end of main
