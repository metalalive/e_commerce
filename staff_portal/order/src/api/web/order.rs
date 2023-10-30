use axum::debug_handler;
use axum::response::IntoResponse;
use axum::extract::{
    Json as ExtractJson,    Path as ExtractPath,
    Query as ExtractQuery,  State as ExtractState,
};
use axum::http::{
    StatusCode as HttpStatusCode,
    HeaderMap as HttpHeaderMap,
    HeaderValue as HttpHeaderValue,
    header as HttpHeader
};
use serde_json;

use crate::{constant as AppConst, AppSharedState, app_log_event};
use crate::api::web::dto::{OrderCreateReqData, OrderEditReqData};
use crate::logging::AppLogLevel;
use crate::repository::{app_repo_order, app_repo_product_price, app_repo_product_policy};
use crate::usecase::{CreateOrderUseCase, CreateOrderUsKsErr};


// always to specify state type explicitly to the debug macro
#[debug_handler(state=AppSharedState)]
pub(crate) async fn post_handler(
    ExtractState(_appstate): ExtractState<AppSharedState>,
    _wrapped_req_body: ExtractJson<OrderCreateReqData> ) -> impl IntoResponse
{
    let ExtractJson(req_body) = _wrapped_req_body;
    let usr_prof_id:u32  = 1234; // TODO, use auth token (e.g. JWT)
    let log_ctx = _appstate.log_context().clone();
    let ds = _appstate.datastore();
    let results = (app_repo_order(ds.clone()).await,
                   app_repo_product_price(ds.clone()).await,
                   app_repo_product_policy(ds).await );
    let (resp_status_code, serial_resp_body) = if let (Ok(repo_o), Ok(repo_price),
        Ok(repo_policy)) = results
    {
        let uc = CreateOrderUseCase {glb_state:_appstate, repo_price, repo_policy,
            repo_order:repo_o, usr_id:usr_prof_id};
        match uc.execute(req_body).await {
            Ok(value) => match serde_json::to_string(&value) {
                Ok(s) => (HttpStatusCode::CREATED, s),
                Err(_) => (HttpStatusCode::INTERNAL_SERVER_ERROR, 
                           "{\"reason\":\"serialization-faulire\"}".to_string()),
            },
            Err(errwrap) => match errwrap {
                CreateOrderUsKsErr::Client(value) => match serde_json::to_string(&value) {
                    Ok(s) => (HttpStatusCode::BAD_REQUEST, s),
                    Err(_) => (HttpStatusCode::INTERNAL_SERVER_ERROR, 
                           "{\"reason\":\"serialization-faulire\"}".to_string()),
                },
                CreateOrderUsKsErr::Server => (HttpStatusCode::INTERNAL_SERVER_ERROR, 
                           "{\"reason\":\"internal-error\"}".to_string()),
            }
        }
    } else {
        let mut errmsgs = Vec::new();
        if let Err(e) = results.0 {
            errmsgs.push(e.to_string());
        } // TODO, improve error message format
        if let Err(e) = results.1 {
            errmsgs.push(e.to_string());
        }
        if let Err(e) = results.2 {
            errmsgs.push(e.to_string());
        }
        app_log_event!(log_ctx, AppLogLevel::ERROR,
            "repository init failure, reason: {:?} ", errmsgs);
        (HttpStatusCode::INTERNAL_SERVER_ERROR,
             r#"{"reason":"internal-error"}"#.to_string())
    };
    let resp_ctype_val = HttpHeaderValue::from_str(AppConst::HTTP_CONTENT_TYPE_JSON).unwrap();
    let mut hdr_map = HttpHeaderMap::new();
    hdr_map.insert(HttpHeader::CONTENT_TYPE, resp_ctype_val);
    (resp_status_code, hdr_map, serial_resp_body)
} // end of post_handler


#[debug_handler(state=AppSharedState)]
pub(crate) async fn patch_handler (
    oid:ExtractPath<String>,
    billing:Option<ExtractQuery<bool>>,
    shipping:Option<ExtractQuery<bool>>,
    ExtractState(_appstate): ExtractState<AppSharedState>,
    _req_body: ExtractJson<OrderEditReqData>) -> impl IntoResponse
{
    let resp_ctype_val = HttpHeaderValue::from_str(AppConst::HTTP_CONTENT_TYPE_JSON).unwrap();
    let mut hdr_map = HttpHeaderMap::new();
    hdr_map.insert(HttpHeader::CONTENT_TYPE, resp_ctype_val);
    let serial_resp_body = "{}".to_string();
    let log_ctx = _appstate.log_context();
    app_log_event!(log_ctx, AppLogLevel::INFO,
            "edited contact info of the order {} ", oid.clone());
    (HttpStatusCode::OK, hdr_map, serial_resp_body)
}

