use std::result::Result as DefaultResult;
use std::vec::Vec;

use axum::debug_handler;
use axum::extract::{Json as ExtractJson, State as ExtractState};
use axum::http::{
    header as HttpHeader, HeaderMap as HttpHeaderMap, HeaderValue as HttpHeaderValue,
    StatusCode as HttpStatusCode,
};
use axum::response::IntoResponse;

use ecommerce_common::error::AppErrorCode;

use crate::api::rpc::{py_celery_deserialize_reply, py_celery_serialize};
use crate::api::web::dto::ProductPolicyDto;
use crate::error::AppError;
use crate::usecase::{
    EditProductPolicyResult, EditProductPolicyUseCase, ProductInfoReq, ProductInfoResp,
};
use crate::{constant as AppConst, AppAuthedClaim, AppSharedState};

fn presenter(result: EditProductPolicyResult) -> impl IntoResponse {
    let resp_ctype_val = HttpHeaderValue::from_str(AppConst::HTTP_CONTENT_TYPE_JSON).unwrap();
    let mut hdr_map = HttpHeaderMap::new();
    hdr_map.insert(HttpHeader::CONTENT_TYPE, resp_ctype_val);
    let default_body = "{}".to_string();
    let (status, serial_resp_body) = match result {
        EditProductPolicyResult::OK => (HttpStatusCode::OK, default_body),
        EditProductPolicyResult::PermissionDeny => (HttpStatusCode::FORBIDDEN, default_body),
        EditProductPolicyResult::QuotaExceed(detail) => (
            HttpStatusCode::FORBIDDEN,
            serde_json::to_string(&detail).unwrap(),
        ),
        EditProductPolicyResult::ClientError(ce) => (
            HttpStatusCode::BAD_REQUEST,
            serde_json::to_value(ce).unwrap().to_string(),
        ),
        EditProductPolicyResult::Other(ec) => {
            let s = match ec {
                AppErrorCode::RpcRemoteUnavail => HttpStatusCode::SERVICE_UNAVAILABLE,
                AppErrorCode::RpcRemoteInvalidReply => HttpStatusCode::NOT_IMPLEMENTED,
                _others => HttpStatusCode::INTERNAL_SERVER_ERROR,
            };
            (s, default_body)
        }
    };
    (status, hdr_map, serial_resp_body)
} // end of fn presenter

#[debug_handler(state = AppSharedState)]
pub(super) async fn post_handler(
    // wrap the variables with the macros, to extract the content automatically
    authed_usr: AppAuthedClaim,
    ExtractState(appstate): ExtractState<AppSharedState>,
    ExtractJson(req_body): ExtractJson<Vec<ProductPolicyDto>>,
) -> impl IntoResponse {
    let log = appstate.log_context().clone();
    let dstore = appstate.datastore();
    let rpc_ctx = appstate.rpc();
    // if RPC client handler trait adds serialize / deserialize methods with generic
    // type parameter , it will make the code more complex,
    // TODO, find better approach to improve code quality
    let rpc_deserialize_msg = if rpc_ctx.label() == "dummy" {
        _rpc_deserialize_dummy
    } else {
        py_celery_deserialize_reply::<ProductInfoResp>
    };
    let input = EditProductPolicyUseCase {
        data: req_body,
        log,
        dstore,
        rpc_ctx,
        authed_usr,
        rpc_deserialize_msg,
        rpc_serialize_msg: py_celery_serialize::<ProductInfoReq>,
    };
    let result = input.execute().await;
    presenter(result)
} // end of endpoint

fn _rpc_deserialize_dummy(_raw: &Vec<u8>) -> DefaultResult<ProductInfoResp, AppError> {
    Err(AppError {
        code: AppErrorCode::RpcRemoteInvalidReply,
        detail: None,
    })
}
