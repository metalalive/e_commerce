use std::env;
use std::fs::File;
use std::sync::{Once, Arc};
use std::result::Result as DefaultResult ;
use std::borrow::BorrowMut;
use std::collections::HashMap;
use std::collections::hash_map::RandomState;

use serde::Deserialize;
use tokio::sync::Mutex;
use tower::Service;
use http::{Request, Response};
use http_body::Body as RawHttpBody; // required by UnsyncBoxBody, to access raw data of body
use http_body::combinators::UnsyncBoxBody;
use hyper::body::{Body as HyperBody, Bytes as HyperBytes};
use axum_core::Error as AxumCoreError;

use order::{AppConfig, AppSharedState, AppBasepathCfg};
use order::error::{AppError, AppErrorCode};
use order::logging::AppLogContext;
use order::constant::EXPECTED_ENV_VAR_LABELS;
use order::network::{generate_web_service, WebApiServer};
use order::api::web::route_table;


static mut GLOBAL_SHARED_STATE : Option<DefaultResult<AppSharedState, AppError>> = None;
static mut SHARED_WEB_SERVER : Option<Arc<Mutex<WebApiServer>>> = None;

static GLB_STATE_INIT : Once = Once::new();
static WEB_SRV_INIT : Once = Once::new();


pub fn test_setup_shr_state() -> DefaultResult<AppSharedState, AppError>
{
    GLB_STATE_INIT.call_once(|| {
        let iter = env::vars().filter(
            |(k, _)| {EXPECTED_ENV_VAR_LABELS.contains(&k.as_str())}
        );
        let args: HashMap<String, String, RandomState> = HashMap::from_iter(iter);
        match AppConfig::new(args) {
            Ok(top_lvl_cfg) => {
                let log_ctx = AppLogContext::new(
                    &top_lvl_cfg.basepath, &top_lvl_cfg.api_server.logging
                );
                let obj = AppSharedState::new(top_lvl_cfg, log_ctx);
                unsafe { GLOBAL_SHARED_STATE = Some(Ok(obj)); }
            },
            Err(e) => unsafe { GLOBAL_SHARED_STATE = Some(Err(e)); }
        };
    });
    unsafe {
        match &GLOBAL_SHARED_STATE {
            Some(r) => match r {
                    Ok(state) => Ok(state.clone()),
                    Err(e) => Err(e.clone())
                },
            _others => {panic!("[test] shared state failed to create")}
        }
    }
} // end of test_setup_shr_state


pub struct TestWebServer {}
type InnerRespBody = UnsyncBoxBody<HyperBytes, AxumCoreError>; 

impl TestWebServer {
    pub fn setup (shr_state:AppSharedState) -> Arc<Mutex<WebApiServer>>
    {
        WEB_SRV_INIT.call_once(|| {
            let rtable = route_table();
            let top_lvl_cfg = shr_state.config().clone();
            let listener = & top_lvl_cfg.api_server.listen ;
            let (_, srv) = generate_web_service(listener, rtable, shr_state);
            let srv = Arc::new(Mutex::new(srv));
            unsafe { SHARED_WEB_SERVER = Some(srv); }
        });
        unsafe {
            match &SHARED_WEB_SERVER {
                Some(s) => s.clone(),
                _others => {panic!("[test] web server failed to create")}
            }
        }
    }

    pub async fn consume (srv: &Arc<Mutex<WebApiServer>>, req:Request<HyperBody>)
        -> Response<InnerRespBody> {
        let mut guard = srv.lock().await;
        let inner_sv = guard.borrow_mut();
        let result = inner_sv.call(req).await;
        result.unwrap()
    }

    pub async fn to_custom_type<T:for<'a> Deserialize<'a>> (body: & mut InnerRespBody)
        -> DefaultResult<T, AppError>
    {
        let mut _err = AppError{code: AppErrorCode::Unknown, detail:None};
        let x = if let Some(r) = body.data().await {
            match r {
                Ok(b) => b,
                Err(e) => {
                    _err.detail = Some(e.to_string());  return Err(_err);
                }
            }
        } else {
            let s = "no response body".to_string();
            _err.detail = Some(s);  return Err(_err);
        };
        let x = x.to_vec();
        let x = match String::from_utf8(x) {
            Ok(b) => b,
            Err(e) => {
                let s = e.utf8_error().to_string();
                _err.detail = Some(s);  return Err(_err);
            }
        };
        match serde_json::from_str::<T>(x.as_str()) {
            Ok(obj) => Ok(obj),
            Err(e) => {
                _err.code = AppErrorCode::InvalidJsonFormat;
                _err.detail = Some(e.to_string());
                Err(_err)
            }
        }
    } // end of to_custom_type
} // end of impl TestWebServer


// higher-ranked trait bound ?
pub fn deserialize_json_template<T: for<'a> Deserialize<'a>>
    // <'a, T:'a + Deserialize<'a>>
    (basepath:&AppBasepathCfg, file_localpath:&str)
    -> DefaultResult<T, AppError>
{
    let fullpath = basepath.service.clone() + "/" + file_localpath;
    let reader = match File::open(fullpath) {
        Ok(g) => g,
        Err(e) => { return Err(AppError{
            detail:Some(file_localpath.to_string()),
            code: AppErrorCode::IOerror(e.kind()) });
        }
    };
    match serde_json::from_reader::<File, T>(reader) {
        Ok(obj) => Ok(obj),
        Err(e) => Err(AppError{ detail: Some(e.to_string()),
            code: AppErrorCode::InvalidJsonFormat  })
    }
}
