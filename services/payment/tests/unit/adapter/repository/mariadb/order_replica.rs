use std::boxed::Box;
use std::collections::hash_map::RandomState;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use chrono::{DateTime, Duration, Local, SubsecRound, Utc};
use rust_decimal::Decimal;

use ecommerce_common::api::dto::CurrencyDto;
use ecommerce_common::constant::ProductType;
use ecommerce_common::model::BaseProductIdentity;
use payment::adapter::repository::AbstractChargeRepo;
use payment::model::{
    BuyerPayInState, Charge3partyModel, OrderLineModel, OrderLineModelSet,
    StripeCheckoutPaymentStatusModel, StripeSessionStatusModel,
};

use super::{ut_setup_currency_snapshot, ut_setup_db_charge_repo};
use crate::adapter::repository::{ut_setup_order_bill, ut_setup_orderline_set};
use crate::model::{ut_default_charge_method_stripe, ut_setup_buyer_charge};
use crate::ut_setup_sharestate;

#[rustfmt::skip]
fn ut_verify_fetched_order(
    actual: OrderLineModelSet,
    expect_order_toplvl: (u32, &str, u32, DateTime<Utc>),
    expect_olines: Vec<(u32, ProductType, u64, Decimal, Decimal, u32, Decimal, u32, Duration)>,
) {
    assert!(!expect_olines.is_empty());
    let (expect_usr_id, expect_order_id, expect_num_charges, expect_ctime) = expect_order_toplvl;
    assert_eq!(actual.buyer_id, expect_usr_id);
    assert_eq!(actual.id, expect_order_id);
    assert_eq!(actual.num_charges, expect_num_charges);
    assert_eq!(actual.create_time, expect_ctime.trunc_subsecs(0));
    let mut expect_line_map = {
        let mut hm = HashMap::new();
        expect_olines.into_iter()
            .map(|c| {
                let ctime = expect_ctime.trunc_subsecs(0);
                let (store_id, prod_typ, prod_id, rsv_unit,
                     rsv_total, rsv_qty, paid_total, paid_qty,
                     rsv_until) = c;
                let key = (store_id, prod_typ, prod_id);
                let value = (rsv_unit, rsv_total, rsv_qty, paid_total,
                             paid_qty, ctime + rsv_until);
                let _empty = hm.insert(key, value);
            }).count();
        assert!(hm.len() > 0);
        hm
    };
    actual.lines.into_iter()
        .map(|line| {
            let OrderLineModel {pid, rsv_total, paid_total, reserved_until} = line;
            let BaseProductIdentity {store_id, product_type, product_id} = pid;
            let key = (store_id, product_type, product_id);
            let actual_val = (
                rsv_total.unit, rsv_total.total, rsv_total.qty,
                paid_total.total, paid_total.qty, reserved_until,
            );
            let expect_val = expect_line_map.remove(&key).unwrap();
            assert_eq!(actual_val, expect_val);
        })
        .count();
    assert!(expect_line_map.is_empty());
} // end of fn ut_verify_fetched_order

#[rustfmt::skip]
#[actix_web::test]
async fn create_order_replica_ok() {
    let mock_order_toplvl_data = (123, "9d73ba76d5", 0, Local::now().to_utc());
    let mock_olines_data = vec![
        (2603, ProductType::Item, 180, Decimal::new(34,0), Decimal::new(340,0), 10, Duration::minutes(2)),
        (2603, ProductType::Package, 211, Decimal::new(29,0), Decimal::new(261,0), 9, Duration::minutes(5)),
        (2379, ProductType::Item, 449, Decimal::new(35,0), Decimal::new(420,0), 12, Duration::minutes(11)),
    ];
    let mock_currency_map = ut_setup_currency_snapshot(vec![123, 2603, 2379]);
    let shr_state = ut_setup_sharestate();
    let repo = ut_setup_db_charge_repo(shr_state).await;
    let expect_ol_set = ut_setup_orderline_set(
        mock_order_toplvl_data.0,
        mock_order_toplvl_data.1,
        mock_order_toplvl_data.2,
        mock_order_toplvl_data.3.clone(),
        mock_currency_map,
        mock_olines_data.clone(),
    );
    let billing = ut_setup_order_bill();
    let result = repo.create_order(&expect_ol_set, &billing).await;
    assert!(result.is_ok());
    let result = repo
        .get_unpaid_olines(mock_order_toplvl_data.0, mock_order_toplvl_data.1)
        .await;
    if let Ok(Some(v)) = result {
        let olines4verify = mock_olines_data
             .into_iter()
             .map(|c| (c.0, c.1, c.2, c.3, c.4, c.5, Decimal::new(0,0), 0u32, c.6))
             .collect();
        ut_verify_fetched_order(v, mock_order_toplvl_data, olines4verify);
    } else {
        assert!(false);
    }
} // end of fn create_order_replica_ok

#[rustfmt::skip]
async fn ut_setup_new_orderlines(
    repo: Arc<Box<dyn AbstractChargeRepo>>,
    mock_o_toplvl: (u32, &str, u32, DateTime<Utc>),
    mock_o_lines: Vec<(u32, ProductType, u64, Decimal, Decimal, u32, Duration)>,
) {
    let currency_usr_ids = {
        let iter = mock_o_lines.iter().map(|dl| dl.0);
        let mut hset: HashSet<u32, RandomState> = HashSet::from_iter(iter);
        let _ = hset.insert(mock_o_toplvl.0);
        hset.into_iter().collect::<Vec<_>>()
    };
    let mock_currency_map = ut_setup_currency_snapshot(currency_usr_ids.clone());
    let expect_buyer_currency = mock_currency_map.get(&mock_o_toplvl.0).cloned().unwrap();
    let expect_ol_set = ut_setup_orderline_set(
        mock_o_toplvl.0,
        mock_o_toplvl.1,
        mock_o_toplvl.2,
        mock_o_toplvl.3.clone(),
        mock_currency_map,
        mock_o_lines.clone(),
    );
    let billing = ut_setup_order_bill();
    let result = repo.create_order(&expect_ol_set, &billing).await;
    if let Err(e) = &result {
        println!("[debug] DB error {:?}", e)
    }
    assert!(result.is_ok());
    let result = repo
        .get_unpaid_olines(mock_o_toplvl.0, mock_o_toplvl.1)
        .await;
    if let Ok(Some(v)) = result {
        let actual_buyer_currency = v.currency_snapshot.get(&mock_o_toplvl.0).cloned().unwrap();
        assert_eq!(expect_buyer_currency.label, actual_buyer_currency.label);
        assert_eq!(expect_buyer_currency.rate, actual_buyer_currency.rate);
        assert_ne!(actual_buyer_currency.label, CurrencyDto::Unknown);
        assert_ne!(actual_buyer_currency.rate, Decimal::ZERO);
        let olines4verify = mock_o_lines
             .clone().into_iter()
             .map(|c| (c.0, c.1, c.2, c.3, c.4, c.5, Decimal::new(0,0), 0u32, c.6))
             .collect();
        ut_verify_fetched_order(v, mock_o_toplvl, olines4verify);
    } else {
        assert!(false);
    }
} // end of fn ut_setup_new_orderlines

#[rustfmt::skip]
pub(super) async fn ut_setup_bulk_add_charges(
    repo: Arc<Box<dyn AbstractChargeRepo>>,
    buyer_id: u32,
    order_id: &str,
    d_charges: Vec<(
        DateTime<Utc>, bool,
        Vec<(u32, ProductType, u64, (i64, u32), (i64, u32), u32)>
    )>,
) { // ---- add charge lines ----
    for dl in d_charges {
        let (ctime, is_3pty_done, d_chargelines) = dl;
        let d_chargelines = d_chargelines.into_iter()
            .map(|d| {
                (d.0, d.1, d.2, d.3, d.4, d.5, (0i64, 0u32), (0i64, 0u32), 0u32, 0u32)
            }).collect::<Vec<_>>(); // TODO, verify refund fields
        let mut mthd_3pty = ut_default_charge_method_stripe(&ctime);
        let state = if is_3pty_done {
            if let Charge3partyModel::Stripe(s) = &mut mthd_3pty {
                s.payment_state = StripeCheckoutPaymentStatusModel::paid;
                s.session_state = StripeSessionStatusModel::complete;
            }
            BuyerPayInState::OrderAppSynced(ctime)
        } else {
            if let Charge3partyModel::Stripe(s) = &mut mthd_3pty {
                s.payment_state = StripeCheckoutPaymentStatusModel::unpaid;
                s.session_state = StripeSessionStatusModel::open;
            }
            BuyerPayInState::ProcessorAccepted(ctime)
        };
        let currency_usr_ids = {
            let iter = d_chargelines.iter().map(|dl| dl.0);
            let mut hset: HashSet<u32, RandomState> = HashSet::from_iter(iter);
            let _ = hset.insert(buyer_id);
            hset.into_iter().collect::<Vec<_>>()
        };
        let currency_map = ut_setup_currency_snapshot(currency_usr_ids);
        let charge_m = ut_setup_buyer_charge(
            buyer_id, ctime, order_id.to_string(), state,
            mthd_3pty, d_chargelines, currency_map,
        );
        let result = repo.create_charge(charge_m).await;
        assert!(result.is_ok());
    } // end of loop
} // end of fn ut_setup_bulk_add_charges

#[rustfmt::skip]
async fn ut_verify_unpaid_orderlines(
    repo: Arc<Box<dyn AbstractChargeRepo>>,
    mock_o_toplvl: (u32, &str, u32, DateTime<Utc>),
    mock_o_lines: Vec<(u32, ProductType, u64, Decimal, Decimal, u32, Duration)>,
    expect_paid_lines: Vec<(Decimal, u32)>,
) {
    let result = repo
        .get_unpaid_olines(mock_o_toplvl.0, mock_o_toplvl.1)
        .await;
    if let Ok(Some(v)) = result {
        let iter = expect_paid_lines.into_iter();
        let combined = mock_o_lines.into_iter().zip(iter);
        let olines4verify = combined
             .map(|(a, b)|
                  (a.0, a.1, a.2, a.3, a.4, a.5, b.0, b.1, a.6))
             .collect();
        ut_verify_fetched_order(v, mock_o_toplvl, olines4verify);
    } else {
        assert!(false);
    }
} // end of fn ut_verify_unpaid_orderlines

#[rustfmt::skip]
#[actix_web::test]
async fn read_unpaid_orderline_ok() {
    let shr_state = ut_setup_sharestate();
    let repo = ut_setup_db_charge_repo(shr_state).await;
    // -- init setup , no order line has been charged yet ---
    let mock_buyer_id = 124u32;
    let mock_oids = ["c071ce550de1", "dee23ea715900d"];
    let t_now = Local::now().to_utc();
    let mock_o_toplvl = [
        (mock_buyer_id, mock_oids[0], 2, t_now - Duration::hours(8)),
        (mock_buyer_id, mock_oids[1], 2, t_now - Duration::hours(11)),
    ];
    let mock_olines = [
        vec![
            (8299, ProductType::Package, 37, Decimal::new(31,0), Decimal::new(310,0), 10, Duration::minutes(15)),
            (8299, ProductType::Item, 219, Decimal::new(45,0), Decimal::new(180,0), 4, Duration::minutes(14)),
            (3034, ProductType::Package, 602, Decimal::new(90,0), Decimal::new(450,0), 5, Duration::minutes(13)),
            (3034, ProductType::Item, 595, Decimal::new(112,0), Decimal::new(336,0), 3, Duration::minutes(12)),
            (8299, ProductType::Item, 253, Decimal::new(48,0), Decimal::new(480,0), 10, Duration::minutes(10)),
            (2642, ProductType::Package, 595, Decimal::new(35,0), Decimal::new(175,0), 5, Duration::minutes(6)),
        ],
        vec![
            (2753, ProductType::Package, 152, Decimal::new(33,0), Decimal::new(330,0), 10, Duration::minutes(15)),
            (8299, ProductType::Item, 219, Decimal::new(44,0),   Decimal::new(616,0), 14, Duration::minutes(14)),
            (8299, ProductType::Package, 511, Decimal::new(67,0), Decimal::new(1072,0), 16, Duration::minutes(13)),
            (2642, ProductType::Item,  253,  Decimal::new(68,0), Decimal::new(680,0), 10, Duration::minutes(10)),
            (2642, ProductType::Package, 595, Decimal::new(70,0), Decimal::new(1260,0), 18, Duration::minutes(6)),
        ],
    ];
    let combined = mock_o_toplvl.iter().zip(mock_olines.iter());
    for (d_toplvl, d_olines) in combined {
        ut_setup_new_orderlines(
            repo.clone(), d_toplvl.clone(), d_olines.clone(),
        ).await;
    }
    // This test case adds few charges against a valid order, note this application
    // does not use repository this way, the test code here is simply for
    // verification of the database repository
    let mock_clines_data = vec![
        (t_now - Duration::minutes(13), true, vec![
            (8299, ProductType::Package, 37, (31i64,0u32), (62i64, 0u32), 2u32),
            (8299, ProductType::Item, 219, (45,0), (45,0), 1),
            (3034, ProductType::Package, 602, (90,0), (90,0), 1),
            (2642, ProductType::Package, 595, (35,0), (70,0), 2),
        ]),
        (t_now - Duration::minutes(12), true, vec![
            (3034, ProductType::Package, 602, (90,0), (90,0), 1),
            (8299, ProductType::Item, 253, (48,0),  (144,0), 3),
        ]),
        (t_now - Duration::minutes(11), true, vec![
            (8299, ProductType::Package, 37, (31,0), (93,0), 3),
            (3034, ProductType::Package, 602, (90,0), (180,0), 2),
            (8299, ProductType::Item, 253, (48,0), (192,0), 4),
            (2642, ProductType::Package, 595, (35,0), (35,0), 1),
        ]),
    ];
    ut_setup_bulk_add_charges(
        repo.clone(), mock_buyer_id, mock_oids[0],
        mock_clines_data,
    ).await;
    let mock_clines_data = vec![
        (t_now - Duration::minutes(10), false, vec![
            (2642, ProductType::Item,  253, (68,0), (272,0), 4),
            (2642, ProductType::Package, 595, (70,0), (420,0), 6),
        ]),
        (t_now - Duration::minutes(9), true, vec![
            (2642, ProductType::Item,  253, (68,0), (204,0), 3),
            (2642, ProductType::Package, 595, (70,0), (280,0), 4),
            (2753, ProductType::Package, 152, (33,0), (198,0), 6),
        ]),
        (t_now - Duration::minutes(8), true, vec![
            (2642, ProductType::Item,  253, (68,0), (136,0), 2),
            (2642, ProductType::Package, 595, (70,0), (210,0), 3),
        ]),
    ];
    ut_setup_bulk_add_charges(
        repo.clone(), mock_buyer_id, mock_oids[1],
        mock_clines_data,
    ).await;

    let expect_paid_lines = vec![
        (Decimal::new(155,0), 5),  // 8299, ProductType::Package, 37,   
        (Decimal::new(45,0), 1),   // 8299, ProductType::Item, 219,
        (Decimal::new(360,0), 4),  // 3034, ProductType::Package, 602,
        (Decimal::new(0,0), 0),    // 3034, ProductType::Item, 595,
        (Decimal::new(336,0), 7),  // 8299, ProductType::Item, 253,
        (Decimal::new(105,0), 3),  // 2642, ProductType::Package, 595,
    ];
    ut_verify_unpaid_orderlines(
        repo.clone(), mock_o_toplvl[0].clone(),
        mock_olines[0].clone(), expect_paid_lines,
    ).await;
    
    let expect_paid_lines = vec![
        (Decimal::new(198,0), 6),  // 2753, ProductType::Package, 152,
        (Decimal::new(0,0), 0),    // 8299, ProductType::Item, 219,,
        (Decimal::new(0,0), 0),
        (Decimal::new(340,0), 5),  // 2642, ProductType::Item,  253,
        (Decimal::new(490,0), 7),  // 2642, ProductType::Package, 595,
    ];
    ut_verify_unpaid_orderlines(
        repo.clone(), mock_o_toplvl[1].clone(),
        mock_olines[1].clone(), expect_paid_lines,
    ).await;
} // end of fn read_unpaid_orderline_ok

#[actix_web::test]
async fn read_order_replica_nonexist() {
    let shr_state = ut_setup_sharestate();
    let repo = ut_setup_db_charge_repo(shr_state).await;
    let result = repo.get_unpaid_olines(125, "beef01").await;
    assert!(result.is_ok());
    if let Ok(v) = result {
        assert!(v.is_none());
    }
} // end of fn read_order_replica_nonexist
