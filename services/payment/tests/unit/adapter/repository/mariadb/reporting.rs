use std::boxed::Box;
use std::str::FromStr;
use std::sync::Arc;

use chrono::{DateTime, Duration, Local, Utc};
use rust_decimal::Decimal;

use ecommerce_common::error::AppErrorCode;
use payment::adapter::repository::{AbstractChargeRepo, AppRepoErrorDetail};
use payment::api::web::dto::ReportTimeRangeDto;
use payment::model::BuyerPayInState;

use super::super::{ut_setup_order_bill, ut_setup_orderline_set};
use super::{ut_setup_currency_snapshot, ut_setup_db_charge_repo, ut_setup_db_reporting_repo};
use crate::model::{
    ut_default_charge_method_stripe, ut_setup_buyer_charge, UTestChargeLineRawData,
};
use crate::ut_setup_sharestate;

#[rustfmt::skip]
async fn ut_add_charges(
    repo_chg: Arc<Box<dyn AbstractChargeRepo>> ,
    time_base: DateTime<Utc>,
    buyer_usr_id: u32,
    merchant_id: u32,
    mock_data: Vec<(&str, &str, i64)>
) {
    for d in mock_data {
        let ctime = time_base - Duration::minutes(d.2);
        let mock_state = BuyerPayInState::OrderAppSynced(ctime);
        let mthd_3pty = ut_default_charge_method_stripe(&ctime);
        let sc = {
            let mut map = ut_setup_currency_snapshot(vec![merchant_id, buyer_usr_id]);
            let entry = map.get_mut(&buyer_usr_id).unwrap();
            entry.rate = Decimal::from_str(d.1).unwrap();
            map
        };
        let d_lines: [UTestChargeLineRawData; 4] = [
            ((merchant_id, 1801, 0), ((450, 1), (450, 1), 1), ((0, 0), (0, 0), 0), 0),
            ((merchant_id, 1801, 1), ((462, 1), (231, 0), 5), ((0, 0), (0, 0), 0), 0),
            ((merchant_id, 885,  0), ((291, 1), (873, 1), 3), ((0, 0), (0, 0), 0), 0),
            ((merchant_id, 1707, 0), ((350, 1), (700, 1), 2), ((0, 0), (0, 0), 0), 0),
        ];
        let mock_cline_set = ut_setup_buyer_charge(
            buyer_usr_id, ctime, d.0.to_string(), mock_state,
            mthd_3pty, d_lines.to_vec(), sc,
        );
        let result = repo_chg.create_charge(mock_cline_set).await;
        assert!(result.is_ok());
    } // end of loop
} // end of ut_add_charges

#[actix_web::test]
async fn merchant_fetch_charges_empty() {
    let time_base = Local::now().to_utc();
    let shr_state = ut_setup_sharestate();
    let mock_store_id = 9999;
    let mock_time_range = ReportTimeRangeDto {
        start_after: time_base - Duration::days(10000),
        end_before: time_base + Duration::hours(1),
    };
    let repo = ut_setup_db_reporting_repo(shr_state).await;
    let result = repo
        .fetch_charges_by_merchant(mock_store_id, mock_time_range)
        .await;
    assert!(result.is_ok());
    let charge_ms = result.unwrap();
    assert!(charge_ms.is_empty());
}

#[actix_web::test]
async fn merchant_fetch_charges_ok() {
    let time_base = Local::now().to_utc();
    let shr_state = ut_setup_sharestate();
    let repo_chg = ut_setup_db_charge_repo(shr_state.clone()).await;
    let repo_rpt = ut_setup_db_reporting_repo(shr_state).await;

    let mock_store_id = 9914u32;
    let mock_buyer_usr_id = 128u32;
    let mock_order_data = [
        ("0286808195", "83.48"),
        ("0286808196", "84.54"),
        ("028680819a", "85.11"),
        ("028680819b", "89.64"),
    ];
    for d in mock_order_data {
        let ctime = time_base - Duration::hours(1);
        let sc = {
            let mut map = ut_setup_currency_snapshot(vec![mock_store_id, mock_buyer_usr_id]);
            let entry = map.get_mut(&mock_buyer_usr_id).unwrap();
            entry.rate = Decimal::from_str(d.1).unwrap();
            map
        };
        let d_lines = [
            (1801, 0, "45.0", "340.0", 8),
            (885, 0, "29.0", "261.0", 9),
            (1707, 0, "35.0", "420.0", 12),
        ]
        .into_iter()
        .map(|dl| {
            let amt_unit = Decimal::from_str(dl.2).unwrap();
            let amt_total = Decimal::from_str(dl.3).unwrap();
            let t_rsv = Duration::hours(2);
            (mock_store_id, dl.0, dl.1, amt_unit, amt_total, dl.4, t_rsv)
        })
        .collect::<Vec<_>>();
        let mock_olines = ut_setup_orderline_set(mock_buyer_usr_id, d.0, 0, ctime, sc, d_lines);
        let mock_billing = ut_setup_order_bill();
        let result = repo_chg.create_order(&mock_olines, &mock_billing).await;
        assert!(result.is_ok());
    } // end of loop

    let mock_charge_data = [
        ("0286808195", "83.48", 7i64),
        ("0286808196", "84.54", 11),
        ("028680819a", "85.11", 13),
        ("028680819b", "89.64", 17),
        ("0286808195", "90.91", 23i64),
        ("0286808196", "91.47", 38),
        ("028680819a", "92.03", 45),
        ("028680819b", "93.73", 53),
    ];
    ut_add_charges(
        repo_chg.clone(),
        time_base,
        mock_buyer_usr_id,
        mock_store_id,
        mock_charge_data.to_vec(),
    )
    .await;

    let mock_time_range = ReportTimeRangeDto {
        start_after: time_base - Duration::minutes(46),
        end_before: time_base - Duration::minutes(12),
    };
    let result = repo_rpt
        .fetch_charges_by_merchant(mock_store_id, mock_time_range)
        .await;
    assert!(result.is_ok());
    let charge_ms = result.unwrap();
    assert_eq!(charge_ms.len(), 5);
    charge_ms
        .into_iter()
        .map(|charge_m| {
            let rd_currency = charge_m.currency_snapshot.get(&mock_buyer_usr_id).unwrap();
            let expect = match charge_m.meta.oid().as_str() {
                "0286808195" => "83.48",
                "0286808196" => "84.54",
                "028680819a" => "85.11",
                "028680819b" => "89.64",
                _others => "-9999.99",
            };
            let expect = Decimal::from_str(expect).unwrap();
            assert_eq!(rd_currency.rate, expect);
            assert_eq!(charge_m.lines.len(), 4);
            charge_m
                .lines
                .into_iter()
                .map(|cl| {
                    let actual_charge_id = cl.id();
                    assert_eq!(actual_charge_id.0, mock_store_id);
                    let expect = match (actual_charge_id.1, actual_charge_id.2) {
                        (1801, 0) => ("45.0", "45.0", 1u32),
                        (1801, 1) => ("46.2", "231", 5),
                        (885, 0) => ("29.1", "87.3", 3),
                        (1707, 0) => ("35.0", "70.0", 2),
                        _others => ("0", "0", 0),
                    };
                    let expect_amt_unit = Decimal::from_str(expect.0).unwrap();
                    let expect_amt_total = Decimal::from_str(expect.1).unwrap();
                    assert_eq!(cl.amount_orig().unit, expect_amt_unit);
                    assert_eq!(cl.amount_orig().total, expect_amt_total);
                    assert_eq!(cl.amount_orig().qty, expect.2);
                })
                .count();
        })
        .count();
} // end of fn merchant_fetch_charges_ok

#[actix_web::test]
async fn merchant_fetch_charges_missing_currency() {
    let time_base = Local::now().to_utc();
    let shr_state = ut_setup_sharestate();
    let repo_chg = ut_setup_db_charge_repo(shr_state.clone()).await;
    let repo_rpt = ut_setup_db_reporting_repo(shr_state).await;

    let mock_store_id = 9303u32;
    let mock_buyer_usr_id = 129u32;
    let mock_charge_data = [
        ("02868081a0", "80.91", 21i64),
        ("02868081a2", "81.47", 26),
        ("02868081a3", "82.03", 31),
        ("02868081a4", "83.73", 36),
    ];
    ut_add_charges(
        repo_chg,
        time_base,
        mock_buyer_usr_id,
        mock_store_id,
        mock_charge_data.to_vec(),
    )
    .await;

    let mock_time_range = ReportTimeRangeDto {
        start_after: time_base - Duration::minutes(32),
        end_before: time_base - Duration::minutes(25),
    };
    let result = repo_rpt
        .fetch_charges_by_merchant(mock_store_id, mock_time_range)
        .await;
    assert!(result.is_err());
    if let Err(e) = result {
        assert_eq!(e.code, AppErrorCode::DataCorruption);
        let cond = matches!(e.detail, AppRepoErrorDetail::ConstructChargeFailure(_));
        assert!(cond);
    }
} // end of fn merchant_fetch_charges_missing_currency
