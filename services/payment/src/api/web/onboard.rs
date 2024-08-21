use actix_web::http::header::{ContentType, CONTENT_TYPE};
use actix_web::http::StatusCode;
use actix_web::web::{Data as AppData, Json as ExtJson, Path as ExtPath};
use actix_web::{HttpResponse, HttpResponseBuilder, Result as ActixResult};

use ecommerce_common::logging::{app_log_event, AppLogLevel};

use super::dto::{
    StoreOnboardAcceptedRespDto, StoreOnboardReqDto, StoreOnboardStatusDto,
    StoreOnboardStatusReqDto,
};
use crate::auth::AppAuthedClaim;
use crate::AppSharedState;

pub(super) async fn onboard_store(
    path_segms: ExtPath<(u32,)>,
    _req_body: ExtJson<StoreOnboardReqDto>,
    _authed_claim: AppAuthedClaim,
    shr_state: AppData<AppSharedState>,
) -> ActixResult<HttpResponse> {
    let store_id = path_segms.into_inner().0;
    let logctx = shr_state.log_context();
    app_log_event!(logctx, AppLogLevel::DEBUG, "store_id : {store_id}");
    let usecase_result = StoreOnboardAcceptedRespDto::Stripe;
    let body_raw = serde_json::to_vec(&usecase_result).unwrap();
    let http_status = StatusCode::ACCEPTED;
    let resp = {
        let mut r = HttpResponseBuilder::new(http_status);
        let header = (CONTENT_TYPE, ContentType::json());
        r.append_header(header);
        r.body(body_raw)
    };
    Ok(resp)
}

pub(super) async fn track_onboarding_status(
    path_segms: ExtPath<(u32,)>,
    _req_body: ExtJson<StoreOnboardStatusReqDto>,
    _authed_claim: AppAuthedClaim,
    shr_state: AppData<AppSharedState>,
) -> ActixResult<HttpResponse> {
    let store_id = path_segms.into_inner().0;
    let logctx = shr_state.log_context();
    app_log_event!(logctx, AppLogLevel::DEBUG, "{store_id}");
    let usecase_result = StoreOnboardStatusDto::Stripe;
    let body_raw = serde_json::to_vec(&usecase_result).unwrap();
    let http_status = StatusCode::OK;
    let resp = {
        let mut r = HttpResponseBuilder::new(http_status);
        let header = (CONTENT_TYPE, ContentType::json());
        r.append_header(header);
        r.body(body_raw)
    };
    Ok(resp)
}
