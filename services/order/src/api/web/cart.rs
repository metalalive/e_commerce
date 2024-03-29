use axum::debug_handler;
use axum::response::IntoResponse;
use axum::extract::{
    Json as ExtractJson, Path as ExtractPath, State as ExtractState,
};
use axum::http::{
    HeaderMap, HeaderValue, header, StatusCode
};

use crate::{AppSharedState, AppAuthedClaim};
use crate::constant::HTTP_CONTENT_TYPE_JSON;
use crate::logging::{app_log_event, AppLogLevel};
use crate::repository::app_repo_cart;
use crate::usecase::{
    ModifyCartLineUseCase, ModifyCartUsKsResult, DiscardCartUseCase, DiscardCartUsKsResult,
    RetrieveCartUseCase, RetrieveCartUsKsResult
};

use super::dto::CartDto;

#[debug_handler(state=AppSharedState)]
pub(super) async fn modify_lines(
    ExtractPath(seq_num): ExtractPath<u8>,
    authed_usr: AppAuthedClaim,
    ExtractState(appstate): ExtractState<AppSharedState>,
    ExtractJson(req_body): ExtractJson<CartDto>
) -> impl IntoResponse
{
    let resp_ctype_val = HeaderValue::from_str(HTTP_CONTENT_TYPE_JSON).unwrap();
    let mut hdr_map = HeaderMap::new();
    hdr_map.insert(header::CONTENT_TYPE, resp_ctype_val);
    let default_body = "{}".to_string();
    
    let logctx = appstate.log_context().clone();
    app_log_event!(logctx, AppLogLevel::DEBUG,"seq_num:{seq_num}");
    
    let repo = match app_repo_cart(appstate.datastore()).await {
        Ok(v) => v,
        Err(e) => {
            app_log_event!(logctx, AppLogLevel::ERROR,"{:?}", e);
            return (StatusCode::INTERNAL_SERVER_ERROR, hdr_map, default_body);
        }
    };
    let uc =  ModifyCartLineUseCase {repo, authed_usr};
    let (status, resp_body) = match uc.execute(seq_num, req_body).await {
        ModifyCartUsKsResult::Success => (StatusCode::OK, default_body),
        ModifyCartUsKsResult::NotFound => (StatusCode::NOT_FOUND, default_body),
        ModifyCartUsKsResult::QuotaExceed(e) =>
            (StatusCode::BAD_REQUEST, serde_json::to_string(&e).unwrap()),
        ModifyCartUsKsResult::ServerError(e) => {
            app_log_event!(logctx, AppLogLevel::ERROR,"{:?}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, default_body)
        },
    };
    (status, hdr_map, resp_body)
} // end of fn modify_lines

#[debug_handler(state=AppSharedState)]
pub(super) async fn discard(
    ExtractPath(seq_num): ExtractPath<u8>,
    authed_usr: AppAuthedClaim,
    ExtractState(appstate): ExtractState<AppSharedState>
) -> impl IntoResponse
{
    let logctx = appstate.log_context().clone();
    let repo = match app_repo_cart(appstate.datastore()).await {
        Ok(v) => v,
        Err(e) => {
            app_log_event!(logctx, AppLogLevel::ERROR,"{:?}", e);
            return (StatusCode::INTERNAL_SERVER_ERROR, HeaderMap::new());
        }
    };
    let uc =  DiscardCartUseCase {repo, authed_usr};
    let status = match uc.execute(seq_num).await {
        DiscardCartUsKsResult::Success => StatusCode::NO_CONTENT,
        DiscardCartUsKsResult::NotFound => StatusCode::GONE,
        DiscardCartUsKsResult::ServerError(e) => {
            app_log_event!(logctx, AppLogLevel::ERROR,"{:?}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        },
    };
    (status, HeaderMap::new())
}

#[debug_handler(state=AppSharedState)]
pub(super) async fn retrieve(
    ExtractPath(seq_num): ExtractPath<u8>,
    authed_usr: AppAuthedClaim,
    ExtractState(appstate): ExtractState<AppSharedState>
) -> impl IntoResponse
{
    let hdr_map = {
        let resp_ctype_val = HeaderValue::from_str(HTTP_CONTENT_TYPE_JSON).unwrap();
        let mut hmap = HeaderMap::new();
        hmap.insert(header::CONTENT_TYPE, resp_ctype_val);
        hmap
    };
    let default_body = "{}".to_string(); 
    let logctx = appstate.log_context().clone();
    
    let repo = match app_repo_cart(appstate.datastore()).await {
        Ok(v) => v,
        Err(e) => {
            app_log_event!(logctx, AppLogLevel::ERROR,"{:?}", e);
            return (StatusCode::INTERNAL_SERVER_ERROR, hdr_map, default_body);
        }
    };
    let uc =  RetrieveCartUseCase {repo, authed_usr};
    let (status, resp_body) = match uc.execute(seq_num).await {
        RetrieveCartUsKsResult::Success(v) =>
            (StatusCode::OK, serde_json::to_string(&v).unwrap()),
        RetrieveCartUsKsResult::NotFound => (StatusCode::NOT_FOUND, default_body),
        RetrieveCartUsKsResult::ServerError(e) => {
            app_log_event!(logctx, AppLogLevel::ERROR,"{:?}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, default_body)
        },
    };
    (status, hdr_map, resp_body)
}
