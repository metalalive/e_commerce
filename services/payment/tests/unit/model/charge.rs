use chrono::{DateTime, Duration, Local, Utc};
use rust_decimal::Decimal;

use ecommerce_common::api::dto::{CurrencyDto, PayAmountDto};
use ecommerce_common::error::AppErrorCode;
use ecommerce_common::model::BaseProductIdentity;

use payment::api::web::dto::{
    ChargeAmountOlineDto, ChargeRefreshRespDto, ChargeReqOrderDto, ChargeStatusDto,
    OrderErrorReason,
};
use payment::model::{
    BuyerPayInState, Charge3partyModel, Charge3partyStripeModel, ChargeBuyerMetaModel,
    ChargeBuyerModel, ChargeToken, PayLineAmountModel, StripeCheckoutPaymentStatusModel,
    StripeSessionStatusModel,
};

use super::order_replica::ut_setup_order_replica;
use super::ut_partial_eq_charge_status_dto;

#[test]
fn buyer_convert_ok_1() {
    let (mock_usr_id, mock_oid) = (582, "phidix".to_string());
    let reserved_until = Local::now().fixed_offset() + Duration::minutes(2);
    let mock_order = ut_setup_order_replica(mock_usr_id, mock_oid.clone(), reserved_until);
    let mock_lines = [
        (140, 1005, 0, 6, "17.15", "102.9"),
        (141, 1006, 0, 1, "21", "21"),
        (143, 1008, 2, 4, "10.60", "42.40"),
        (143, 1008, 0, 4, "10", "40"),
    ]
    .into_iter()
    .map(|d| ChargeAmountOlineDto {
        seller_id: d.0,
        product_id: d.1,
        attr_set_seq: d.2,
        quantity: d.3,
        amount: PayAmountDto {
            unit: d.4.to_string(),
            total: d.5.to_string(),
        },
    })
    .collect();
    let mock_new_req = ChargeReqOrderDto {
        id: mock_oid.clone(),
        lines: mock_lines,
        currency: CurrencyDto::TWD,
    };
    let result = ChargeBuyerModel::try_from((mock_order, mock_new_req));
    assert!(result.is_ok());
    if let Ok(v) = result {
        // println!("token generated , {:?}", &v.token);
        assert_eq!(v.meta.oid(), &mock_oid);
        assert_eq!(v.meta.owner(), mock_usr_id);
        assert!(matches!(v.meta.progress(), BuyerPayInState::Initialized));
        v.lines
            .into_iter()
            .map(|l| {
                let (pid, attr_seq, amt_orig, amt_refunded, num_rejected) = l.into_parts();
                let BaseProductIdentity {
                    store_id,
                    product_id,
                } = pid;
                let PayLineAmountModel { unit, total, qty } = amt_orig;
                let expect = match (store_id, product_id, attr_seq) {
                    (140, 1005, 0) => (6u32, (1715i64, 2u32), (1029i64, 1u32)),
                    (141, 1006, 0) => (1, (21, 0), (21, 0)),
                    (143, 1008, 0) => (4, (10, 0), (40, 0)),
                    (143, 1008, 2) => (4, (106, 1), (424, 1)),
                    _others => (0, (0, 0), (0, 0)),
                };
                let expect = (
                    expect.0,
                    Decimal::new(expect.1 .0, expect.1 .1),
                    Decimal::new(expect.2 .0, expect.2 .1),
                );
                assert_eq!((qty, unit, total), expect);
                let PayLineAmountModel {
                    unit: _,
                    total,
                    qty,
                } = amt_refunded;
                assert_eq!(qty, 0u32);
                assert!(total.is_zero());
                assert_eq!(num_rejected, 0u32);
            })
            .count();
    }
} // end of fn buyer_convert_ok_1

#[test]
fn buyer_convert_ok_2() {
    let (mock_usr_id, mock_oid) = (584, "NikuSan".to_string());
    let reserved_until = Local::now().fixed_offset() + Duration::minutes(2);
    let mut mock_order = ut_setup_order_replica(mock_usr_id, mock_oid.clone(), reserved_until);
    mock_order
        .lines
        .iter_mut()
        .map(|v| {
            v.paid_total.qty = 11;
            v.paid_total.unit = v.rsv_total.unit;
            v.paid_total.total = v.paid_total.unit * Decimal::new(v.paid_total.qty as i64, 0);
        })
        .count();
    let mock_lines = [
        (140, 1005, 0, 9, "17.15", "154.35"),
        (142, 1007, 0, 14, "22.09", "309.26"),
        (142, 1007, 1, 2, "22.56", "45.12"),
        (143, 1008, 0, 7, "10", "70"),
    ]
    .into_iter()
    .map(|d| ChargeAmountOlineDto {
        seller_id: d.0,
        product_id: d.1,
        attr_set_seq: d.2,
        quantity: d.3,
        amount: PayAmountDto {
            unit: d.4.to_string(),
            total: d.5.to_string(),
        },
    })
    .collect();
    let mock_new_req = ChargeReqOrderDto {
        id: mock_oid.clone(),
        lines: mock_lines,
        currency: CurrencyDto::TWD,
    };
    let result = ChargeBuyerModel::try_from((mock_order, mock_new_req));
    assert!(result.is_ok());
    if let Ok(v) = result {
        assert_eq!(v.meta.oid(), &mock_oid);
        assert_eq!(v.meta.owner(), mock_usr_id);
        v.lines
            .into_iter()
            .map(|l| {
                let (
                    actual_pid,
                    actual_attr_set_seq,
                    actual_amt_orig,
                    actual_amt_refunded,
                    actual_num_rejected,
                ) = l.into_parts();
                let BaseProductIdentity {
                    store_id,
                    product_id,
                } = actual_pid;
                let PayLineAmountModel { unit, total, qty } = actual_amt_orig;
                let expect = match (store_id, product_id, actual_attr_set_seq) {
                    (140, 1005, 0) => (9u32, 17150i64, 154350i64),
                    (142, 1007, 0) => (14, 22090, 309260),
                    (142, 1007, 1) => (2, 22560, 45120),
                    (143, 1008, 0) => (7, 10000, 70000),
                    _others => (0, 0, 0),
                };
                let expect = (
                    expect.0,
                    Decimal::new(expect.1, 3),
                    Decimal::new(expect.2, 3),
                );
                assert_eq!((qty, unit, total), expect);
                let PayLineAmountModel {
                    unit: _,
                    total,
                    qty,
                } = actual_amt_refunded;
                assert_eq!(qty, 0u32);
                assert_eq!(total, Decimal::ZERO);
                assert_eq!(actual_num_rejected, 0u32);
            })
            .count();
    }
} // end of fn buyer_convert_ok_2

#[test]
fn buyer_convert_oid_mismatch() {
    let mock_usr_id = 585;
    let reserved_until = Local::now().fixed_offset() + Duration::minutes(2);
    let mock_order = ut_setup_order_replica(mock_usr_id, "lemon".to_string(), reserved_until);
    let mock_new_req = ChargeReqOrderDto {
        id: "lime".to_string(),
        lines: Vec::new(),
        currency: CurrencyDto::TWD,
    };
    let result = ChargeBuyerModel::try_from((mock_order, mock_new_req));
    assert!(result.is_err());
    if let Err(e) = result {
        if let Some(detail) = e.order_id {
            assert!(matches!(detail, OrderErrorReason::InvalidOrder));
        } else {
            assert!(false);
        }
    }
} // end of fn buyer_convert_oid_mismatch

#[test]
fn buyer_convert_currency_mismatch() {
    let mock_usr_id = 585;
    let reserved_until = Local::now().fixed_offset() + Duration::minutes(2);
    let mock_order = ut_setup_order_replica(mock_usr_id, "lemon".to_string(), reserved_until);
    let mock_new_req = ChargeReqOrderDto {
        id: "lemon".to_string(),
        lines: Vec::new(),
        currency: CurrencyDto::USD, // buyer currency should be TWD
    };
    let result = ChargeBuyerModel::try_from((mock_order, mock_new_req));
    assert!(result.is_err());
    if let Err(e) = result {
        assert!(e.currency.is_some());
        if let Some(detail) = e.currency {
            assert_eq!(detail, CurrencyDto::TWD);
        }
    }
} // end of fn buyer_convert_currency_mismatch

#[test]
fn buyer_convert_expired() {
    let (mock_usr_id, mock_oid) = (586, "UncleRoger".to_string());
    let reserved_until = Local::now().fixed_offset() + Duration::minutes(1);
    let mut mock_order = ut_setup_order_replica(mock_usr_id, mock_oid.clone(), reserved_until);
    mock_order
        .lines
        .iter_mut()
        .map(|l| {
            l.reserved_until -= Duration::minutes(7);
        })
        .count(); // assume expiry time reached
    let mock_lines = [
        (140, 1005, 1, "17.15", "17.15"),
        (141, 1006, 1, "21", "21"),
        (142, 1007, 1, "22.09", "22.09"),
        (143, 1008, 1, "10", "10"),
    ]
    .into_iter()
    .map(|d| ChargeAmountOlineDto {
        seller_id: d.0,
        product_id: d.1,
        attr_set_seq: 0,
        quantity: d.2,
        amount: PayAmountDto {
            unit: d.3.to_string(),
            total: d.4.to_string(),
        },
    })
    .collect();
    let mock_new_req = ChargeReqOrderDto {
        id: mock_oid.clone(),
        lines: mock_lines,
        currency: CurrencyDto::TWD,
    };
    let result = ChargeBuyerModel::try_from((mock_order, mock_new_req));
    assert!(result.is_err());
    if let Err(e) = result {
        if let Some(detail) = e.lines {
            detail
                .into_iter()
                .map(|l| {
                    let expect_pid = [(140u32, 1005u64), (141, 1006)];
                    let actual_pid = (l.seller_id, l.product_id);
                    assert!(expect_pid.contains(&actual_pid));
                    assert!(l.expired.unwrap());
                })
                .count();
        } else {
            assert!(false);
        }
    }
} // end of fn buyer_convert_expired

#[test]
fn buyer_convert_qty_exceed_limit() {
    let (mock_usr_id, mock_oid) = (587, "hijurie3k7".to_string());
    let reserved_until = Local::now().fixed_offset() + Duration::minutes(2);
    let mut mock_order = ut_setup_order_replica(mock_usr_id, mock_oid.clone(), reserved_until);
    mock_order
        .lines
        .iter_mut()
        .map(|v| {
            v.paid_total.qty = 11;
            v.paid_total.unit = v.rsv_total.unit;
            v.paid_total.total = v.paid_total.unit * Decimal::new(v.paid_total.qty as i64, 0);
        })
        .count();
    let mock_lines = [
        (140, 1005, 0, 8, "17.15", "137.2"),
        (141, 1006, 0, 2, "21", "42"),
        (142, 1007, 0, 15, "22.09", "331.35"),
        (143, 1008, 0, 7, "10", "70"),
        (143, 1008, 1, 1, "10.29", "10.29"),
    ]
    .into_iter()
    .map(|d| ChargeAmountOlineDto {
        seller_id: d.0,
        product_id: d.1,
        attr_set_seq: d.2,
        quantity: d.3,
        amount: PayAmountDto {
            unit: d.4.to_string(),
            total: d.5.to_string(),
        },
    })
    .collect();
    let mock_new_req = ChargeReqOrderDto {
        id: mock_oid.clone(),
        lines: mock_lines,
        currency: CurrencyDto::TWD,
    };
    let result = ChargeBuyerModel::try_from((mock_order, mock_new_req));
    assert!(result.is_err());
    if let Err(e) = result {
        if let Some(detail) = e.lines {
            detail
                .into_iter()
                .map(|l| {
                    let actual_pid = (l.seller_id, l.product_id, l.attr_set_seq);
                    let expect = match actual_pid {
                        (142u32, 1007u64, 0u16) => (14u16, 15u32),
                        (141, 1006, 0) => (0, 2),
                        (143, 1008, 1) => (3, 11), // total-rsv, total-paid
                        _others => (9999, 9999),
                    };
                    // assert!(l.amount.is_none());
                    let range_err = l.quantity.unwrap();
                    let actual = (range_err.max_, range_err.given);
                    assert_eq!(expect, actual);
                })
                .count();
        } else {
            assert!(false);
        }
    }
} // end of  fn buyer_convert_qty_exceed_limit

#[test]
fn buyer_convert_amount_mismatch_1() {
    let (mock_usr_id, mock_oid) = (589, "nova".to_string());
    let reserved_until = Local::now().fixed_offset() + Duration::minutes(2);
    let mock_order = ut_setup_order_replica(mock_usr_id, mock_oid.clone(), reserved_until);
    let mock_lines = [
        (140, 1005, 9, "17.01", "159"),
        (141, 1006, 2, "21", "42"),
        (142, 1007, 15, "22.09", "333"),
        (143, 1008, 7, "10", "70"),
    ]
    .into_iter()
    .map(|d| ChargeAmountOlineDto {
        seller_id: d.0,
        product_id: d.1,
        attr_set_seq: 0,
        quantity: d.2,
        amount: PayAmountDto {
            unit: d.3.to_string(),
            total: d.4.to_string(),
        },
    })
    .collect();
    let mock_new_req = ChargeReqOrderDto {
        id: mock_oid.clone(),
        lines: mock_lines,
        currency: CurrencyDto::TWD,
    };
    let result = ChargeBuyerModel::try_from((mock_order, mock_new_req));
    assert!(result.is_err());
    if let Err(e) = result {
        if let Some(detail) = e.lines {
            detail
                .into_iter()
                .map(|l| {
                    let actual_pid = (l.seller_id, l.product_id);
                    let expect = match actual_pid {
                        (140u32, 1005u64) => (9u32, "17.01", "159"),
                        (142, 1007) => (15, "22.09", "333"),
                        _others => (0, "0", "0"),
                    };
                    let expect = (expect.0, expect.1.to_string(), expect.2.to_string());
                    let qty_err = l.quantity.unwrap();
                    let amt_err = l.amount.unwrap();
                    let actual = (qty_err.given, amt_err.unit, amt_err.total);
                    assert_eq!(expect, actual);
                })
                .count();
        }
    }
} // end of fn buyer_convert_amount_mismatch_1

#[test]
fn buyer_convert_amount_mismatch_2() {
    let (mock_usr_id, mock_oid) = (587, "hijurie3k7".to_string());
    let reserved_until = Local::now().fixed_offset() + Duration::minutes(2);
    let mock_order = ut_setup_order_replica(mock_usr_id, mock_oid.clone(), reserved_until);
    let mock_lines = [
        (141, 1006, 2, "21", "42"),
        (142, 1007, 15, "22.09", "331.35"),
        (143, 1008, 7, "11", "77"),
    ]
    .into_iter()
    .map(|d| ChargeAmountOlineDto {
        seller_id: d.0,
        product_id: d.1,
        attr_set_seq: 0,
        quantity: d.2,
        amount: PayAmountDto {
            unit: d.3.to_string(),
            total: d.4.to_string(),
        },
    })
    .collect();
    let mock_new_req = ChargeReqOrderDto {
        id: mock_oid.clone(),
        lines: mock_lines,
        currency: CurrencyDto::TWD,
    };
    let result = ChargeBuyerModel::try_from((mock_order, mock_new_req));
    assert!(result.is_err());
    if let Err(e) = result {
        if let Some(detail) = e.lines {
            detail
                .into_iter()
                .map(|l| {
                    let actual_pid = (l.seller_id, l.product_id);
                    let expect = match actual_pid {
                        (143u32, 1008u64) => (11u32.to_string(), 77u32.to_string()),
                        _others => (0.to_string(), 0.to_string()),
                    };
                    assert!(l.quantity.is_none());
                    let amt_err = l.amount.unwrap();
                    let actual = (amt_err.unit, amt_err.total);
                    assert_eq!(expect, actual);
                })
                .count();
        }
    }
} // end of fn buyer_convert_amount_mismatch_2

fn ut_default_charge_3pty_stripe(
    session_state: StripeSessionStatusModel,
    payment_state: StripeCheckoutPaymentStatusModel,
) -> Charge3partyModel {
    let s = Charge3partyStripeModel {
        checkout_session_id: "mock-unit-test".to_string(),
        session_state,
        payment_state,
        payment_intent_id: "mock-unit-test".to_string(),
        transfer_group: "mock-tx-grp-utest".to_string(),
        expiry: Local::now().to_utc() + Duration::minutes(10),
    };
    Charge3partyModel::Stripe(s)
}

#[test]
fn buyer_meta_to_resp_dto_ok() {
    let mock_owner = 1717u32;
    let mock_now_time = Local::now().to_utc();
    let mock_oid = "b90b273c72".to_string();
    [
        (
            BuyerPayInState::Initialized,
            StripeSessionStatusModel::open,
            StripeCheckoutPaymentStatusModel::unpaid,
            ChargeStatusDto::Initialized,
        ),
        (
            BuyerPayInState::ProcessorAccepted(mock_now_time + Duration::minutes(1)),
            StripeSessionStatusModel::open,
            StripeCheckoutPaymentStatusModel::unpaid,
            ChargeStatusDto::PspProcessing,
        ),
        (
            BuyerPayInState::ProcessorAccepted(mock_now_time + Duration::minutes(1)),
            StripeSessionStatusModel::complete,
            StripeCheckoutPaymentStatusModel::paid,
            ChargeStatusDto::InternalSyncing,
        ),
        (
            BuyerPayInState::ProcessorAccepted(mock_now_time + Duration::minutes(1)),
            StripeSessionStatusModel::complete,
            StripeCheckoutPaymentStatusModel::unpaid,
            ChargeStatusDto::PspRefused,
        ),
        (
            BuyerPayInState::ProcessorAccepted(mock_now_time + Duration::minutes(1)),
            StripeSessionStatusModel::expired,
            StripeCheckoutPaymentStatusModel::unpaid,
            ChargeStatusDto::SessionExpired,
        ),
        (
            BuyerPayInState::ProcessorCompleted(mock_now_time + Duration::minutes(2)),
            StripeSessionStatusModel::open,
            StripeCheckoutPaymentStatusModel::unpaid,
            ChargeStatusDto::PspProcessing,
        ),
        (
            BuyerPayInState::ProcessorCompleted(mock_now_time + Duration::minutes(2)),
            StripeSessionStatusModel::complete,
            StripeCheckoutPaymentStatusModel::paid,
            ChargeStatusDto::InternalSyncing,
        ),
        (
            BuyerPayInState::ProcessorCompleted(mock_now_time + Duration::minutes(2)),
            StripeSessionStatusModel::complete,
            StripeCheckoutPaymentStatusModel::unpaid,
            ChargeStatusDto::PspRefused,
        ),
        (
            BuyerPayInState::ProcessorCompleted(mock_now_time + Duration::minutes(2)),
            StripeSessionStatusModel::expired,
            StripeCheckoutPaymentStatusModel::unpaid,
            ChargeStatusDto::SessionExpired,
        ),
        (
            BuyerPayInState::OrderAppSynced(mock_now_time + Duration::minutes(3)),
            StripeSessionStatusModel::complete,
            StripeCheckoutPaymentStatusModel::paid,
            ChargeStatusDto::Completed,
        ),
    ]
    .into_iter()
    .map(|(payin_state, session_3pty, payment_3pty, expect_dto)| {
        // FIXME, linter false alarm, `expect_dto` is actually used
        let mock_method = ut_default_charge_3pty_stripe(session_3pty, payment_3pty);
        let arg = (mock_oid.clone(), mock_owner, mock_now_time);
        let mut meta = ChargeBuyerMetaModel::from(arg);
        meta.update_progress(&payin_state);
        meta.update_3party(mock_method);
        let resp = ChargeRefreshRespDto::from(&meta);
        let cond = ut_partial_eq_charge_status_dto(&resp.status, &expect_dto);
        assert!(cond);
        assert_eq!(resp.order_id, mock_oid.clone());
    })
    .count();
} // end of fn buyer_meta_to_resp_dto_ok

#[test]
fn buyer_3pty_pay_in_confirm() {
    [
        (
            StripeSessionStatusModel::open,
            StripeCheckoutPaymentStatusModel::unpaid,
            None,
        ),
        (
            StripeSessionStatusModel::open,
            StripeCheckoutPaymentStatusModel::paid,
            None,
        ),
        (
            StripeSessionStatusModel::complete,
            StripeCheckoutPaymentStatusModel::paid,
            Some(true),
        ),
        (
            StripeSessionStatusModel::complete,
            StripeCheckoutPaymentStatusModel::unpaid,
            Some(false),
        ),
        (
            StripeSessionStatusModel::expired,
            StripeCheckoutPaymentStatusModel::unpaid,
            Some(false),
        ),
        (
            StripeSessionStatusModel::expired,
            StripeCheckoutPaymentStatusModel::paid,
            Some(false),
        ),
    ]
    .into_iter()
    .map(|(session_3pty, payment_3pty, expect)| {
        let mock_3pty = ut_default_charge_3pty_stripe(session_3pty, payment_3pty);
        let actual = mock_3pty.pay_in_comfirmed();
        assert_eq!(actual, expect);
    })
    .count();
} // end of fn buyer_3pty_pay_in_confirm

#[rustfmt::skip]
#[test]
fn charge_token_encode_ok() {
    let given_data = [
        (
            8374u32,
            "1998-10-31T18:38:25+00:00",
            [0x0u8, 0x0, 0x20, 0xb6, 0x1f, 0x38 | 0x2, 0x80 | 0x3e | 0x1, 0x20 | 0x9, 0x80 | 0x19],
        ),
        (
            8010095,
            "2012-04-24T23:01:30+00:00",
            [0x0, 0x7a, 0x39, 0x6f, 0x1f, 0x70 | 0x1, 0x0 | 0x30 | 0x01, 0x70 | 0x0, 0x40 | 0x1e],
        ),
        (
            100290278,
            "2019-01-17T00:59:35+00:00",
            [0x05, 0xfa, 0x4e, 0xe6, 0x1f, 0x8c | 0x0, 0x40 | 0x22 | 0x0, 0x0 | 0xe, 0xc0 | 0x23],
        ),
        (
            305419896,
            "2015-12-07T13:47:05+00:00",
            [0x12, 0x34, 0x56, 0x78, 0x1f, 0x7F, 0x0E, 0xdb, 0xc5],
        ),
        (
            1122867,
            "2007-02-28T06:25:40+00:00",
            [0x00, 0x11, 0x22, 0x33, 0x1f, 0x5c, 0xb8, 0x66, 0x68],
        ),
    ];
    given_data
        .clone()
        .into_iter()
        .map(|(mock_usr_id, time_serial, expect_encoded)| {
            let mock_ctime = DateTime::parse_from_rfc3339(time_serial).unwrap().to_utc();
            let actual = ChargeToken::encode(mock_usr_id, mock_ctime);
            assert_eq!(actual.0, expect_encoded);
        })
        .count();
    given_data
        .into_iter()
        .map(|(expect_usr_id, time_serial, tok_encoded)| {
            let expect_time = DateTime::parse_from_rfc3339(time_serial).unwrap();
            let tok = ChargeToken::try_from(tok_encoded.to_vec()).unwrap();
            let actual: (u32, DateTime<Utc>) = tok.try_into().unwrap();
            assert_eq!(actual.0, expect_usr_id);
            assert_eq!(actual.1, expect_time);
        })
        .count();
} // end of fn charge_token_encode_ok

#[rustfmt::skip]
#[test]
fn charge_token_decode_err() {
    // year of the time is zero
    let data = [0x00, 0x11, 0x22, 0x33, 0x1f, 0x50, 0x00, 0x66, 0x68];
    let tok = ChargeToken::try_from(data.to_vec()).unwrap();
    let result: Result<(u32, DateTime<Utc>), (AppErrorCode, String)> = tok.try_into();
    assert!(result.is_err());
    if let Err((code, _detail)) = result {
        assert_eq!(code, AppErrorCode::DataCorruption);
    }
} // end of fn charge_token_decode_err
