use std::env;
use std::fs::File;
use std::sync::Arc;

use chrono::{Duration, Local};
use serde_json::Value as JsnVal;

use ecommerce_common::constant::env_vars::SERVICE_BASEPATH;
use ecommerce_common::error::AppErrorCode;

use payment::adapter::processor::{
    AbstractPaymentProcessor, AppProcessorError, AppProcessorErrorReason, AppProcessorFnLabel,
    AppProcessorMerchantResult,
};
use payment::adapter::repository::{AppRepoError, AppRepoErrorDetail, AppRepoErrorFnLabel};
use payment::adapter::rpc::{AbstractRpcContext, AppRpcReply};
use payment::api::web::dto::StoreOnboardRespDto;
use payment::model::{Merchant3partyModel, MerchantModelError, MerchantProfileModel};
use payment::usecase::{OnboardStoreUcError, OnboardStoreUseCase, RefreshOnboardStatusUseCase};

use super::{
    MockMerchantRepo, MockPaymentProcessor, MockRpcClient, MockRpcContext, MockRpcPublishEvent,
};
use crate::auth::ut_setup_auth_claim;
use crate::dto::{ut_default_store_onboard_req_stripe, ut_setup_storeprofile_dto};
use crate::model::ut_default_merchant_3party_stripe;
use crate::EXAMPLE_REL_PATH;

fn ut_rpc_storeprof_replica(mock_filename: &str) -> Vec<u8> {
    let basepath = env::var(SERVICE_BASEPATH).unwrap();
    let fullpath = basepath + EXAMPLE_REL_PATH + mock_filename;
    let file = File::open(fullpath).unwrap();
    let mut obj = serde_json::from_reader::<File, JsnVal>(file).unwrap();
    let mock_staff = obj
        .as_object_mut()
        .unwrap()
        .get_mut("result")
        .unwrap()
        .as_object_mut()
        .unwrap()
        .get_mut("staff")
        .unwrap()
        .as_array_mut()
        .unwrap();
    let startafter = Local::now().fixed_offset();
    let endbefore = startafter + Duration::minutes(19);
    mock_staff
        .iter_mut()
        .map(|v| {
            let start = v.get_mut("start_after").unwrap();
            *start = JsnVal::String(startafter.to_rfc3339());
            let end = v.get_mut("end_before").unwrap();
            *end = JsnVal::String(endbefore.to_rfc3339());
        })
        .count();
    serde_json::to_vec(&obj).unwrap()
} // end of fn ut_rpc_storeprof_replica

fn ut_setup_rpc_ctx(reply_raw_msg: Vec<u8>) -> Arc<Box<dyn AbstractRpcContext>> {
    let reply = AppRpcReply {
        message: reply_raw_msg,
    };
    let mock_evt = MockRpcPublishEvent::build(Some(Ok(reply)));
    let mock_client = MockRpcClient::build(Some(Ok(mock_evt)));
    let mock_ctx = MockRpcContext::build(Some(Ok(mock_client)));
    Arc::new(mock_ctx)
}

fn ut_setup_processor(
    res: Option<Result<AppProcessorMerchantResult, AppProcessorError>>,
) -> Box<dyn AbstractPaymentProcessor> {
    MockPaymentProcessor::build(None, None, res, None, None)
}

#[actix_web::test]
async fn new_merchant_ok() {
    let auth_claim = ut_setup_auth_claim(1234, 85);
    let processors = {
        let pay3pty_result = Ok(AppProcessorMerchantResult::default());
        let m3pty = ut_setup_processor(Some(pay3pty_result));
        Arc::new(m3pty)
    };
    let repo = MockMerchantRepo::build(Some(Ok(())), None, None, None);
    let rpc_ctx = {
        let msg = ut_rpc_storeprof_replica("store_profile_replica_dto_1.json");
        ut_setup_rpc_ctx(msg)
    };
    let mock_store_id = 1008;
    let req_body = ut_default_store_onboard_req_stripe();
    let uc = OnboardStoreUseCase {
        auth_claim,
        processors,
        repo,
        rpc_ctx,
    };
    let result = uc.execute(mock_store_id, req_body).await;
    assert!(result.is_ok());
    if let Ok(v) = result {
        let cond = matches!(v, StoreOnboardRespDto::Unknown);
        assert!(cond);
    }
} // end of fn new_merchant_ok

#[actix_web::test]
async fn new_merchant_err_rpc_reply() {
    let auth_claim = ut_setup_auth_claim(1001, 79);
    let processors = {
        let pay3pty_result = Ok(AppProcessorMerchantResult::default());
        let m3pty = ut_setup_processor(Some(pay3pty_result));
        Arc::new(m3pty)
    };
    let repo = MockMerchantRepo::build(Some(Ok(())), None, None, None);
    let rpc_ctx = ut_setup_rpc_ctx(Vec::new());
    let mock_store_id = 1009;
    let req_body = ut_default_store_onboard_req_stripe();
    let uc = OnboardStoreUseCase {
        auth_claim,
        processors,
        repo,
        rpc_ctx,
    };
    let result = uc.execute(mock_store_id, req_body).await;
    assert!(result.is_err());
    if let Err(e) = result {
        let cond = matches!(e, OnboardStoreUcError::CorruptedStoreProfile(_, _));
        assert!(cond);
    }
} // end of fn new_merchant_err_rpc_reply

#[actix_web::test]
async fn new_merchant_3party_failure() {
    let auth_claim = ut_setup_auth_claim(1234, 85);
    let processors = {
        let pay3pty_result = Err(AppProcessorError {
            reason: AppProcessorErrorReason::InvalidMethod("unit-test".to_string()),
            fn_label: AppProcessorFnLabel::OnboardMerchant,
        });
        let m3pty = ut_setup_processor(Some(pay3pty_result));
        Arc::new(m3pty)
    };
    let repo = MockMerchantRepo::build(Some(Ok(())), None, None, None);
    let rpc_ctx = {
        let msg = ut_rpc_storeprof_replica("store_profile_replica_dto_1.json");
        ut_setup_rpc_ctx(msg)
    };
    let mock_store_id = 1010;
    let req_body = ut_default_store_onboard_req_stripe();
    let uc = OnboardStoreUseCase {
        auth_claim,
        processors,
        repo,
        rpc_ctx,
    };
    let result = uc.execute(mock_store_id, req_body).await;
    assert!(result.is_err());
    if let Err(e) = result {
        if let OnboardStoreUcError::ThirdParty(pe) = e {
            let cond = matches!(pe.fn_label, AppProcessorFnLabel::OnboardMerchant);
            assert!(cond);
            if let AppProcessorErrorReason::InvalidMethod(name) = pe.reason {
                assert_eq!(name.as_str(), "unit-test");
            } else {
                assert!(false);
            }
        } else {
            assert!(false);
        }
    }
} // end of fn new_merchant_3party_failure

#[actix_web::test]
async fn new_merchant_err_repo_create() {
    let auth_claim = ut_setup_auth_claim(1234, 85);
    let processors = {
        let pay3pty_result = Ok(AppProcessorMerchantResult::default());
        let m3pty = ut_setup_processor(Some(pay3pty_result));
        Arc::new(m3pty)
    };
    let repo = {
        let err = Err(AppRepoError {
            fn_label: AppRepoErrorFnLabel::CreateMerchant,
            code: AppErrorCode::RemoteDbServerFailure,
            detail: AppRepoErrorDetail::DatabaseExec("unit-test".to_string()),
        });
        MockMerchantRepo::build(Some(err), None, None, None)
    };
    let rpc_ctx = {
        let msg = ut_rpc_storeprof_replica("store_profile_replica_dto_1.json");
        ut_setup_rpc_ctx(msg)
    };
    let mock_store_id = 1012;
    let req_body = ut_default_store_onboard_req_stripe();
    let uc = OnboardStoreUseCase {
        auth_claim,
        processors,
        repo,
        rpc_ctx,
    };
    let result = uc.execute(mock_store_id, req_body).await;
    assert!(result.is_err());
    if let Err(e) = result {
        if let OnboardStoreUcError::RepoCreate(re) = e {
            let cond = matches!(re.fn_label, AppRepoErrorFnLabel::CreateMerchant);
            assert!(cond);
            assert_eq!(re.code, AppErrorCode::RemoteDbServerFailure);
            if let AppRepoErrorDetail::DatabaseExec(name) = re.detail {
                assert_eq!(name.as_str(), "unit-test");
            } else {
                assert!(false);
            }
        } else {
            assert!(false);
        }
    }
} // end of fn new_merchant_err_repo_create

fn ut_setup_store_models(
    store_id: u32,
    supervisor_id: u32,
) -> (MerchantProfileModel, Merchant3partyModel) {
    let mock_store_d = ut_setup_storeprofile_dto(
        "Social Splash Taco House",
        supervisor_id,
        vec![1236, 1237, 1239],
        Local::now().to_utc(),
    );
    let arg = (store_id, &mock_store_d);
    let mock_storeprof_m = MerchantProfileModel::try_from(arg).unwrap();
    let mock_store3pty_m = Merchant3partyModel::Stripe(ut_default_merchant_3party_stripe());
    (mock_storeprof_m, mock_store3pty_m)
}

#[actix_web::test]
async fn refresh_status_ok() {
    let mock_store_id = 1012;
    let mock_supervisor_id = 1230;
    let auth_claim = ut_setup_auth_claim(mock_supervisor_id, 85);
    let processors = {
        let pay3pty_result = Ok(AppProcessorMerchantResult::default());
        let m3pty = ut_setup_processor(Some(pay3pty_result));
        Arc::new(m3pty)
    };
    let repo = {
        let arg = ut_setup_store_models(mock_store_id, mock_supervisor_id);
        MockMerchantRepo::build(None, Some(arg), None, Some(Ok(())))
    };
    let req_body = ut_default_store_onboard_req_stripe();
    let uc = RefreshOnboardStatusUseCase {
        auth_claim,
        processors,
        repo,
    };
    let result = uc.execute(mock_store_id, req_body).await;
    assert!(result.is_ok());
    if let Ok(v) = result {
        // in this test case I am concerned only about the workflow, not the response detail
        // TODO, consider to improve this test case
        let cond = matches!(v, StoreOnboardRespDto::Unknown);
        assert!(cond);
    }
} // end of fn refresh_status_ok

#[actix_web::test]
async fn refresh_status_err_merchant_empty() {
    let mock_store_id = 1012;
    let mock_supervisor_id = 1230;
    let auth_claim = ut_setup_auth_claim(mock_supervisor_id, 85);
    let processors = Arc::new(ut_setup_processor(None));
    let repo = MockMerchantRepo::build(None, None, None, None);
    let req_body = ut_default_store_onboard_req_stripe();
    let uc = RefreshOnboardStatusUseCase {
        auth_claim,
        processors,
        repo,
    };
    let result = uc.execute(mock_store_id, req_body).await;
    assert!(result.is_err());
    if let Err(e) = result {
        let cond = matches!(
            e,
            OnboardStoreUcError::InvalidStoreProfile(MerchantModelError::NotExist)
        );
        assert!(cond);
    }
} // end of fn refresh_status_err_merchant_empty

#[actix_web::test]
async fn refresh_status_3party_failure() {
    let mock_store_id = 1012;
    let mock_supervisor_id = 1230;
    let auth_claim = ut_setup_auth_claim(mock_supervisor_id, 85);
    let processors = {
        let pay3pty_result = Err(AppProcessorError {
            reason: AppProcessorErrorReason::InvalidMethod("unit-test".to_string()),
            fn_label: AppProcessorFnLabel::RefreshOnboardStatus,
        });
        let m3pty = ut_setup_processor(Some(pay3pty_result));
        Arc::new(m3pty)
    };
    let repo = {
        let arg = ut_setup_store_models(mock_store_id, mock_supervisor_id);
        MockMerchantRepo::build(None, Some(arg), None, None)
    };
    let req_body = ut_default_store_onboard_req_stripe();
    let uc = RefreshOnboardStatusUseCase {
        auth_claim,
        processors,
        repo,
    };
    let result = uc.execute(mock_store_id, req_body).await;
    assert!(result.is_err());
    if let Err(uce) = result {
        if let OnboardStoreUcError::ThirdParty(e) = uce {
            let cond = matches!(e.fn_label, AppProcessorFnLabel::RefreshOnboardStatus);
            assert!(cond);
            if let AppProcessorErrorReason::InvalidMethod(s) = e.reason {
                assert_eq!(s.as_str(), "unit-test");
            } else {
                assert!(false);
            }
        } else {
            assert!(false);
        }
    }
} // end of fn refresh_status_3party_failure

#[actix_web::test]
async fn refresh_status_err_repo_update() {
    let mock_store_id = 1012;
    let mock_supervisor_id = 1230;
    let auth_claim = ut_setup_auth_claim(mock_supervisor_id, 85);
    let processors = {
        let pay3pty_result = Ok(AppProcessorMerchantResult::default());
        let m3pty = ut_setup_processor(Some(pay3pty_result));
        Arc::new(m3pty)
    };
    let repo = {
        let arg = ut_setup_store_models(mock_store_id, mock_supervisor_id);
        let e = AppRepoError {
            fn_label: AppRepoErrorFnLabel::UpdateMerchant3party,
            code: AppErrorCode::DataCorruption,
            detail: AppRepoErrorDetail::DatabaseExec("unit-test".to_string()),
        };
        MockMerchantRepo::build(None, Some(arg), None, Some(Err(e)))
    };
    let req_body = ut_default_store_onboard_req_stripe();
    let uc = RefreshOnboardStatusUseCase {
        auth_claim,
        processors,
        repo,
    };
    let result = uc.execute(mock_store_id, req_body).await;
    assert!(result.is_err());
    if let Err(uce) = result {
        if let OnboardStoreUcError::RepoCreate(e) = uce {
            let cond = matches!(e.fn_label, AppRepoErrorFnLabel::UpdateMerchant3party);
            assert!(cond);
            if let AppRepoErrorDetail::DatabaseExec(s) = e.detail {
                assert_eq!(s.as_str(), "unit-test");
            } else {
                assert!(false);
            }
        } else {
            assert!(false);
        }
    }
} // end of fn refresh_status_err_repo_update
