use std::sync::Arc;

use axum::debug_handler;
use axum::extract::{Json as ExtractJson, State as ExtractState};
use axum::response::IntoResponse;
use axum::http::{
    StatusCode as HttpStatusCode,
    HeaderMap as HttpHeaderMap,
    HeaderValue as HttpHeaderValue,
    header as HttpHeader
};

use crate::error::AppErrorCode;
use crate::logging::{AppLogLevel, AppLogContext};
use crate::{constant as AppConst, AppSharedState, app_log_event};
use crate::api::web::dto::ProductPolicyDto;
use crate::usecase::{EditProductPolicyUseCase, EditProductPolicyResult};

fn presenter (ucout:EditProductPolicyUseCase, log_ctx:Arc<AppLogContext>)
    -> impl IntoResponse
{
    let resp_ctype_val = HttpHeaderValue::from_str(AppConst::HTTP_CONTENT_TYPE_JSON).unwrap();
    let mut hdr_map = HttpHeaderMap::new();
    hdr_map.insert(HttpHeader::CONTENT_TYPE, resp_ctype_val);
    let default_body = "{}".to_string();
    if let EditProductPolicyUseCase::OUTPUT { result, detail } = ucout
    {
        let status = match result {
            EditProductPolicyResult::OK => HttpStatusCode::OK,
            EditProductPolicyResult::ProductNotExists => HttpStatusCode::BAD_REQUEST,
            EditProductPolicyResult::Other(ec) =>
                match ec {
                    AppErrorCode::InvalidInput => HttpStatusCode::BAD_REQUEST,
                    AppErrorCode::RpcRemoteUnavail => HttpStatusCode::SERVICE_UNAVAILABLE,
                    AppErrorCode::RpcRemoteInvalidReply => HttpStatusCode::NOT_IMPLEMENTED,
                    _others => HttpStatusCode::INTERNAL_SERVER_ERROR,
                }
        };
        let serial_resp_body = {
            let detail = detail.unwrap_or(default_body.clone());
            // TODO, move to middleware ? avoid writing internal server 
            // to response body
            let is_srv_err = status.ge(&HttpStatusCode::INTERNAL_SERVER_ERROR);
            let is_nonhttp_err = status.lt(&HttpStatusCode::OK);
            if is_srv_err || is_nonhttp_err {
                app_log_event!(log_ctx, AppLogLevel::ERROR, "detail:{} ", detail);
                default_body.clone()
            } else { detail }
        };
        (status, hdr_map, serial_resp_body)
    } else {
        (HttpStatusCode::INTERNAL_SERVER_ERROR, hdr_map, default_body)
    }
} // end of fn presenter

#[debug_handler(state = AppSharedState)]
pub(crate) async fn post_handler(
    // wrap the variables with the macros, to extract the content automatically
    ExtractState(appstate): ExtractState<AppSharedState>,
    ExtractJson(req_body): ExtractJson<Vec<ProductPolicyDto>> ) -> impl IntoResponse
{
    let log_ctx = appstate.log_context().clone();
    // TODO, extract user profile ID from authenticated JWT
    let input = EditProductPolicyUseCase::INPUT {
        data: req_body, app_state: appstate, profile_id : 1234u32
    };
    let output = input.execute().await ;
    presenter(output, log_ctx)
} // end of endpoint
