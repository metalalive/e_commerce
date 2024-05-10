use axum::debug_handler;
use axum::extract::{
    Json as ExtractJson, Path as ExtractPath, Query as ExtractQuery, State as ExtractState,
};
use axum::http::{
    header as HttpHeader, HeaderMap as HttpHeaderMap, HeaderValue as HttpHeaderValue,
    StatusCode as HttpStatusCode,
};
use axum::response::IntoResponse;
use serde_json;

use ecommerce_common::logging::{app_log_event, AppLogLevel};

use crate::api::web::dto::{OrderCreateReqData, OrderEditReqData, OrderLineReqDto};
use crate::constant as AppConst;
use crate::repository::{
    app_repo_order, app_repo_order_return, app_repo_product_policy, app_repo_product_price,
};
use crate::usecase::{
    CreateOrderUsKsErr, CreateOrderUseCase, ReturnLinesReqUcOutput, ReturnLinesReqUseCase,
};
use crate::{AppAuthedClaim, AppSharedState};

// always to specify state type explicitly to the debug macro
#[debug_handler(state=AppSharedState)]
pub(super) async fn create_handler(
    authed: AppAuthedClaim,
    ExtractState(_appstate): ExtractState<AppSharedState>,
    _wrapped_req_body: ExtractJson<OrderCreateReqData>,
) -> impl IntoResponse {
    let usr_id = authed.profile;
    let ExtractJson(req_body) = _wrapped_req_body;
    let log_ctx = _appstate.log_context().clone();
    let ds = _appstate.datastore();
    let results = (
        app_repo_order(ds.clone()).await,
        app_repo_product_price(ds.clone()).await,
        app_repo_product_policy(ds).await,
    );
    let (resp_status_code, serial_resp_body) =
        if let (Ok(repo_o), Ok(repo_price), Ok(repo_policy)) = results {
            let uc = CreateOrderUseCase {
                glb_state: _appstate,
                repo_price,
                repo_policy,
                repo_order: repo_o,
                auth_claim: authed,
            };
            match uc.execute(req_body).await {
                Ok(value) => match serde_json::to_string(&value) {
                    Ok(s) => (HttpStatusCode::CREATED, s),
                    Err(_) => (
                        HttpStatusCode::INTERNAL_SERVER_ERROR,
                        r#"{"reason":"serialization-faulire"}"#.to_string(),
                    ),
                },
                Err(errwrap) => match errwrap {
                    CreateOrderUsKsErr::ReqContent(value) => match serde_json::to_string(&value) {
                        Ok(s) => (HttpStatusCode::BAD_REQUEST, s),
                        Err(_) => (
                            HttpStatusCode::INTERNAL_SERVER_ERROR,
                            "{\"reason\":\"serialization-faulire\"}".to_string(),
                        ),
                    },
                    CreateOrderUsKsErr::Quota(value) => match serde_json::to_string(&value) {
                        Ok(s) => (HttpStatusCode::FORBIDDEN, s),
                        Err(_) => (
                            HttpStatusCode::INTERNAL_SERVER_ERROR,
                            "{\"reason\":\"serialization-faulire\"}".to_string(),
                        ),
                    },
                    CreateOrderUsKsErr::Server(errors) => {
                        let msg = errors
                            .into_iter()
                            .map(|e| format!("{:?}", e))
                            .collect::<Vec<_>>()
                            .join(", ");
                        app_log_event!(log_ctx, AppLogLevel::ERROR, "{msg}");
                        (
                            HttpStatusCode::INTERNAL_SERVER_ERROR,
                            r#"{"reason":"internal-error"}"#.to_string(),
                        )
                    }
                },
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
            app_log_event!(
                log_ctx,
                AppLogLevel::ERROR,
                "repository init failure, user:{}, reason: {:?} ",
                usr_id,
                errmsgs
            );
            (
                HttpStatusCode::INTERNAL_SERVER_ERROR,
                r#"{"reason":"internal-error"}"#.to_string(),
            )
        };
    let resp_ctype_val = HttpHeaderValue::from_str(AppConst::HTTP_CONTENT_TYPE_JSON).unwrap();
    let mut hdr_map = HttpHeaderMap::new();
    hdr_map.insert(HttpHeader::CONTENT_TYPE, resp_ctype_val);
    (resp_status_code, hdr_map, serial_resp_body)
} // end of create_handler

#[debug_handler(state=AppSharedState)]
pub(super) async fn return_lines_request_handler(
    ExtractPath(oid): ExtractPath<String>,
    authed_claim: AppAuthedClaim,
    ExtractState(_app_state): ExtractState<AppSharedState>,
    ExtractJson(req_body): ExtractJson<Vec<OrderLineReqDto>>,
) -> impl IntoResponse {
    let logctx = _app_state.log_context().clone();
    let usr_prof_id = authed_claim.profile;
    let ds = _app_state.datastore();
    let results = (
        app_repo_order(ds.clone()).await,
        app_repo_order_return(ds).await,
    );
    let (status_code, resp_body) = if results.0.is_ok() && results.1.is_ok() {
        let o_repo = results.0.unwrap();
        let or_repo = results.1.unwrap();
        let uc = ReturnLinesReqUseCase {
            authed_claim,
            o_repo,
            or_repo,
            logctx: logctx.clone(),
        };
        match uc.execute(oid.clone(), req_body).await {
            Ok(output) => match output {
                ReturnLinesReqUcOutput::Success => (HttpStatusCode::OK, r#"{}"#.to_string()),
                ReturnLinesReqUcOutput::InvalidOwner | ReturnLinesReqUcOutput::PermissionDeny => {
                    (HttpStatusCode::FORBIDDEN, r#"{}"#.to_string())
                }
                ReturnLinesReqUcOutput::InvalidRequest(errors) => {
                    let serialized = serde_json::to_string(&errors).unwrap();
                    (HttpStatusCode::BAD_REQUEST, serialized)
                }
            },
            Err(e) => {
                app_log_event!(
                    logctx,
                    AppLogLevel::ERROR,
                    "internal error from use-case, oid:{}, user:{}, reason:{:?}",
                    oid.as_str(),
                    usr_prof_id,
                    e
                );
                (HttpStatusCode::INTERNAL_SERVER_ERROR, r#"{}"#.to_string())
            }
        }
    } else {
        if let Err(e) = results.0.as_ref() {
            app_log_event!(
                logctx,
                AppLogLevel::ERROR,
                "failed to init order repo, oid:{}, user:{}, reason:{:?}",
                oid,
                usr_prof_id,
                e
            );
        }
        if let Err(e) = results.1.as_ref() {
            app_log_event!(
                logctx,
                AppLogLevel::ERROR,
                "failed to init order-return repo, oid:{}, user:{}, reason:{:?}",
                oid,
                usr_prof_id,
                e
            );
        }
        (HttpStatusCode::INTERNAL_SERVER_ERROR, r#"{}"#.to_string())
    };
    let resp_ctype_val = HttpHeaderValue::from_str(AppConst::HTTP_CONTENT_TYPE_JSON).unwrap();
    let hdr_kv_pairs = [(HttpHeader::CONTENT_TYPE, resp_ctype_val)];
    let hdr_map = HttpHeaderMap::from_iter(hdr_kv_pairs.into_iter());
    (status_code, hdr_map, resp_body)
} // end of return_lines_request_handler

#[debug_handler(state=AppSharedState)]
pub(super) async fn edit_billing_shipping_handler(
    oid: ExtractPath<String>,
    _billing: Option<ExtractQuery<bool>>,
    _shipping: Option<ExtractQuery<bool>>,
    _authed: AppAuthedClaim,
    ExtractState(_appstate): ExtractState<AppSharedState>,
    _req_body: ExtractJson<OrderEditReqData>,
) -> impl IntoResponse {
    let resp_ctype_val = HttpHeaderValue::from_str(AppConst::HTTP_CONTENT_TYPE_JSON).unwrap();
    let mut hdr_map = HttpHeaderMap::new();
    hdr_map.insert(HttpHeader::CONTENT_TYPE, resp_ctype_val);
    let serial_resp_body = "{}".to_string();
    let log_ctx = _appstate.log_context();
    app_log_event!(
        log_ctx,
        AppLogLevel::INFO,
        "edited contact info of the order {} ",
        oid.clone()
    );
    (HttpStatusCode::OK, hdr_map, serial_resp_body)
}
