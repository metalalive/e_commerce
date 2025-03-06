use std::collections::HashMap;

use chrono::{DateTime, Duration, Local, Utc};
use rust_decimal::Decimal;

use ecommerce_common::api::dto::CurrencyDto;
use payment::api::web::dto::{ReportChargeRespDto, ReportTimeRangeDto};
use payment::model::{
    BuyerPayInState, ChargeBuyerModel, MerchantReportChargeModel, OrderCurrencySnapshot,
    ReportModelError,
};

use super::{ut_default_charge_method_stripe, ut_setup_buyer_charge};

#[rustfmt::skip]
type UTestReportChargeLineRaw = (u64, u16, (i64, u32), (i64, u32), u32);

#[rustfmt::skip]
fn ut_setup_buyer_charge_inner(
    mock_oid: &str,
    buyer_usr_id: u32,
    charge_ctime: DateTime<Utc>,
    merchant_id: u32,
    pay_in_done : bool,
    merchant_currency :(CurrencyDto, (i64, u32)) ,
    d_lines: Vec<UTestReportChargeLineRaw>,
) -> ChargeBuyerModel {
    let mock_oid = mock_oid  .to_string();
    // for testing purpose , the internal 3rd-party state can be omitted
    let paymethod = ut_default_charge_method_stripe(&charge_ctime);
    let progress = if pay_in_done {
        BuyerPayInState::OrderAppSynced(charge_ctime)
    } else {
        BuyerPayInState::ProcessorAccepted(charge_ctime)
    };
    let currency_snapshot = {
        let iter = [
            (merchant_id, merchant_currency.0, merchant_currency.1),
            (buyer_usr_id, CurrencyDto::USD, (1, 0)),
        ]
        .map(|(usr_id, label, ratescalar)| {
            let rate = Decimal::new(ratescalar.0, ratescalar.1);
            let obj = OrderCurrencySnapshot { label, rate };
            (usr_id, obj)
        });
        HashMap::from_iter(iter)
    };
    let charge_dlines = d_lines
        .into_iter()
        .map(|d| ((merchant_id, d.0, d.1), (d.2, d.3, d.4),
                  ((0i64, 0u32), (0i64, 0u32), 0u32), 0u32))
        .collect::< Vec<_>>() ;
    ut_setup_buyer_charge(
        buyer_usr_id,
        charge_ctime,
        mock_oid,
        progress,
        paymethod,
        charge_dlines,
        currency_snapshot,
    )
} // end of fn ut_setup_buyer_charge_inner

#[test]
fn merge_charges_empty() {
    let time_base = Local::now().to_utc();
    let mock_merchant_id = 5566u32;
    let mock_buyer_usr_id = 8299u32;
    let mock_t_range = ReportTimeRangeDto {
        start_after: time_base - Duration::hours(1),
        end_before: time_base + Duration::hours(1),
    };
    let arg = (mock_merchant_id, mock_t_range);
    let mut report_m = MerchantReportChargeModel::from(arg);
    let charge_ms = vec![ut_setup_buyer_charge_inner(
        "d1e5390dd2",
        mock_buyer_usr_id,
        time_base - Duration::minutes(64),
        mock_merchant_id,
        false,
        (CurrencyDto::TWD, (3184, 2)),
        vec![(463, 0, (201, 1), (1608, 1), 8)],
    )];
    let result = report_m.try_merge(charge_ms);
    assert!(result.is_ok());
    let num_added = result.unwrap();
    assert_eq!(num_added, 0);
}

#[test]
fn merge_charges_ok() {
    let time_base = Local::now().to_utc();
    let mock_merchant_id = 5566u32;
    let mock_buyer_usr_id = 8299u32;
    let mock_t_range = ReportTimeRangeDto {
        start_after: time_base - Duration::hours(2),
        end_before: time_base,
    };
    let charge_ms = vec![
        ut_setup_buyer_charge_inner(
            "d1e5390dd2",
            mock_buyer_usr_id,
            time_base - Duration::minutes(86),
            mock_merchant_id,
            true,
            (CurrencyDto::TWD, (3184, 2)),
            vec![
                (463, 0, (201, 1), (1809, 1), 9),
                (83, 0, (8312, 2), (16624, 2), 2),
            ],
        ),
        ut_setup_buyer_charge_inner(
            "83000010bc35",
            mock_buyer_usr_id,
            time_base - Duration::minutes(54),
            mock_merchant_id,
            true,
            (CurrencyDto::TWD, (3179, 2)),
            vec![
                (463, 0, (203, 1), (609, 1), 3),
                (83, 0, (8348, 2), (8348, 2), 1),
            ],
        ),
        // in rare case, merchant might change currency type to receive
        // TODO, reconsider whether this case could really happen in real world
        ut_setup_buyer_charge_inner(
            "3e08b7f1",
            mock_buyer_usr_id,
            time_base - Duration::minutes(12),
            mock_merchant_id,
            true,
            (CurrencyDto::INR, (8964, 2)),
            vec![
                (463, 0, (203, 1), (609, 1), 3),
                (83, 0, (8348, 2), (83480, 2), 10),
            ],
        ),
        ut_setup_buyer_charge_inner(
            "13e08b7f",
            mock_buyer_usr_id,
            time_base - Duration::minutes(5),
            mock_merchant_id,
            true,
            (CurrencyDto::INR, (6464, 2)),
            vec![
                (463, 0, (202, 1), (1010, 1), 5),
                (83, 0, (8341, 2), (8341, 2), 1),
            ],
        ),
    ]; // end of charge_ms

    let arg = (mock_merchant_id, mock_t_range);
    let mut report_m = MerchantReportChargeModel::from(arg);
    let result = report_m.try_merge(charge_ms);
    assert!(result.is_ok());
    let num_added = result.unwrap();
    assert_eq!(num_added, 8);
    let dto = ReportChargeRespDto::from(report_m);
    dto.lines
        .iter()
        .map(|d| {
            let k = (d.product_id, d.currency.clone());
            let expect = match k {
                (463, CurrencyDto::TWD) => ("7695.86", 12),
                (83, CurrencyDto::TWD) => ("7946.90", 3),
                (463, CurrencyDto::INR) => ("11987.71", 8),
                (83, CurrencyDto::INR) => ("80223.09", 11),
                _others => ("0.0", 0),
            };
            assert_eq!(d.amount.to_string().as_str(), expect.0);
            assert_eq!(d.qty, expect.1);
        })
        .count();
} // end of fn merge_charges_ok

#[test]
fn merge_charges_err_missing_currency() {
    let time_base = Local::now().to_utc();
    let mock_merchant_id = 5566u32;
    let mock_buyer_usr_id = 8299u32;
    let mock_t_range = ReportTimeRangeDto {
        start_after: time_base - Duration::hours(1),
        end_before: time_base,
    };
    let charge_ms = {
        let mut m = ut_setup_buyer_charge_inner(
            "d1e5390dd2",
            mock_buyer_usr_id,
            time_base - Duration::minutes(49),
            mock_merchant_id,
            true,
            (CurrencyDto::TWD, (3176, 2)),
            vec![(83, 0, (8312, 2), (16624, 2), 2)],
        );
        let _discarded = m.currency_snapshot.remove(&mock_merchant_id);
        vec![m]
    };
    let arg = (mock_merchant_id, mock_t_range);
    let mut report_m = MerchantReportChargeModel::from(arg);
    let result = report_m.try_merge(charge_ms);
    assert!(result.is_err());
    if let Err(mut es) = result {
        assert_eq!(es.len(), 1);
        let e = es.remove(0);
        if let ReportModelError::MissingCurrency(actor, actual_id) = e {
            assert_eq!(actor.as_str(), "seller");
            assert_eq!(actual_id, mock_merchant_id);
        } else {
            assert!(false);
        }
    }
} // end of fn merge_charges_err_missing_currency

#[test]
fn merge_charges_err_amount_overflow() {
    let time_base = Local::now().to_utc();
    let mock_merchant_id = 5566u32;
    let mock_buyer_usr_id = 8299u32;
    let mock_t_range = ReportTimeRangeDto {
        start_after: time_base - Duration::hours(1),
        end_before: time_base,
    };
    let charge_ms = {
        let m = ut_setup_buyer_charge_inner(
            "d1e5390dd2",
            mock_buyer_usr_id,
            time_base - Duration::minutes(45),
            mock_merchant_id,
            true,
            (CurrencyDto::TWD, (i64::MAX - 1, 2)),
            vec![(230, 0, (9911, 0), (i64::MAX - 2, 0), 10000)],
        );
        vec![m]
    };
    let arg = (mock_merchant_id, mock_t_range);
    let mut report_m = MerchantReportChargeModel::from(arg);
    let result = report_m.try_merge(charge_ms);
    assert!(result.is_err());
    if let Err(mut es) = result {
        assert_eq!(es.len(), 1);
        let e = es.remove(0);
        if let ReportModelError::AmountOverflow(rate, amt_orig) = e {
            assert_eq!(rate.mantissa() as i64, i64::MAX - 1);
            assert_eq!(amt_orig.mantissa() as i64, i64::MAX - 2);
        } else {
            assert!(false);
        }
    }
} // end of fn merge_charges_err_amount_overflow

#[test]
fn merge_charges_err_merchant_inconsistent() {
    let time_base = Local::now().to_utc();
    let mock_orig_merchant_id = 5566u32;
    let mock_another_merchant_id = 7788u32;
    let mock_buyer_usr_id = 8299u32;
    let mock_t_range = ReportTimeRangeDto {
        start_after: time_base - Duration::hours(1),
        end_before: time_base,
    };
    let charge_ms = {
        let mut m = ut_setup_buyer_charge_inner(
            "d1e5390dd2",
            mock_buyer_usr_id,
            time_base - Duration::minutes(49),
            mock_another_merchant_id,
            true,
            (CurrencyDto::TWD, (3168, 2)),
            vec![
                (83, 0, (8312, 2), (16624, 2), 2),
                (99, 0, (515, 1), (3605, 1), 7),
            ],
        );
        let snapshot = OrderCurrencySnapshot {
            label: CurrencyDto::IDR,
            rate: Decimal::new(130407, 1),
        };
        m.currency_snapshot.insert(mock_orig_merchant_id, snapshot);
        vec![m]
    };
    let arg = (mock_orig_merchant_id, mock_t_range);
    let mut report_m = MerchantReportChargeModel::from(arg);
    let result = report_m.try_merge(charge_ms);
    assert!(result.is_err());
    if let Err(mut es) = result {
        assert_eq!(es.len(), 2);
        let e = es.remove(0);
        if let ReportModelError::MerchantNotConsistent(expect, unexpect) = e {
            assert_eq!(expect, mock_orig_merchant_id);
            assert_eq!(unexpect, mock_another_merchant_id);
        } else {
            assert!(false);
        }
    }
} // end of fn merge_charges_err_merchant_inconsistent
