use actix_web::http::header::{ContentType, CONTENT_TYPE};
use actix_web::web::{Data as WebData, Json as ExtJson, Path as ExtPath};
use actix_web::{HttpResponse, Result as ActixResult};

use ecommerce_common::logging::{app_log_event, AppLogLevel};

use super::dto::ChargeReqDto;
use crate::AppSharedState;

pub(super) async fn create_charge(
    _req_body: ExtJson<ChargeReqDto>,
    shr_state: WebData<AppSharedState>,
) -> ActixResult<HttpResponse> {
    let logctx = shr_state.log_context();
    let logctx_p = &logctx;
    app_log_event!(logctx_p, AppLogLevel::DEBUG, "create-charge-api");
    let resp = HttpResponse::Accepted()
        .append_header(ContentType::json())
        .body("{}");
    Ok(resp)
}

pub(super) async fn refresh_charge_status(
    _path: ExtPath<String>,
    shr_state: WebData<AppSharedState>,
) -> ActixResult<HttpResponse> {
    let logctx = shr_state.log_context();
    let logctx_p = &logctx;
    app_log_event!(logctx_p, AppLogLevel::DEBUG, "refresh-charge-status");
    let resp = HttpResponse::Ok()
        .append_header((CONTENT_TYPE.as_str(), "application/json"))
        .body("{}");
    Ok(resp)
}
