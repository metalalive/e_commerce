use actix_web::http::header::{ContentType, CONTENT_TYPE};
use actix_web::http::StatusCode;
use actix_web::web::{Data as AppData, Json as ExtJson, Path as ExtPath};
use actix_web::{HttpResponse, HttpResponseBuilder, Result as ActixResult};

use ecommerce_common::logging::{app_log_event, AppLogLevel};

use crate::auth::AppAuthedClaim;
use crate::AppSharedState;

use super::dto::{RefundCompletionReqDto, RefundCompletionRespDto};

pub(super) async fn mechant_complete_refund(
    path_segms: ExtPath<(String, u32)>,
    ExtJson(req_body): ExtJson<RefundCompletionReqDto>,
    _auth_claim: AppAuthedClaim,
    shr_state: AppData<AppSharedState>,
) -> ActixResult<HttpResponse> {
    let (oid, store_id) = path_segms.into_inner();
    let logctx = shr_state.log_context();
    app_log_event!(logctx, AppLogLevel::DEBUG, "{oid}, {store_id}");

    let body_raw = {
        let o = RefundCompletionRespDto {
            req_time: req_body.req_time,
            lines: Vec::new(),
        };
        serde_json::to_vec(&o).unwrap()
    };
    let http_status = StatusCode::OK;
    let resp = {
        let mut r = HttpResponseBuilder::new(http_status);
        let header = (CONTENT_TYPE, ContentType::json());
        r.append_header(header);
        r.body(body_raw)
    };
    Ok(resp)
} // end of fn track_onboarding_status
