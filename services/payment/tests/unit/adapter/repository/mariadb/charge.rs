use std::boxed::Box;
use std::sync::Arc;

use chrono::{DateTime, Duration, Local, SubsecRound, Utc};

use ecommerce_common::constant::ProductType;
use ecommerce_common::error::AppErrorCode;
use ecommerce_common::model::BaseProductIdentity;
use payment::adapter::repository::{AbstractChargeRepo, AppRepoErrorDetail};
use payment::model::{
    BuyerPayInState, Charge3partyModel, ChargeBuyerMetaModel, ChargeBuyerModel,
    ChargeLineBuyerModel, StripeCheckoutPaymentStatusModel, StripeSessionStatusModel,
};

use super::{ut_setup_currency_snapshot, ut_setup_db_charge_repo};
use crate::model::{ut_default_charge_method_stripe, ut_setup_buyer_charge};
use crate::ut_setup_sharestate;

#[rustfmt::skip]
fn _ut_setup_buyer_charge(
    owner: u32,
    create_time: DateTime<Utc>,
    accepted_time_duration: Duration,
) -> ChargeBuyerModel {
    let oid = "dee50de6".to_string();
    let state = BuyerPayInState::ProcessorAccepted(create_time + accepted_time_duration);
    let mut mthd_3pty = ut_default_charge_method_stripe(&create_time);
    if let Charge3partyModel::Stripe(s) = &mut mthd_3pty {
        s.payment_state = StripeCheckoutPaymentStatusModel::unpaid;
        s.session_state = StripeSessionStatusModel::open;
    }
    let data_lines = vec![
        (3034, ProductType::Package, 602, (9028,2), (36112,2), 4),
        (8299, ProductType::Item, 351, (551,1), (1102,1), 2),
        (2615, ProductType::Item, 90040, (82,0), (246,0), 3),
        (8299, ProductType::Item, 479, (839,1), (5873,1), 7),
        (2615, ProductType::Package, 961, (1946,2), (21406,2), 11),
    ];
    let currency_map = ut_setup_currency_snapshot(vec![126, 8299, 3034]);
    ut_setup_buyer_charge(
        owner, create_time, oid, state,
        mthd_3pty, data_lines, currency_map,
    )
}

async fn ut_fetch_existing_charge_meta(
    repo: Arc<Box<dyn AbstractChargeRepo>>,
    owner: u32,
    create_time: DateTime<Utc>,
) -> ChargeBuyerMetaModel {
    let result = repo.fetch_charge_meta(owner, create_time).await;
    assert!(result.is_ok());
    let optional_meta = result.unwrap();
    assert!(optional_meta.is_some());
    let loaded_meta = optional_meta.unwrap();
    assert_eq!(loaded_meta.owner(), owner);
    let expect_create_time = create_time.trunc_subsecs(0);
    assert_eq!(loaded_meta.create_time(), &expect_create_time);
    assert_eq!(loaded_meta.oid().as_str(), "dee50de6");
    loaded_meta
}

fn ut_verify_charge_lines(loaded_lines: Vec<ChargeLineBuyerModel>) {
    assert_eq!(loaded_lines.len(), 5);
    loaded_lines
        .into_iter()
        .map(|v| {
            let ChargeLineBuyerModel { amount, pid } = v;
            let BaseProductIdentity {
                store_id,
                product_type,
                product_id,
            } = pid;
            let expect = match (store_id, product_type, product_id) {
                (3034, ProductType::Package, 602) => ((9028i128, 2u32), (36112i128, 2u32), 4u32),
                (8299, ProductType::Item, 351) => ((5510, 2), (11020, 2), 2),
                (2615, ProductType::Item, 90040) => ((8200, 2), (24600, 2), 3),
                (8299, ProductType::Item, 479) => ((8390, 2), (58730, 2), 7),
                (2615, ProductType::Package, 961) => ((1946, 2), (21406, 2), 11),
                _others => ((0, 0), (0, 0), 0),
            };
            assert_eq!(amount.unit.mantissa(), expect.0 .0);
            assert_eq!(amount.unit.scale(), expect.0 .1);
            assert_eq!(amount.total.mantissa(), expect.1 .0);
            assert_eq!(amount.total.scale(), expect.1 .1);
            assert_eq!(amount.qty, expect.2);
        })
        .count();
}

#[actix_web::test]
async fn buyer_create_stripe_charge_ok() {
    let mock_owner = 126;
    let mock_create_time = Local::now().to_utc() - Duration::minutes(4);
    let accepted_time_duration = Duration::seconds(95);
    let shr_state = ut_setup_sharestate();
    let repo = ut_setup_db_charge_repo(shr_state).await;
    let cline_set = _ut_setup_buyer_charge(mock_owner, mock_create_time, accepted_time_duration);
    let result = repo.create_charge(cline_set).await;
    assert!(result.is_ok());
    // --- fetch charge metadata ---
    let loaded_meta =
        ut_fetch_existing_charge_meta(repo.clone(), mock_owner, mock_create_time).await;
    if let BuyerPayInState::ProcessorAccepted(t) = loaded_meta.progress() {
        let expect = mock_create_time.trunc_subsecs(3) + accepted_time_duration;
        assert_eq!(t, &expect);
    } else {
        assert!(false);
    }
    if let Charge3partyModel::Stripe(s) = loaded_meta.method_3party() {
        assert_eq!(s.checkout_session_id.as_str(), "mock-session-id");
        assert_eq!(s.payment_intent_id.as_str(), "mock-payment-intent-id");
        let cond = matches!(s.session_state, StripeSessionStatusModel::open);
        assert!(cond);
        let cond = matches!(s.payment_state, StripeCheckoutPaymentStatusModel::unpaid);
        assert!(cond);
    } else {
        assert!(false);
    }
    // --- update charge metadata and save ---
    let complete_t_duration = Duration::seconds(167);
    let mut updating_meta = loaded_meta;
    {
        let t = mock_create_time + complete_t_duration;
        let mut m3pty = ut_default_charge_method_stripe(&t);
        if let Charge3partyModel::Stripe(s) = &mut m3pty {
            s.payment_state = StripeCheckoutPaymentStatusModel::paid;
            s.session_state = StripeSessionStatusModel::complete;
        }
        updating_meta.update_3party(m3pty);
        let value = BuyerPayInState::ProcessorCompleted(t);
        updating_meta.update_progress(&value);
    }
    let result = repo.update_charge_progress(updating_meta).await;
    assert!(result.is_ok());
    // --- fetch charge metadata again ---
    let loaded_meta =
        ut_fetch_existing_charge_meta(repo.clone(), mock_owner, mock_create_time).await;
    if let BuyerPayInState::ProcessorCompleted(t) = loaded_meta.progress() {
        let expect = mock_create_time.trunc_subsecs(3) + complete_t_duration;
        assert_eq!(t, &expect);
    } else {
        assert!(false);
    }
    if let Charge3partyModel::Stripe(s) = &loaded_meta.method_3party() {
        let cond = matches!(s.session_state, StripeSessionStatusModel::complete);
        assert!(cond);
        let cond = matches!(s.payment_state, StripeCheckoutPaymentStatusModel::paid);
        assert!(cond);
    } else {
        assert!(false);
    }
    // ---- fetch charge lines ----
    let result = repo
        .fetch_all_charge_lines(mock_owner, mock_create_time)
        .await;
    assert!(result.is_ok());
    ut_verify_charge_lines(result.unwrap());
} // end of fn buyer_create_stripe_charge_ok

#[actix_web::test]
async fn buyer_create_charge_invalid_state() {
    let mock_owner = 126;
    let mock_create_time = Local::now().to_utc() - Duration::minutes(4);
    let accepted_time_duration = Duration::seconds(95);
    let shr_state = ut_setup_sharestate();
    let repo = ut_setup_db_charge_repo(shr_state).await;
    let mut cline_set =
        _ut_setup_buyer_charge(mock_owner, mock_create_time, accepted_time_duration);
    cline_set
        .meta
        .update_progress(&BuyerPayInState::Initialized);
    let result = repo.create_charge(cline_set).await;
    assert!(result.is_err());
    if let Err(e) = result {
        assert_eq!(e.code, AppErrorCode::InvalidInput);
        if let AppRepoErrorDetail::ChargeStatus(s) = e.detail {
            let cond = matches!(s, BuyerPayInState::Initialized);
            assert!(cond);
        } else {
            assert!(false);
        }
    }
}

#[actix_web::test]
async fn buyer_create_charge_unknown_3party() {
    let mock_owner = 126;
    let mock_create_time = Local::now().to_utc();
    let accepted_time_duration = Duration::seconds(107);
    let shr_state = ut_setup_sharestate();
    let repo = ut_setup_db_charge_repo(shr_state).await;
    let mut charge_m = _ut_setup_buyer_charge(mock_owner, mock_create_time, accepted_time_duration);
    charge_m.meta.update_3party(Charge3partyModel::Unknown);
    let result = repo.create_charge(charge_m).await;
    assert!(result.is_err());
    if let Err(e) = result {
        assert_eq!(e.code, AppErrorCode::InvalidInput);
        let cond = matches!(e.detail, AppRepoErrorDetail::PayMethodUnsupport(_msg));
        assert!(cond);
    }
}

#[actix_web::test]
async fn buyer_fetch_charge_meta_nonexist() {
    let mock_owner = 9999;
    let mock_create_time = Local::now().to_utc() - Duration::days(3650);
    let shr_state = ut_setup_sharestate();
    let repo = ut_setup_db_charge_repo(shr_state).await;
    let result = repo.fetch_charge_meta(mock_owner, mock_create_time).await;
    assert!(result.is_ok());
    let optional_meta = result.unwrap();
    assert!(optional_meta.is_none());
}

#[actix_web::test]
async fn buyer_update_charge_meta_invalid_state() {
    let mock_owner = 126;
    let mock_create_time = Local::now().to_utc() - Duration::minutes(5);
    let accepted_time_duration = Duration::seconds(176);
    let shr_state = ut_setup_sharestate();
    let repo = ut_setup_db_charge_repo(shr_state).await;
    let mut charge_m = _ut_setup_buyer_charge(mock_owner, mock_create_time, accepted_time_duration);
    charge_m.meta.update_progress(&BuyerPayInState::Initialized);
    let result = repo.update_charge_progress(charge_m.meta).await;
    assert!(result.is_err());
    if let Err(e) = result {
        assert_eq!(e.code, AppErrorCode::InvalidInput);
        if let AppRepoErrorDetail::ChargeStatus(s) = e.detail {
            let cond = matches!(s, BuyerPayInState::Initialized);
            assert!(cond);
        } else {
            assert!(false);
        }
    }
}
