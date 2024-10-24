use std::boxed::Box;
use std::result::Result;
use std::sync::Arc;

use chrono::{DateTime, Duration, Utc};
use rust_decimal::Decimal;

use ecommerce_common::error::AppErrorCode;
use payment::adapter::processor::{
    AbstractPaymentProcessor, AppProcessorError, AppProcessorErrorReason, AppProcessorFnLabel,
    AppProcessorPayoutResult,
};
use payment::adapter::repository::{
    AbstractChargeRepo, AbstractMerchantRepo, AppRepoError, AppRepoErrorDetail, AppRepoErrorFnLabel,
};
use payment::api::web::dto::CapturePay3partyRespDto;
use payment::model::{
    BuyerPayInState, ChargeBuyerModel, Merchant3partyModel, MerchantProfileModel, PayoutModel,
    PayoutModelError,
};
use payment::usecase::{ChargeCaptureUcError, ChargeCaptureUseCase};

use crate::auth::ut_setup_auth_claim;
use crate::dto::ut_setup_capture_pay_resp_dto;
use crate::model::payout::{
    ut_common_create_first_payout, ut_setup_buyer_charge_inner, ut_setup_merchant_3party_stripe,
    ut_setup_merchant_profile,
};

use super::{MockChargeRepo, MockMerchantRepo, MockPaymentProcessor};

fn ut_setup_processor(
    res: Option<Result<AppProcessorPayoutResult, AppProcessorError>>,
) -> Box<dyn AbstractPaymentProcessor> {
    MockPaymentProcessor::build(None, None, None, res, None)
}

fn ut_setup_repo_merchant(
    res: Option<(MerchantProfileModel, Merchant3partyModel)>,
) -> Box<dyn AbstractMerchantRepo> {
    MockMerchantRepo::build(None, res, None, None)
}

#[rustfmt::skip]
fn ut_setup_repo_charge(
    charge_by_merchant: Option<ChargeBuyerModel>,
    rd_payout: Option<PayoutModel>,
    create_payout_res: Option<Result<(), AppRepoError>>,
) -> Box<dyn AbstractChargeRepo> {
    let maybe_charge_ms = charge_by_merchant.map(|item| vec![item]) ;
    MockChargeRepo::build(
        None, None, None,
        None, None, None,
        maybe_charge_ms, rd_payout, create_payout_res,
        None, None,
    )
}

#[rustfmt::skip]
fn ut_common_mock_data() -> (u32, DateTime<Utc>, String, u32, u32)
{
    let buyer_usr_id = 8010095;
    let charge_time = DateTime::parse_from_rfc3339("2012-04-24T23:01:30+00:00")
        .unwrap().to_utc();
    let charge_id = "007a396f1f7131705e".to_string();
    let staff_usr_id = 1234u32;
    let merchant_id = 1009u32;
    (buyer_usr_id, charge_time, charge_id, staff_usr_id, merchant_id)
}

#[rustfmt::skip]
#[actix_web::test]
async fn done_ok() {
    let (mock_buyer_id, charge_create_time, mock_charge_id, mock_staff_id, mock_store_id) = ut_common_mock_data();
    let repo_c = {
        let payin_state = BuyerPayInState::OrderAppSynced(charge_create_time + Duration::minutes(5));
        let charge_m = ut_setup_buyer_charge_inner(mock_buyer_id, charge_create_time, payin_state);
        let create_payout_res = Some(Ok(()));
        ut_setup_repo_charge(Some(charge_m), None, create_payout_res)
    };
    let repo_m = {
        let mprof = ut_setup_merchant_profile(mock_store_id, mock_staff_id);
        let m3pt = ut_setup_merchant_3party_stripe();
        ut_setup_repo_merchant(Some((mprof, m3pt)))
    };
    let processors = {
        let m = ut_common_create_first_payout(
            mock_buyer_id, mock_store_id, mock_staff_id, charge_create_time,
        ).unwrap();
        let d = ut_setup_capture_pay_resp_dto(mock_store_id);
        let bp = ut_setup_processor(Some(Ok(AppProcessorPayoutResult::new(d, m))));
        Arc::new(bp)
    };
    let auth_claim = ut_setup_auth_claim(mock_staff_id, 85);
    let uc = ChargeCaptureUseCase { auth_claim, processors, repo_c, repo_m };
    let result = uc.execute(mock_charge_id, mock_store_id).await;
    assert!(result.is_ok());
    if let Ok(v) = result {
        assert_eq!(v.store_id, mock_store_id);
        let cond = matches!(v.processor, CapturePay3partyRespDto::Stripe { amount:_, currency:_ });
        assert!(cond);
    }
} // end of fn done_ok

#[actix_web::test]
async fn err_missing_charge() {
    let (_, _, mock_charge_id, mock_staff_id, mock_store_id) = ut_common_mock_data();
    let repo_c = ut_setup_repo_charge(None, None, None);
    let repo_m = ut_setup_repo_merchant(None);
    let processors = Arc::new(ut_setup_processor(None));
    let auth_claim = ut_setup_auth_claim(mock_staff_id, 85);
    let uc = ChargeCaptureUseCase {
        auth_claim,
        processors,
        repo_c,
        repo_m,
    };
    let result = uc.execute(mock_charge_id, mock_store_id).await;
    assert!(result.is_err());
    if let Err(e) = result {
        let cond = matches!(e, ChargeCaptureUcError::MissingCharge);
        assert!(cond);
    }
}

#[rustfmt::skip]
#[actix_web::test]
async fn err_charge_payin_ongoing() {
    let (mock_buyer_id, charge_create_time, mock_charge_id, mock_staff_id, mock_store_id) = ut_common_mock_data();
    let repo_c = {
        let payin_state = BuyerPayInState::ProcessorAccepted(charge_create_time);
        let charge_m = ut_setup_buyer_charge_inner(mock_buyer_id, charge_create_time, payin_state);
        ut_setup_repo_charge(Some(charge_m), None, None)
    };
    let repo_m = ut_setup_repo_merchant(None);
    let processors = Arc::new(ut_setup_processor(None));
    let auth_claim = ut_setup_auth_claim(mock_staff_id, 85);
    let uc = ChargeCaptureUseCase { auth_claim, processors, repo_c, repo_m };
    let result = uc.execute(mock_charge_id, mock_store_id).await;
    assert!(result.is_err());
    if let Err(ChargeCaptureUcError::PayInNotCompleted(state)) = result {
        let cond = matches!(state, BuyerPayInState::ProcessorAccepted(_));
        assert!(cond);
    } else {
        assert!(false);
    }
}

#[rustfmt::skip]
#[actix_web::test]
async fn err_missing_merchant() {
    let (mock_buyer_id, charge_create_time, mock_charge_id, mock_staff_id, mock_store_id) = ut_common_mock_data();
    let repo_c = {
        let payin_state = BuyerPayInState::OrderAppSynced(charge_create_time);
        let charge_m = ut_setup_buyer_charge_inner(mock_buyer_id, charge_create_time, payin_state);
        ut_setup_repo_charge(Some(charge_m), None, None)
    };
    let repo_m = ut_setup_repo_merchant(None);
    let processors = Arc::new(ut_setup_processor(None));
    let auth_claim = ut_setup_auth_claim(mock_staff_id, 85);
    let uc = ChargeCaptureUseCase { auth_claim, processors, repo_c, repo_m };
    let result = uc.execute(mock_charge_id, mock_store_id).await;
    assert!(result.is_err());
    if let Err(e) = result {
        let cond = matches!(e, ChargeCaptureUcError::MissingMerchant);
        assert!(cond);
    }
}

#[rustfmt::skip]
#[actix_web::test]
async fn err_3party_failure() {
    let (mock_buyer_id, charge_create_time, mock_charge_id, mock_staff_id, mock_store_id) = ut_common_mock_data();
    let repo_c = {
        let payin_state = BuyerPayInState::OrderAppSynced(charge_create_time + Duration::minutes(5));
        let charge_m = ut_setup_buyer_charge_inner(mock_buyer_id, charge_create_time, payin_state);
        ut_setup_repo_charge(Some(charge_m), None, None)
    };
    let repo_m = {
        let mprof = ut_setup_merchant_profile(mock_store_id, mock_staff_id);
        let m3pt = ut_setup_merchant_3party_stripe();
        ut_setup_repo_merchant(Some((mprof, m3pt)))
    };
    let processors = {
        let e = AppProcessorError {
            reason: AppProcessorErrorReason::InvalidMethod("unit-test".to_string()),
            fn_label: AppProcessorFnLabel::PayOut,
        };
        Arc::new(ut_setup_processor(Some(Err(e))))
    };
    let auth_claim = ut_setup_auth_claim(mock_staff_id, 85);
    let uc = ChargeCaptureUseCase { auth_claim, processors, repo_c, repo_m };
    let result = uc.execute(mock_charge_id, mock_store_id).await;
    assert!(result.is_err());
    if let Err(ChargeCaptureUcError::ThirdParty(pe)) = result {
        let cond = matches!(pe.fn_label, AppProcessorFnLabel::PayOut);
        assert!(cond);
        if let AppProcessorErrorReason::InvalidMethod(r) = pe.reason {
            assert_eq!(r.as_str(), "unit-test");
        } else {
            assert!(false);
        }
    } else {
        assert!(false);
    }
} // end of fn err_3party_failure

#[rustfmt::skip]
#[actix_web::test]
async fn err_repo_create_payout() {
    let (mock_buyer_id, charge_create_time, mock_charge_id, mock_staff_id, mock_store_id) = ut_common_mock_data();
    let repo_c = {
        let payin_state = BuyerPayInState::OrderAppSynced(charge_create_time + Duration::minutes(5));
        let charge_m = ut_setup_buyer_charge_inner(mock_buyer_id, charge_create_time, payin_state);
        let create_payout_res = Some(Err(AppRepoError {
            fn_label: AppRepoErrorFnLabel::CreatePayout,
            code: AppErrorCode::DatabaseServerBusy,
            detail: AppRepoErrorDetail::DatabaseExec("unit-test".to_string()),
        }));
        ut_setup_repo_charge(Some(charge_m), None, create_payout_res)
    };
    let repo_m = {
        let mprof = ut_setup_merchant_profile(mock_store_id, mock_staff_id);
        let m3pt = ut_setup_merchant_3party_stripe();
        ut_setup_repo_merchant(Some((mprof, m3pt)))
    };
    let processors = {
        let m = ut_common_create_first_payout(
            mock_buyer_id, mock_store_id, mock_staff_id, charge_create_time,
        ).unwrap();
        let d = ut_setup_capture_pay_resp_dto(mock_store_id);
        let bp = ut_setup_processor(Some(Ok(AppProcessorPayoutResult::new(d, m))));
        Arc::new(bp)
    };
    let auth_claim = ut_setup_auth_claim(mock_staff_id, 85);
    let uc = ChargeCaptureUseCase { auth_claim, processors, repo_c, repo_m };
    let result = uc.execute(mock_charge_id, mock_store_id).await;
    assert!(result.is_err());
    if let Err(ChargeCaptureUcError::RepoOpFailure(re)) = result {
        let cond = matches!(re.fn_label, AppRepoErrorFnLabel::CreatePayout);
        assert!(cond);
        assert_eq!(re.code, AppErrorCode::DatabaseServerBusy);
        if let AppRepoErrorDetail::DatabaseExec(r) = re.detail {
            assert_eq!(r.as_str(), "unit-test");
        } else {
            assert!(false);
        }
    } else {
        assert!(false);
    }
} // end of fn err_repo_create_payout

#[rustfmt::skip]
#[actix_web::test]
async fn err_already_captured() {
    let (mock_buyer_id, charge_create_time, mock_charge_id, mock_staff_id, mock_store_id) = ut_common_mock_data();
    let repo_c = {
        let payin_state = BuyerPayInState::OrderAppSynced(charge_create_time + Duration::minutes(5));
        let charge_m = ut_setup_buyer_charge_inner(mock_buyer_id, charge_create_time, payin_state);
        let mock_existing_payout = ut_common_create_first_payout(
            mock_buyer_id, mock_store_id, mock_staff_id, charge_create_time,
        ).unwrap();
        ut_setup_repo_charge(Some(charge_m), Some(mock_existing_payout), None)
    };
    let repo_m = {
        let mprof = ut_setup_merchant_profile(mock_store_id, mock_staff_id);
        let m3pt = ut_setup_merchant_3party_stripe();
        ut_setup_repo_merchant(Some((mprof, m3pt)))
    };
    let processors = Arc::new(ut_setup_processor(None));
    let auth_claim = ut_setup_auth_claim(mock_staff_id, 85);
    let uc = ChargeCaptureUseCase { auth_claim, processors, repo_c, repo_m };
    let result = uc.execute(mock_charge_id, mock_store_id).await;
    assert!(result.is_err());
    if let Err(ChargeCaptureUcError::CorruptedModel(PayoutModelError::AmountNotEnough(p0, p1))) = result {
        assert!(p0 > Decimal::ZERO);
        assert_eq!(p0, p1);
    } else {
        assert!(false);
    }
} // end of fn err_already_captured
