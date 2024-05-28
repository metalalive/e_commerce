use actix_web::http::header::{ContentType, CONTENT_TYPE};
use actix_web::web::{Data as WebData, Json as ExtJson, Path as ExtPath};
use actix_web::{HttpResponse, Result as ActixResult};

use ecommerce_common::logging::{app_log_event, AppLogLevel};

use super::dto::ChargeReqDto;
use crate::adapter::repository::app_repo_charge;
use crate::usecase::ChargeCreateUseCase;
use crate::{AppAuthedClaim, AppSharedState};

pub(super) async fn create_charge(
    req_body: ExtJson<ChargeReqDto>,
    authed_claim: AppAuthedClaim,
    shr_state: WebData<AppSharedState>,
) -> ActixResult<HttpResponse> {
    let logctx = shr_state.log_context();
    let logctx_p = &logctx;
    app_log_event!(logctx_p, AppLogLevel::DEBUG, "create-charge-api");

    let repo = match app_repo_charge(shr_state.datastore()).await {
        Ok(v) => v,
        Err(e) => {
            app_log_event!(logctx_p, AppLogLevel::ERROR, "repo-init-error {:?}", e);
            let resp = HttpResponse::InternalServerError()
                .append_header(ContentType::plaintext())
                .body("");
            return Ok(resp);
        }
    };
    let uc = ChargeCreateUseCase {
        repo,
        processors: shr_state.processor_context(),
        ordersync_lockset: shr_state.ordersync_lockset(),
        rpc_ctx: shr_state.rpc_context(),
    };
    let usr_id = 123;
    let req_body = req_body.into_inner();
    let result = uc.execute(usr_id, req_body).await;
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
