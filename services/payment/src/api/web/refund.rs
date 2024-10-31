use std::sync::Arc;

use actix_web::error::Error as ActixError;
use actix_web::http::header::{ContentType, CONTENT_TYPE};
use actix_web::http::StatusCode;
use actix_web::web::{Data as AppData, Json as ExtJson, Path as ExtPath};
use actix_web::{HttpResponse, HttpResponseBuilder, Result as ActixResult};

use ecommerce_common::logging::{app_log_event, AppLogContext, AppLogLevel};

use crate::adapter::datastore::AppDataStoreContext;
use crate::adapter::repository::{app_repo_refund, AbstractRefundRepo, AppRepoErrorDetail};
use crate::auth::AppAuthedClaim;
use crate::usecase::{FinalizeRefundUcError, FinalizeRefundUseCase};
use crate::AppSharedState;

use super::charge::try_creating_charge_repo;
use super::dto::RefundCompletionReqDto;
use super::onboard::try_creating_merchant_repo;
use super::RepoInitFailure;

async fn try_creating_refund_repo(
    dstore: Arc<AppDataStoreContext>,
    logctx: Arc<AppLogContext>,
) -> ActixResult<Box<dyn AbstractRefundRepo>> {
    app_repo_refund(dstore).await.map_err(|e_repo| {
        app_log_event!(logctx, AppLogLevel::ERROR, "repo-init-error {:?}", e_repo);
        ActixError::from(RepoInitFailure)
    })
}

pub(super) async fn mechant_complete_refund(
    path_segms: ExtPath<(String, u32)>,
    ExtJson(req_body): ExtJson<RefundCompletionReqDto>,
    auth_claim: AppAuthedClaim,
    shr_state: AppData<AppSharedState>,
) -> ActixResult<HttpResponse> {
    let (oid, store_id) = path_segms.into_inner();
    let logctx = shr_state.log_context();

    let dstore = shr_state.datastore();
    let repo_ch = try_creating_charge_repo(dstore.clone(), logctx.clone()).await?;
    let repo_mc = try_creating_merchant_repo(dstore.clone(), logctx.clone()).await?;
    let repo_rfd = try_creating_refund_repo(dstore.clone(), logctx.clone()).await?;

    let processors = shr_state.processor_context();
    let uc = FinalizeRefundUseCase {
        repo_ch,
        repo_mc,
        repo_rfd,
        processors,
    };
    let result = uc.execute(oid, store_id, auth_claim, req_body).await;
    let (http_status, body_raw) = match result {
        Ok((o, es)) => {
            if !es.is_empty() {
                app_log_event!(logctx, AppLogLevel::ERROR, "{:?}", es);
            }
            (StatusCode::OK, serde_json::to_vec(&o).unwrap())
        }
        Err(e) => {
            let err_status = match e {
                FinalizeRefundUcError::MissingMerchant(err_store_id) => {
                    app_log_event!(logctx, AppLogLevel::INFO, "{err_store_id}");
                    StatusCode::FORBIDDEN
                }
                FinalizeRefundUcError::MissingChargeId(oid) => {
                    app_log_event!(logctx, AppLogLevel::DEBUG, "{oid}");
                    StatusCode::NOT_FOUND
                }
                FinalizeRefundUcError::MissingCharge(buyer_id, ctime) => {
                    app_log_event!(logctx, AppLogLevel::ERROR, "{buyer_id}-{ctime}");
                    StatusCode::INTERNAL_SERVER_ERROR
                }
                FinalizeRefundUcError::PermissionDenied(auth_usr_id) => {
                    app_log_event!(logctx, AppLogLevel::WARNING, "{auth_usr_id}");
                    StatusCode::FORBIDDEN
                }
                FinalizeRefundUcError::DataStore(re) => {
                    let client_err = matches!(re.detail, AppRepoErrorDetail::RefundResolution(_));
                    if client_err {
                        app_log_event!(logctx, AppLogLevel::WARNING, "{:?}", re);
                        StatusCode::BAD_REQUEST
                    } else {
                        app_log_event!(logctx, AppLogLevel::ERROR, "{:?}", re);
                        StatusCode::INTERNAL_SERVER_ERROR
                    }
                }
            };
            (err_status, Vec::new())
        }
    };
    let resp = {
        let mut r = HttpResponseBuilder::new(http_status);
        let header = (CONTENT_TYPE, ContentType::json());
        r.append_header(header);
        r.body(body_raw)
    };
    Ok(resp)
} // end of fn mechant_complete_refund
