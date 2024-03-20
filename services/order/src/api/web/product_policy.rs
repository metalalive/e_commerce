use std::vec::Vec;
use std::result::Result as DefaultResult;

use axum::debug_handler;
use axum::extract::{Json as ExtractJson, State as ExtractState};
use axum::response::IntoResponse;
use axum::http::{
    StatusCode as HttpStatusCode,
    HeaderMap as HttpHeaderMap,
    HeaderValue as HttpHeaderValue,
    header as HttpHeader
};

use crate::error::{AppErrorCode, AppError};
use crate::{constant as AppConst, AppSharedState, AppAuthedClaim};
use crate::api::web::dto::ProductPolicyDto;
use crate::api::rpc::{py_celery_serialize, py_celery_deserialize_reply};
use crate::usecase::{
    EditProductPolicyUseCase, EditProductPolicyResult, ProductInfoReq, ProductInfoResp
};

use super::dto::ProductPolicyClientErrorDto;


fn presenter(
    result: EditProductPolicyResult,
    client_err: Option<Vec<ProductPolicyClientErrorDto>>
) -> impl IntoResponse
{
    let resp_ctype_val = HttpHeaderValue::from_str(AppConst::HTTP_CONTENT_TYPE_JSON).unwrap();
    let mut hdr_map = HttpHeaderMap::new();
    hdr_map.insert(HttpHeader::CONTENT_TYPE, resp_ctype_val);
    let default_body = "{}".to_string();
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
    }
} // end of fn presenter


#[debug_handler(state = AppSharedState)]
pub(super) async fn post_handler(
    // wrap the variables with the macros, to extract the content automatically
    authed: AppAuthedClaim,
    ExtractState(appstate): ExtractState<AppSharedState>,
    ExtractJson(req_body): ExtractJson<Vec<ProductPolicyDto>> ) -> impl IntoResponse
{
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
        data: req_body, log, dstore, rpc_ctx, profile_id : authed.profile,
        rpc_serialize_msg: py_celery_serialize::<ProductInfoReq>,
        rpc_deserialize_msg 
    };
    let (result, client_err) = input.execute().await ;
    presenter(result, client_err)
} // end of endpoint

fn  _rpc_deserialize_dummy(_raw:&Vec<u8>) -> DefaultResult<ProductInfoResp, AppError>
{
    Err(AppError {code:AppErrorCode::RpcRemoteInvalidReply, detail:None})
}
