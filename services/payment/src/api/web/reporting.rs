use std::boxed::Box;
use std::sync::Arc;

use actix_web::error::Error as ActixError;
use actix_web::http::header::{ContentType, CONTENT_TYPE};
use actix_web::http::StatusCode;
use actix_web::web::{Data as AppData, Path as ExtPath, Query as ExtQuery};
use actix_web::{HttpResponse, HttpResponseBuilder, Result as ActixResult};

use ecommerce_common::logging::{app_log_event, AppLogContext, AppLogLevel};

use super::onboard::try_creating_merchant_repo;
use super::RepoInitFailure;
use crate::adapter::datastore::AppDataStoreContext;
use crate::adapter::repository::{app_repo_reporting, AbstractReportingRepo};
use crate::api::web::dto::ReportTimeRangeDto;
use crate::auth::AppAuthedClaim;
use crate::usecase::MerchantReportChargeUseCase;
use crate::AppSharedState;

async fn try_creating_reporting_repo(
    dstore: Arc<AppDataStoreContext>,
    logctx: Arc<AppLogContext>,
) -> ActixResult<Box<dyn AbstractReportingRepo>> {
    app_repo_reporting(dstore).await.map_err(|e_repo| {
        app_log_event!(logctx, AppLogLevel::ERROR, "repo-init-error {:?}", e_repo);
        ActixError::from(RepoInitFailure)
    })
}

pub(super) async fn report_charge_lines(
    path_m: ExtPath<(u32,)>,
    query_m: ExtQuery<ReportTimeRangeDto>,
    auth_claim: AppAuthedClaim,
    shr_state: AppData<AppSharedState>,
) -> ActixResult<HttpResponse> {
    let store_id = path_m.into_inner().0;
    let time_range = query_m.into_inner();

    let logctx = shr_state.log_context();
    app_log_event!(logctx, AppLogLevel::DEBUG, "{store_id}, {:?}", &time_range);
    let dstore = shr_state.datastore();

    let repo_mc = try_creating_merchant_repo(dstore.clone(), logctx.clone()).await?;
    let repo_rpt = try_creating_reporting_repo(dstore, logctx.clone()).await?;
    let uc = MerchantReportChargeUseCase::new(auth_claim, repo_mc, repo_rpt);
    let result = uc.execute(store_id, time_range).await;
    let (http_status, body_raw) = match result {
        Ok(v) => {
            let body_raw = serde_json::to_vec(&v).unwrap();
            (StatusCode::OK, body_raw)
        }
        Err(_e) => {
            // TODO, finish implementation
            (StatusCode::NOT_IMPLEMENTED, Vec::new())
        }
    };
    let mut r = HttpResponseBuilder::new(http_status);
    let header = (CONTENT_TYPE, ContentType::json());
    r.append_header(header);
    Ok(r.body(body_raw))
} // end of fn report_charge_lines
