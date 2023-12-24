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
use crate::{constant as AppConst, AppSharedState, AppAuthedClaim};
use crate::api::web::dto::ProductPolicyDto;
use crate::usecase::{EditProductPolicyUseCase, EditProductPolicyResult};

fn presenter (ucout:EditProductPolicyUseCase) -> impl IntoResponse
{
    let resp_ctype_val = HttpHeaderValue::from_str(AppConst::HTTP_CONTENT_TYPE_JSON).unwrap();
    let mut hdr_map = HttpHeaderMap::new();
    hdr_map.insert(HttpHeader::CONTENT_TYPE, resp_ctype_val);
    let default_body = "{}".to_string();
    if let EditProductPolicyUseCase::OUTPUT { result, client_err } = ucout
    {
        let status = match result {
            EditProductPolicyResult::OK => HttpStatusCode::OK,
            EditProductPolicyResult::Other(ec) =>
                match ec {
                    AppErrorCode::InvalidInput => HttpStatusCode::BAD_REQUEST,
                    AppErrorCode::RpcRemoteUnavail => HttpStatusCode::SERVICE_UNAVAILABLE,
                    AppErrorCode::RpcRemoteInvalidReply => HttpStatusCode::NOT_IMPLEMENTED,
                    _others => HttpStatusCode::INTERNAL_SERVER_ERROR,
                }
        };
        let serial_resp_body = if let Some(ce) = client_err {
            let value = serde_json::to_value(ce).unwrap();
            value.to_string()
        } else { default_body.clone() } ;
        (status, hdr_map, serial_resp_body)
    } else {
        (HttpStatusCode::INTERNAL_SERVER_ERROR, hdr_map, default_body)
    }
} // end of fn presenter

#[debug_handler(state = AppSharedState)]
pub(super) async fn post_handler(
    // wrap the variables with the macros, to extract the content automatically
    authed: AppAuthedClaim,
    ExtractState(appstate): ExtractState<AppSharedState>,
    ExtractJson(req_body): ExtractJson<Vec<ProductPolicyDto>> ) -> impl IntoResponse
{
    let input = EditProductPolicyUseCase::INPUT {
        data: req_body, app_state: appstate, profile_id : authed.profile
    };
    let output = input.execute().await ;
    presenter(output)
} // end of endpoint
