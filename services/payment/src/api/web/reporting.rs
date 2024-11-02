use actix_web::http::header::{ContentType, CONTENT_TYPE};
use actix_web::http::StatusCode;
use actix_web::web::{Data as AppData, Path as ExtPath, Query as ExtQuery};
use actix_web::{HttpResponse, HttpResponseBuilder, Result as ActixResult};

use ecommerce_common::logging::{app_log_event, AppLogLevel};

use crate::api::web::dto::{ReportChargeRespDto, ReportTimeRangeDto};
use crate::auth::AppAuthedClaim;
use crate::AppSharedState;

pub(super) async fn report_charge_lines(
    path_m: ExtPath<(u32,)>,
    query_m: ExtQuery<ReportTimeRangeDto>,
    _auth_claim: AppAuthedClaim,
    shr_state: AppData<AppSharedState>,
) -> ActixResult<HttpResponse> {
    let store_id = path_m.into_inner().0;
    let time_range = query_m.into_inner();

    let logctx = shr_state.log_context();
    app_log_event!(logctx, AppLogLevel::DEBUG, "{store_id}, {:?}", &time_range);

    let resp = ReportChargeRespDto;
    let body_raw = serde_json::to_vec(&resp).unwrap();
    let http_status = StatusCode::OK;
    let mut r = HttpResponseBuilder::new(http_status);
    let header = (CONTENT_TYPE, ContentType::json());
    r.append_header(header);
    Ok(r.body(body_raw))
} // end of fn report_charge_lines
