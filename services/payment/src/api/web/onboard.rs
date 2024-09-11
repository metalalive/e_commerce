use std::boxed::Box;
use std::sync::Arc;

use actix_web::error::Error as ActixError;
use actix_web::http::header::{ContentType, CONTENT_TYPE};
use actix_web::http::StatusCode;
use actix_web::web::{Data as AppData, Json as ExtJson, Path as ExtPath};
use actix_web::{HttpResponse, HttpResponseBuilder, Result as ActixResult};

use ecommerce_common::logging::{app_log_event, AppLogContext, AppLogLevel};

use crate::adapter::datastore::AppDataStoreContext;
use crate::adapter::repository::{app_repo_merchant, AbstractMerchantRepo};
use crate::auth::AppAuthedClaim;
use crate::usecase::{OnboardStoreUcError, OnboardStoreUseCase, RefreshOnboardStatusUseCase};
use crate::AppSharedState;

use super::dto::{StoreOnboardReqDto, StoreOnboardRespDto};
use super::RepoInitFailure;

async fn try_creating_merchant_repo(
    dstore: Arc<AppDataStoreContext>,
    logctx: Arc<AppLogContext>,
) -> ActixResult<Box<dyn AbstractMerchantRepo>> {
    app_repo_merchant(dstore).await.map_err(|e_repo| {
        app_log_event!(logctx, AppLogLevel::ERROR, "repo-init-error {:?}", e_repo);
        ActixError::from(RepoInitFailure)
    })
}

fn usecase_result_to_httpresp(
    logctx: Arc<AppLogContext>,
    result: Result<StoreOnboardRespDto, OnboardStoreUcError>,
) -> HttpResponse {
    let (body_raw, http_status) = match result {
        Ok(u) => {
            let status = if u.is_complete() {
                StatusCode::OK
            } else {
                StatusCode::ACCEPTED
            };
            (serde_json::to_vec(&u).unwrap(), status)
        }
        Err(uce) => {
            let status = match uce {
                OnboardStoreUcError::InvalidStoreSupervisor(usr_id) => {
                    app_log_event!(logctx, AppLogLevel::INFO, "{usr_id}");
                    StatusCode::FORBIDDEN
                }
                OnboardStoreUcError::ThirdParty(e) => {
                    app_log_event!(logctx, AppLogLevel::ERROR, "{:?}", e);
                    StatusCode::SERVICE_UNAVAILABLE
                }
                OnboardStoreUcError::RepoCreate(e) => {
                    app_log_event!(logctx, AppLogLevel::ERROR, "{:?}", e);
                    StatusCode::SERVICE_UNAVAILABLE
                }
                OnboardStoreUcError::RpcStoreReplica(_e) => {
                    app_log_event!(logctx, AppLogLevel::ERROR, "low-lvl-rpc-ctx");
                    StatusCode::SERVICE_UNAVAILABLE
                }
                OnboardStoreUcError::RpcMsgSerialize(code, detail) => {
                    app_log_event!(logctx, AppLogLevel::ERROR, "{:?} {}", code, detail);
                    StatusCode::INTERNAL_SERVER_ERROR
                }
                OnboardStoreUcError::CorruptedStoreProfile(orig_msg_raw, err_detail) => {
                    let orig_msg_portion = &orig_msg_raw[0..20];
                    app_log_event!(
                        logctx,
                        AppLogLevel::ERROR,
                        "orig_msg_raw:{:?}, err_detail:{}",
                        orig_msg_portion,
                        err_detail
                    );
                    StatusCode::INTERNAL_SERVER_ERROR
                }
                OnboardStoreUcError::InvalidStoreProfile(e) => {
                    app_log_event!(logctx, AppLogLevel::ERROR, "{:?}", e);
                    StatusCode::INTERNAL_SERVER_ERROR
                }
            };
            (b"{}".to_vec(), status)
        }
    }; // end of match use-case result

    let mut r = HttpResponseBuilder::new(http_status);
    let header = (CONTENT_TYPE, ContentType::json());
    r.append_header(header);
    r.body(body_raw)
} // end of fn usecase_result_to_httpresp

pub(super) async fn onboard_store(
    path_segms: ExtPath<(u32,)>,
    ExtJson(req_body): ExtJson<StoreOnboardReqDto>,
    auth_claim: AppAuthedClaim,
    shr_state: AppData<AppSharedState>,
) -> ActixResult<HttpResponse> {
    let store_id = path_segms.into_inner().0;
    let logctx = shr_state.log_context();
    app_log_event!(logctx, AppLogLevel::DEBUG, "store_id : {store_id}");

    let repo = try_creating_merchant_repo(shr_state.datastore(), logctx.clone()).await?;
    let processors = shr_state.processor_context();
    let rpc_ctx = shr_state.rpc_context();
    let uc = OnboardStoreUseCase {
        auth_claim,
        processors,
        rpc_ctx,
        repo,
    };
    let result = uc.execute(store_id, req_body).await;
    let resp = usecase_result_to_httpresp(logctx, result);
    Ok(resp)
} // end of fn onboard_store

pub(super) async fn track_onboarding_status(
    path_segms: ExtPath<(u32,)>,
    ExtJson(req_body): ExtJson<StoreOnboardReqDto>,
    auth_claim: AppAuthedClaim,
    shr_state: AppData<AppSharedState>,
) -> ActixResult<HttpResponse> {
    let store_id = path_segms.into_inner().0;
    let logctx = shr_state.log_context();
    app_log_event!(logctx, AppLogLevel::DEBUG, "{store_id}");

    let repo = try_creating_merchant_repo(shr_state.datastore(), logctx.clone()).await?;
    let processors = shr_state.processor_context();
    let uc = RefreshOnboardStatusUseCase {
        repo,
        auth_claim,
        processors,
    };
    let result = uc.execute(store_id, req_body).await;
    let resp = usecase_result_to_httpresp(logctx, result);
    Ok(resp)
} // end of fn track_onboarding_status
