use std::boxed::Box;
use std::collections::hash_map::RandomState;
use std::collections::HashSet;
use std::sync::Arc;

use chrono::{DateTime, Duration, Local, SubsecRound, Utc};
use rust_decimal::Decimal;

use ecommerce_common::error::AppErrorCode;
use payment::adapter::repository::{AbstractChargeRepo, AppRepoErrorDetail};
use payment::model::{
    BuyerPayInState, Charge3partyModel, ChargeBuyerMetaModel, ChargeBuyerModel,
    ChargeLineBuyerModel, ChargeRefundMap, RefundReqResolutionModel,
    StripeCheckoutPaymentStatusModel, StripeSessionStatusModel,
};

use super::super::{ut_setup_order_bill, ut_setup_orderline_set};
use super::order_replica::ut_setup_bulk_add_charges;
use super::{ut_setup_currency_snapshot, ut_setup_db_charge_repo, ut_verify_currency_snapshot};
use crate::model::refund::ut_setup_refund_cmplt_dto;
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
        ((3034, 602, 0), ((9028,2), (36112,2), 4), ((0,0), (0,0), 0), 0),
        ((8299, 351, 0), ((551,1), (1102,1), 2), ((0,0), (0,0), 0), 0),
        ((2615, 90040, 0), ((82,0), (246,0), 3), ((0,0), (0,0), 0), 0),
        ((8299, 479, 0), ((839,1), (5873,1), 7), ((0,0), (0,0), 0), 0),
        ((8299, 479, 1), ((845,1), (845,0), 10), ((0,0), (0,0), 0), 0),
        ((2615, 961, 0), ((1946,2), (21406,2), 11), ((0,0), (0,0), 0), 0),
        ((2615, 961, 1), ((1948,2), (3896,2), 2), ((0,0), (0,0), 0), 0),
        ((8299, 961, 0), ((118,0), (236,0), 2), ((0,0), (0,0), 0), 0),
    ]; // TODO, verify read / write refund fields
    let currency_map = ut_setup_currency_snapshot(vec![owner, 8299, 3034, 2615]);
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

fn ut_verify_all_lines(loaded_lines: Vec<ChargeLineBuyerModel>) {
    assert_eq!(loaded_lines.len(), 8);
    loaded_lines
        .into_iter()
        .map(|v| {
            let expect = match v.id() {
                (3034, 602, 0) => ((9028i128, 2u32), (36112i128, 2u32), 4u32),
                (8299, 351, 0) => ((5510, 2), (11020, 2), 2),
                (2615, 90040, 0) => ((8200, 2), (24600, 2), 3),
                (8299, 479, 0) => ((8390, 2), (58730, 2), 7),
                (8299, 479, 1) => ((8450, 2), (84500, 2), 10),
                (2615, 961, 0) => ((1946, 2), (21406, 2), 11),
                (2615, 961, 1) => ((1948, 2), (3896, 2), 2),
                (8299, 961, 0) => ((11800, 2), (23600, 2), 2),
                _others => ((0, 0), (0, 0), 0),
            };
            let (_, _, amt_orig, amt_rfd, num_rej) = v.into_parts();
            assert_eq!(amt_orig.unit.mantissa(), expect.0 .0);
            assert_eq!(amt_orig.unit.scale(), expect.0 .1);
            assert_eq!(amt_orig.total.mantissa(), expect.1 .0);
            assert_eq!(amt_orig.total.scale(), expect.1 .1);
            assert_eq!(amt_orig.qty, expect.2);
            assert_eq!(amt_rfd.qty, 0u32); // TODO, verify amount refunded
            assert_eq!(amt_rfd.unit, amt_orig.unit);
            assert_eq!(amt_rfd.total, Decimal::ZERO);
            assert_eq!(num_rej, 0u32);
        })
        .count();
} // end of fn ut_verify_all_lines

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
    ut_verify_all_lines(result.unwrap());
} // end of fn buyer_create_stripe_charge_ok

#[rustfmt::skip]
#[actix_web::test]
async fn fetch_charge_by_merchant_ok() {
    let (mock_buyer_id, mock_merchant_id) = (126, 8299u32);
    let mock_create_time = Local::now().to_utc() - Duration::minutes(140);
    let accepted_time_duration = Duration::seconds(105);
    let shr_state = ut_setup_sharestate();
    let repo = ut_setup_db_charge_repo(shr_state).await;
     
    let new_charge_m = _ut_setup_buyer_charge(mock_buyer_id, mock_create_time, accepted_time_duration);
    { // create order-replica to ensure currency snapshot data is ready
        let mock_olines_data = new_charge_m.lines
            .iter().map(|cl| {
                let amt_orig = cl.amount_orig();
                let (store_id, product_id, attr_seq) = cl.id();
                (store_id, product_id, attr_seq, amt_orig.unit, amt_orig.total,
                amt_orig.qty, Duration::minutes(219))
            }).collect::<Vec<_>>();
        let expect_ol_set = ut_setup_orderline_set(
            mock_buyer_id, new_charge_m.meta.oid().as_str(), 0,
            mock_create_time, new_charge_m.currency_snapshot.clone(),
            mock_olines_data,
        );
        let billing = ut_setup_order_bill();
        let result = repo.create_order(&expect_ol_set, &billing).await;
        assert!(result.is_ok());
    }
    let result = repo.create_charge(new_charge_m).await;
    assert!(result.is_ok());
    
    let result = repo.fetch_charge_by_merchant(
        mock_buyer_id, mock_create_time, mock_merchant_id
    ).await;
    assert!(result.is_ok());
    let maybe_charge_m = result.unwrap();
    assert!(maybe_charge_m.is_some());
    let saved_charge_m = maybe_charge_m.unwrap();
    {
        let buyer_currency = saved_charge_m.currency_snapshot.get(&mock_buyer_id).unwrap();
        ut_verify_currency_snapshot(buyer_currency);
        let merchant_currency = saved_charge_m.currency_snapshot.get(&mock_merchant_id).unwrap();
        ut_verify_currency_snapshot(merchant_currency);
    }
    assert_eq!(saved_charge_m.lines.len(), 4);
    saved_charge_m.lines.into_iter()
        .map(|v| {
            let (store_id, product_id, attr_seq) = v.id();
            assert_eq!(store_id, mock_merchant_id);
            let expect = match (product_id, attr_seq) {
                (351, 0) => ((551i64, 1u32), (1102i64, 1u32), 2u32),
                (479, 0) => ((839, 1), (5873, 1), 7),
                (479, 1) => ((845, 1), (845, 0), 10),
                (961, 0) => ((118, 0), (236, 0), 2),
                _others => ((0, 0), (0, 0), 0),
            };
            let (_, _, amt_orig, amt_rfd, num_rej) = v.into_parts();
            assert_eq!(amt_orig.unit, Decimal::new(expect.0 .0, expect.0 .1));
            assert_eq!(amt_orig.total, Decimal::new(expect.1 .0, expect.1 .1));
            assert_eq!(amt_orig.qty, expect.2);
            assert_eq!(amt_rfd.qty, 0u32); // TODO, verify amount refunded
            assert_eq!(amt_rfd.unit, amt_orig.unit);
            assert_eq!(amt_rfd.total, Decimal::ZERO);
            assert_eq!(num_rej, 0u32);
        })
        .count();
} // end of fn fetch_charge_by_merchant_ok

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
async fn fetch_meta_nonexist() {
    let (mock_buyer_id, mock_merchant_id) = (9999, 134);
    let mock_create_time = Local::now().to_utc() - Duration::days(3650);
    let shr_state = ut_setup_sharestate();
    let repo = ut_setup_db_charge_repo(shr_state).await;
    let result = repo
        .fetch_charge_meta(mock_buyer_id, mock_create_time)
        .await;
    assert!(result.is_ok());
    let optional_meta = result.unwrap();
    assert!(optional_meta.is_none());
    let result = repo
        .fetch_charge_by_merchant(mock_buyer_id, mock_create_time, mock_merchant_id)
        .await;
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

// TODO, simplify setup flow, currently it is too complicated
#[rustfmt::skip]
#[actix_web::test]
async fn update_refund_in_chargeline_ok() {
    let mock_buyer_id = 127u32;
    let mock_merchant_id = 219u32;
    let mock_oid = "70019835b3a0";
    let time_now = Local::now().to_utc();
    let shr_state = ut_setup_sharestate();
    let repo_ch  = ut_setup_db_charge_repo(shr_state).await;
    
    let mock_orig_olines = vec![
        (mock_merchant_id, 1294u64, 0, Decimal::new(191, 1), Decimal::new(1910, 1), 10u32, Duration::days(1)),
        (mock_merchant_id, 1295, 0, Decimal::new(214, 1), Decimal::new(1498, 1), 7, Duration::days(2)),
        (mock_merchant_id, 1299, 0, Decimal::new(798, 1), Decimal::new(7980, 1), 10, Duration::days(3)),
        (mock_merchant_id, 2945, 0, Decimal::new(505, 1), Decimal::new(5050, 1), 10, Duration::days(4)),
        (mock_merchant_id, 2945, 1, Decimal::new(511, 1), Decimal::new(6132, 1), 12, Duration::days(5)),
    ];
    let mock_currency_map = ut_setup_currency_snapshot(
        vec![mock_merchant_id, mock_buyer_id]
    );
    let mock_order_replica = ut_setup_orderline_set(
        mock_buyer_id,  mock_oid, 0, time_now - Duration::hours(127),
        mock_currency_map, mock_orig_olines,
    );
    let billing = ut_setup_order_bill();
    let result = repo_ch.create_order(&mock_order_replica, &billing).await;
    assert!(result.is_ok());

    let charge_ctime = [
        time_now - Duration::hours(85),
        time_now - Duration::hours(70),
    ];
    let mock_clines = vec![
        (charge_ctime[0], true, vec![
             (mock_merchant_id, 1294u64, 0u16, (191i64, 1u32), (1910i64, 1u32), 10u32),
             (mock_merchant_id, 1295, 0, (214, 1), (1070, 1), 5),
             (mock_merchant_id, 1299, 0, (798, 1), (1596, 1), 2),
             (mock_merchant_id, 2945, 1, (511, 1), (4599, 1), 9),
        ]),
        (charge_ctime[1], true, vec![
             (mock_merchant_id, 1295, 0, (214, 1), (428, 1), 2),
             (mock_merchant_id, 1299, 0, (798, 1), (6384, 1), 8),
             (mock_merchant_id, 2945, 0, (505, 1), (5050, 1), 10),
        ]),
    ];
    ut_setup_bulk_add_charges(repo_ch.clone(), mock_buyer_id, mock_oid, mock_clines).await;

    macro_rules! setup_resolve_model {
        ($ctime: expr, $lines: expr) => {{
            let result = repo_ch.fetch_charge_by_merchant(mock_buyer_id, $ctime, mock_merchant_id).await;
            assert!(result.is_ok());
            let charge_m = result.unwrap().unwrap();
            let t = $ctime + Duration::minutes(29);
            let req = ut_setup_refund_cmplt_dto(t, $lines);
            RefundReqResolutionModel::try_from((mock_merchant_id, &charge_m, &req)).unwrap()
        }};
    }
    let rslv_m0 = setup_resolve_model!(
        charge_ctime[0], vec![
            ((1294, 0), 10, 573, 3, 2, 1),
            ((1295, 0), 12, 214, 1, 0, 1),
            ((1299, 0), 14, 0,   0, 1, 0),
            ((2945, 1), 90, 511, 1, 2, 1),
            ((2945, 1), 80, 511, 1, 1, 2),
        ]);
    let rslv_m1 = setup_resolve_model!(
        charge_ctime[1], vec![
            ((1295, 0), 16, 428, 2, 0, 0),
            ((1299, 0), 18, 2394, 3, 2, 0),
            ((2945, 0), 20, 3030, 6, 1, 2),
        ]);

    let rslv_ms = vec![rslv_m0, rslv_m1];
    let cl_map = ChargeRefundMap::build(&rslv_ms);
    let result = repo_ch.update_lines_refund(cl_map).await;
    assert!(result.is_ok());
    
    macro_rules! verify_updated_line {
        ($ctime: expr, $num_clines: literal, $data_selector: expr) => {
            let result = repo_ch.fetch_charge_by_merchant(mock_buyer_id, $ctime, mock_merchant_id).await;
            let charge_m = result.unwrap().unwrap();
            assert_eq!(charge_m.lines.len(), $num_clines);
            charge_m.lines.into_iter()
                .map(|line| {
                    let amt_rfd = line.amount_refunded();
                    let actual = (amt_rfd.total, amt_rfd.qty, line.num_rejected());
                    let (store_id, product_id, attr_seq) = line.id();
                    assert_eq!(store_id, mock_merchant_id);
                    let expect = $data_selector(product_id, attr_seq);
                    assert_eq!(actual, expect);
                }).count();
        };
    }

    let fn1 = |product_id, attr_seq|
        match (product_id, attr_seq) {
            (1294, 0) => (Decimal::new(573,1), 3u32, 3u32),
            (1295, 0) => (Decimal::new(214,1), 1, 1),
            (1299, 0) => (Decimal::ZERO, 0, 1),
            (2945, 1) => (Decimal::new(1022, 1), 2, 6),
            _others => (Decimal::MAX, 9999, 9999),
        };
    let fn2 = |product_id, attr_seq|
        match (product_id, attr_seq) {
            (1295u64, 0u16) => (Decimal::new(428,1), 2u32, 0u32),
            (1299, 0) => (Decimal::new(2394,1), 3, 2),
            (2945, 0) => (Decimal::new(3030,1), 6, 3),
            _others => (Decimal::MAX, 9999, 9999),
        };
    verify_updated_line!(charge_ctime[0], 4, fn1);
    verify_updated_line!(charge_ctime[1], 3, fn2);
} // update_refund_in_chargeline_ok

#[rustfmt::skip]
#[actix_web::test]
async fn fetch_all_charge_ids_ok() {
    let (mock_buyer_id, mock_merchant_id) = (127u32, 219u32);
    let mock_oid = "11501a410c";
    let time_base = Local::now().to_utc();
    let shr_state = ut_setup_sharestate();
    let repo_ch  = ut_setup_db_charge_repo(shr_state).await;
    
    let mock_orig_olines = vec![
        (mock_merchant_id, 117u64, 0u16, Decimal::new(191, 1),
         Decimal::new(19100, 1), 100u32, Duration::days(29)),
    ];
    let mock_currency_map = ut_setup_currency_snapshot(
        vec![mock_merchant_id, mock_buyer_id]
    );
    let mock_order_replica = ut_setup_orderline_set(
        mock_buyer_id,  mock_oid, 0, time_base - Duration::hours(127),
        mock_currency_map, mock_orig_olines,
    );
    let billing = ut_setup_order_bill();
    let result = repo_ch.create_order(&mock_order_replica, &billing).await;
    assert!(result.is_ok());

    let orig_charge_ctime = [99i64, 91, 87, 82]
        .into_iter()
        .map(|td| time_base.trunc_subsecs(0) - Duration::hours(td))
        .collect::<Vec<_>>();
    let mock_clines = orig_charge_ctime.iter()
        .map(|ctime| {
            (*ctime, true, vec![
                 (mock_merchant_id, 117u64, 0u16, (191i64, 1u32), (1910i64, 1u32), 10u32),
            ])
        })
        .collect::<Vec<_>>() ;
    ut_setup_bulk_add_charges(repo_ch.clone(), mock_buyer_id, mock_oid, mock_clines).await;
    let result = repo_ch.fetch_charge_ids(mock_oid).await;
    assert!(result.is_ok());
    let (actual_buyer_id, actual_ctime) = result.unwrap().unwrap();
    assert_eq!(actual_buyer_id , mock_buyer_id);
    assert_eq!(actual_ctime.len(), 4);

    let actual_ctimes: HashSet<DateTime<Utc>,RandomState> = HashSet::from_iter(actual_ctime.into_iter());
    let expect_ctimes = HashSet::from_iter(orig_charge_ctime.into_iter());
    let diff = expect_ctimes.difference(&actual_ctimes).collect::<Vec<_>>();
    assert!(diff.is_empty());
} // end of fn fetch_all_charge_ids_ok
