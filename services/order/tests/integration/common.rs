use std::borrow::BorrowMut;
use std::collections::HashMap;
use std::env;
use std::fs::File;
use std::result::Result as DefaultResult;
use std::sync::{Arc, OnceLock};

use axum_core::body::Body as AxumBody;
use http::{Request, Response};
use http_body_util::BodyExt;
use serde::Deserialize;
use tokio::sync::Mutex;
use tower::Service;

use ecommerce_common::confidentiality;
use ecommerce_common::config::{AppBasepathCfg, AppCfgHardLimit, AppCfgInitArgs, AppConfig};
use ecommerce_common::constant::env_vars::EXPECTED_LABELS;
use ecommerce_common::error::AppErrorCode;
use ecommerce_common::logging::AppLogContext;

use order::api::web::route_table;
use order::error::AppError;
use order::network::{app_web_service, WebServiceRoute};
use order::AppSharedState;

type InnerRespBody = AxumBody;

static SHARED_WEB_SERVER: OnceLock<Arc<Mutex<WebServiceRoute>>> = OnceLock::new();

static APP_SHARED_STATE_CONTAINER: OnceLock<DefaultResult<order::AppSharedState, AppError>> =
    OnceLock::new();

fn _init_app_shared_state_internal() -> DefaultResult<order::AppSharedState, AppError> {
    let iter = env::vars().filter(|(k, _)| EXPECTED_LABELS.contains(&k.as_str()));
    let args = AppCfgInitArgs {
        env_var_map: HashMap::from_iter(iter),
        limit: AppCfgHardLimit {
            nitems_per_inmem_table: 1200,
            num_db_conns: 1000,
            seconds_db_idle: 130,
        },
    };
    let top_lvl_cfg = AppConfig::new(args)?;
    let cfdntl = confidentiality::build_context(&top_lvl_cfg)?;
    let log_ctx = AppLogContext::new(&top_lvl_cfg.basepath, &top_lvl_cfg.api_server.logging);
    let obj = AppSharedState::new(top_lvl_cfg, log_ctx, cfdntl);
    Ok(obj)
}

pub fn test_setup_shr_state() -> DefaultResult<order::AppSharedState, AppError> {
    let result_ref = APP_SHARED_STATE_CONTAINER.get_or_init(|| _init_app_shared_state_internal());
    match result_ref {
        Ok(state) => Ok(state.clone()),
        Err(e) => Err(e.clone()),
    }
}

pub(crate) struct TestWebServer {}

impl TestWebServer {
    pub fn setup(shr_state: order::AppSharedState) -> Arc<Mutex<WebServiceRoute>> {
        let srv_arc_mutex = SHARED_WEB_SERVER.get_or_init(|| {
            let rtable = route_table();
            let top_lvl_cfg = shr_state.config().clone();
            let listener = &top_lvl_cfg.api_server.listen;
            let (srv_instance, _) = app_web_service(listener, rtable, shr_state);
            Arc::new(Mutex::new(srv_instance))
        });
        srv_arc_mutex.clone()
    }

    pub async fn consume(
        srv: &Arc<Mutex<WebServiceRoute>>,
        req: Request<AxumBody>,
    ) -> Response<InnerRespBody> {
        let mut guard = srv.lock().await;
        let inner_sv = guard.borrow_mut();
        let result = inner_sv.call(req).await;
        result.unwrap()
    }

    pub async fn to_custom_type<T: for<'a> Deserialize<'a>>(
        body: &mut InnerRespBody,
    ) -> DefaultResult<T, AppError> {
        let frame = body
            .frame()
            .await
            .ok_or_else(|| AppError {
                code: AppErrorCode::Unknown,
                detail: Some("no response body".to_string()),
            })?
            .map_err(|e| AppError {
                code: AppErrorCode::DataCorruption,
                detail: Some(e.to_string()),
            })?;
        let data = frame.into_data().map_err(|frm| AppError {
            code: AppErrorCode::Unknown,
            detail: Some(format!("{:?}", frm)),
        })?;
        let utf8_string = String::from_utf8(data.to_vec()).map_err(|e| AppError {
            code: AppErrorCode::Unknown,
            detail: Some(e.utf8_error().to_string()),
        })?;
        serde_json::from_str::<T>(utf8_string.as_str()).map_err(|e| AppError {
            code: AppErrorCode::InvalidJsonFormat,
            detail: Some(e.to_string()),
        })
    }
} // end of impl TestWebServer

// higher-ranked trait bound ?
pub fn deserialize_json_template<T: for<'a> Deserialize<'a>>(
    basepath: &AppBasepathCfg,
    file_localpath: &str,
) -> DefaultResult<T, AppError> {
    let fullpath = basepath.service.clone() + "/" + file_localpath;
    let reader = File::open(fullpath).map_err(|e| AppError {
        detail: Some(file_localpath.to_string()),
        code: AppErrorCode::IOerror(e.kind()),
    })?;
    serde_json::from_reader::<File, T>(reader).map_err(|e| AppError {
        detail: Some(e.to_string()),
        code: AppErrorCode::InvalidJsonFormat,
    })
}
