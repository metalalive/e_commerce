use std::env;
use std::fs::File;
use std::sync::Arc;

use chrono::{Duration, Local};
use serde_json::Value as JsnVal;

use ecommerce_common::constant::env_vars::SERVICE_BASEPATH;
use ecommerce_common::error::AppErrorCode;

use payment::adapter::processor::{
    AppProcessorError, AppProcessorErrorReason, AppProcessorFnLabel, AppProcessorMerchantResult,
};
use payment::adapter::repository::{AppRepoError, AppRepoErrorDetail, AppRepoErrorFnLabel};
use payment::adapter::rpc::{AbstractRpcContext, AppRpcReply};
use payment::api::web::dto::{StoreOnboardAcceptedRespDto, StoreOnboardReqDto};
use payment::usecase::{OnboardStoreUcError, OnboardStoreUcOk, OnboardStoreUseCase};

use super::{
    MockMerchantRepo, MockPaymentProcessor, MockRpcClient, MockRpcContext, MockRpcPublishEvent,
};
use crate::auth::ut_setup_auth_claim;
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

#[actix_web::test]
async fn ok_new_shop() {
    let auth_claim = ut_setup_auth_claim(85);
    let processors = {
        let pay3pty_result = Ok(AppProcessorMerchantResult::Stripe);
        let m3pty = MockPaymentProcessor::build(None, None, Some(pay3pty_result));
        Arc::new(m3pty)
    };
    let repo = MockMerchantRepo::build(Some(Ok(())));
    let rpc_ctx = {
        let msg = ut_rpc_storeprof_replica("store_profile_replica_dto_1.json");
        ut_setup_rpc_ctx(msg)
    };
    let mock_store_id = 1008;
    let req_body = StoreOnboardReqDto::Stripe;
    let uc = OnboardStoreUseCase {
        auth_claim,
        processors,
        repo,
        rpc_ctx,
    };
    let result = uc.execute(mock_store_id, req_body).await;
    assert!(result.is_ok());
    if let Ok(v) = result {
        match v {
            OnboardStoreUcOk::Accepted(c) => {
                matches!(c, StoreOnboardAcceptedRespDto::Stripe);
                // TODO, verify more detail about 3rd party info
            }
        }
    }
} // end of fn ok_new_shop

#[actix_web::test]
async fn err_rpc_corrupted_reply() {
    let auth_claim = ut_setup_auth_claim(79);
    let processors = {
        let pay3pty_result = Ok(AppProcessorMerchantResult::Stripe);
        let m3pty = MockPaymentProcessor::build(None, None, Some(pay3pty_result));
        Arc::new(m3pty)
    };
    let repo = MockMerchantRepo::build(Some(Ok(())));
    let rpc_ctx = ut_setup_rpc_ctx(Vec::new());
    let mock_store_id = 1009;
    let req_body = StoreOnboardReqDto::Stripe;
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
} // end of fn err_rpc_corrupted_reply

#[actix_web::test]
async fn err_3party_failure() {
    let auth_claim = ut_setup_auth_claim(85);
    let processors = {
        let pay3pty_result = Err(AppProcessorError {
            reason: AppProcessorErrorReason::InvalidMethod("unit-test".to_string()),
            fn_label: AppProcessorFnLabel::OnboardMerchant,
        });
        let m3pty = MockPaymentProcessor::build(None, None, Some(pay3pty_result));
        Arc::new(m3pty)
    };
    let repo = MockMerchantRepo::build(Some(Ok(())));
    let rpc_ctx = {
        let msg = ut_rpc_storeprof_replica("store_profile_replica_dto_1.json");
        ut_setup_rpc_ctx(msg)
    };
    let mock_store_id = 1010;
    let req_body = StoreOnboardReqDto::Stripe;
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
} // end of fn err_3party_failure

#[actix_web::test]
async fn err_repo_create_op() {
    let auth_claim = ut_setup_auth_claim(85);
    let processors = {
        let pay3pty_result = Ok(AppProcessorMerchantResult::Stripe);
        let m3pty = MockPaymentProcessor::build(None, None, Some(pay3pty_result));
        Arc::new(m3pty)
    };
    let repo = {
        let err = Err(AppRepoError {
            fn_label: AppRepoErrorFnLabel::CreateMerchant,
            code: AppErrorCode::RemoteDbServerFailure,
            detail: AppRepoErrorDetail::DatabaseExec("unit-test".to_string()),
        });
        MockMerchantRepo::build(Some(err))
    };
    let rpc_ctx = {
        let msg = ut_rpc_storeprof_replica("store_profile_replica_dto_1.json");
        ut_setup_rpc_ctx(msg)
    };
    let mock_store_id = 1012;
    let req_body = StoreOnboardReqDto::Stripe;
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
} // end of fn err_repo_create_op
