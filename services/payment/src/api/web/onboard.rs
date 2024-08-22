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
use crate::usecase::{OnboardStoreUcError, OnboardStoreUcOk, OnboardStoreUseCase};
use crate::AppSharedState;

use super::dto::{StoreOnboardReqDto, StoreOnboardStatusDto, StoreOnboardStatusReqDto};
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
    let (body_raw, http_status) = match result {
        Ok(u) => match u {
            OnboardStoreUcOk::Accepted(v) => {
                (serde_json::to_vec(&v).unwrap(), StatusCode::ACCEPTED)
            }
        },
        Err(uce) => {
            let status = match uce {
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

    let resp = {
        let mut r = HttpResponseBuilder::new(http_status);
        let header = (CONTENT_TYPE, ContentType::json());
        r.append_header(header);
        r.body(body_raw)
    };
    Ok(resp)
} // end of fn onboard_store

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
