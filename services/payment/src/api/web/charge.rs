use std::boxed::Box;
use std::sync::Arc;

use actix_web::body::BoxBody;
use actix_web::error::{Error as ActixError, ResponseError};
use actix_web::http::header::{ContentType, TryIntoHeaderValue, CONTENT_TYPE};
use actix_web::http::StatusCode;
use actix_web::web::{Data as WebData, Json as ExtJson, Path as ExtPath};
use actix_web::{HttpResponse, HttpResponseBuilder, Result as ActixResult};

use ecommerce_common::logging::{app_log_event, AppLogContext, AppLogLevel};

use super::dto::{CapturePay3partyRespDto, CapturePayReqDto, CapturePayRespDto, ChargeReqDto};
use crate::adapter::datastore::AppDataStoreContext;
use crate::adapter::repository::{app_repo_charge, AbstractChargeRepo};
use crate::usecase::{
    ChargeCreateUcError, ChargeCreateUseCase, ChargeRefreshUcError, ChargeStatusRefreshUseCase,
};
use crate::{AppAuthedClaim, AppSharedState};

#[derive(Debug)]
struct RepoInitFailure;

impl std::fmt::Display for RepoInitFailure {
    fn fmt(&self, _f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Ok(())
    }
}
impl ResponseError for RepoInitFailure {
    fn status_code(&self) -> StatusCode {
        StatusCode::SERVICE_UNAVAILABLE
    }
    fn error_response(&self) -> HttpResponse<BoxBody> {
        HttpResponse::ServiceUnavailable()
            .append_header(ContentType::plaintext())
            .body("")
    }
}
async fn try_creating_charge_repo(
    dstore: Arc<AppDataStoreContext>,
    logctx: Arc<AppLogContext>,
) -> ActixResult<Box<dyn AbstractChargeRepo>> {
    app_repo_charge(dstore).await.map_err(|e_repo| {
        app_log_event!(logctx, AppLogLevel::ERROR, "repo-init-error {:?}", e_repo);
        ActixError::from(RepoInitFailure)
    })
}

pub(super) async fn create_charge(
    req_body: ExtJson<ChargeReqDto>,
    authed_claim: AppAuthedClaim,
    shr_state: WebData<AppSharedState>,
) -> ActixResult<HttpResponse> {
    let logctx = shr_state.log_context();
    let logctx_p = &logctx;
    app_log_event!(logctx_p, AppLogLevel::DEBUG, "create-charge-api");

    let repo = try_creating_charge_repo(shr_state.datastore(), logctx.clone()).await?;
    let uc = ChargeCreateUseCase {
        repo,
        processors: shr_state.processor_context(),
        ordersync_lockset: shr_state.ordersync_lockset(),
        rpc_ctx: shr_state.rpc_context(),
    };
    let req_body = req_body.into_inner();
    let resp = match uc.execute(authed_claim.profile, req_body).await {
        // TODO, return session detail from chosen 3rd-party processor
        Ok(v) => {
            let body_serial = serde_json::to_vec(&v).unwrap();
            HttpResponse::Accepted()
                .append_header(ContentType::json())
                .body(body_serial)
        }
        Err(uce) => match uce {
            ChargeCreateUcError::ClientBadRequest(e) => {
                let body = serde_json::to_vec(&e).unwrap();
                HttpResponse::BadRequest()
                    .append_header(ContentType::json())
                    .body(body)
            }
            ChargeCreateUcError::RpcOlineParseError(es) => {
                app_log_event!(logctx_p, AppLogLevel::ERROR, "{:?}", es);
                HttpResponse::UnprocessableEntity().finish()
            }
            ChargeCreateUcError::ExternalProcessorError(e) => {
                app_log_event!(logctx_p, AppLogLevel::ERROR, "{:?}", e);
                HttpResponse::InternalServerError().finish()
            }
            ChargeCreateUcError::DataStoreError(e) => {
                app_log_event!(logctx_p, AppLogLevel::ERROR, "{:?}", e);
                HttpResponse::InternalServerError().finish()
            }
            ChargeCreateUcError::LoadOrderInternalError(_) => {
                app_log_event!(logctx_p, AppLogLevel::ERROR, "order-rpc-failure");
                HttpResponse::InternalServerError().finish()
            }
            _others => {
                app_log_event!(logctx_p, AppLogLevel::ERROR, "unclassified-error");
                HttpResponse::InternalServerError().finish()
            }
        }, // analyze error type, give different error response
    }; // end of use-case execution
    Ok(resp)
} // end of fn create_charge

pub(super) async fn refresh_charge_status(
    path_segms: ExtPath<(String,)>,
    authed_claim: AppAuthedClaim,
    shr_state: WebData<AppSharedState>,
) -> ActixResult<HttpResponse> {
    let charge_id_serial = path_segms.into_inner().0;
    let logctx = shr_state.log_context();
    app_log_event!(logctx, AppLogLevel::DEBUG, "charge-id: {charge_id_serial}");

    let repo = try_creating_charge_repo(shr_state.datastore(), logctx.clone()).await?;
    let uc = ChargeStatusRefreshUseCase {
        repo,
        processors: shr_state.processor_context(),
        rpc_ctx: shr_state.rpc_context(),
    };
    let result = uc.execute(authed_claim.profile, charge_id_serial).await;
    let (http_status, body) = match result {
        Ok(v) => {
            let b = serde_json::to_vec(&v).unwrap();
            (StatusCode::OK, b)
        }
        Err(e) => {
            let s = match e {
                ChargeRefreshUcError::OwnerMismatch => StatusCode::FORBIDDEN,
                ChargeRefreshUcError::ChargeNotExist(owner_id, ctime) => {
                    app_log_event!(logctx, AppLogLevel::DEBUG, "{owner_id}, {ctime}");
                    StatusCode::NOT_FOUND
                }
                ChargeRefreshUcError::RpcContext(_e) => {
                    app_log_event!(logctx, AppLogLevel::ERROR, "low-lvl-rpc-ctx");
                    StatusCode::SERVICE_UNAVAILABLE
                }
                ChargeRefreshUcError::DataStore(e) => {
                    app_log_event!(logctx, AppLogLevel::ERROR, "{:?}", e);
                    StatusCode::SERVICE_UNAVAILABLE
                }
                ChargeRefreshUcError::ChargeIdDecode(ecode, detail) => {
                    app_log_event!(
                        logctx,
                        AppLogLevel::WARNING,
                        "code:{:?}, detail:{}",
                        ecode,
                        detail
                    );
                    StatusCode::BAD_REQUEST
                }
                ChargeRefreshUcError::RpcContentSerialisation(detail) => {
                    app_log_event!(logctx, AppLogLevel::ERROR, "detail:{detail}");
                    StatusCode::INTERNAL_SERVER_ERROR
                }
                ChargeRefreshUcError::ExternalProcessor(e) => {
                    app_log_event!(logctx, AppLogLevel::ERROR, "{:?}", e);
                    StatusCode::SERVICE_UNAVAILABLE
                }
                ChargeRefreshUcError::RpcUpdateOrder(e) => {
                    app_log_event!(logctx, AppLogLevel::ERROR, "{:?}", e);
                    StatusCode::INTERNAL_SERVER_ERROR
                }
            };
            (s, b"{}".to_vec())
        }
    };
    let resp = {
        let mut r = HttpResponse::new(http_status);
        let ctype = ContentType::json().try_into_value().unwrap();
        r.headers_mut().insert(CONTENT_TYPE, ctype);
        r.set_body(BoxBody::new(body))
    };
    Ok(resp)
}

pub(super) async fn capture_authorized_charge(
    path_segms: ExtPath<(String,)>,
    req_body: ExtJson<CapturePayReqDto>,
    _authed_claim: AppAuthedClaim,
    shr_state: WebData<AppSharedState>,
) -> ActixResult<HttpResponse> {
    let charge_id = path_segms.into_inner().0;
    let store_id = req_body.store_id;
    let logctx = shr_state.log_context();
    app_log_event!(logctx, AppLogLevel::DEBUG, "{charge_id}, {store_id}");
    let usecase_result = CapturePayRespDto {
        store_id,
        processor: CapturePay3partyRespDto::Stripe,
    };
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
