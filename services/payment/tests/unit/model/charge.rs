use chrono::{DateTime, Duration, FixedOffset, Local};
use rust_decimal::Decimal;

use ecommerce_common::api::dto::{
    CurrencyDto, CurrencySnapshotDto, OrderCurrencySnapshotDto, OrderLinePayDto,
    OrderSellerCurrencyDto, PayAmountDto,
};
use ecommerce_common::constant::ProductType;
use ecommerce_common::model::BaseProductIdentity;

use payment::api::web::dto::{
    ChargeAmountOlineDto, ChargeReqDto, OrderErrorReason, PaymentMethodReqDto,
    StripeCheckoutSessionReqDto, StripeCheckoutUImodeDto,
};
use payment::model::{
    BuyerPayInState, ChargeBuyerModel, ChargeToken, OrderLineModelSet, OrderModelError,
    PayLineAmountError, PayLineAmountModel,
};

fn ut_default_currency_snapshot_dto(seller_ids: Vec<u32>) -> OrderCurrencySnapshotDto {
    OrderCurrencySnapshotDto {
        snapshot: vec![CurrencySnapshotDto {
            name: CurrencyDto::TWD,
            rate: "32.060".to_string(),
        }],
        sellers: seller_ids
            .into_iter()
            .map(|seller_id| OrderSellerCurrencyDto {
                seller_id,
                currency: CurrencyDto::TWD,
            })
            .collect::<Vec<_>>(),
        buyer: CurrencyDto::TWD,
    }
}

#[rustfmt::skip]
fn ut_setup_order_replica(
    mock_buyer_id: u32,
    mock_oid: String,
    reserved_until: DateTime<FixedOffset>,
) -> OrderLineModelSet {
    let mock_lines = [
        (140, ProductType::Item, 1005, 20, "17.015", "340.3", Duration::minutes(6)),
        (141, ProductType::Package, 1006, 11, "21.0", "231.0", Duration::minutes(4)),
        (142, ProductType::Item, 1007, 25, "22.09", "552.25", Duration::minutes(10)),
        (143, ProductType::Package, 1008, 18, "10.0", "180.0", Duration::minutes(9)),
    ]
    .into_iter()
    .map(|d| OrderLinePayDto {
        seller_id: d.0, product_id: d.2, product_type: d.1,
        reserved_until: (reserved_until + d.6).to_rfc3339(), quantity: d.3,
        amount: PayAmountDto {unit: d.4.to_string(), total: d.5.to_string()},
    })
    .collect::<Vec<_>>();
    let mock_currency_snapshot = {
        let mut cs = ut_default_currency_snapshot_dto(vec![143,140,141,142]);
        let second_currency = CurrencySnapshotDto {
            name: CurrencyDto::THB, rate: "37.012".to_string()
        };
        let _cnt = cs.sellers.iter_mut()
            .filter(|v| [140,143].contains(&v.seller_id))
            .map(|v| { v.currency = second_currency.name.clone(); })
            .count();
        cs.snapshot.push(second_currency);
        cs
    };
    let result = OrderLineModelSet::try_from(
        (mock_oid, mock_buyer_id, mock_lines, mock_currency_snapshot)
    );
    assert!(result.is_ok());
    if let Ok(v) = &result {
        v.currency_snapshot.iter().map(|(usr_id, cm)| {
            let expect = if usr_id == &mock_buyer_id {
                (CurrencyDto::TWD, Decimal::new(3206, 2))
            } else {
                match usr_id {
                    140 | 143 => (CurrencyDto::THB, Decimal::new(37012, 3)),
                    141 | 142 => (CurrencyDto::TWD, Decimal::new(3206, 2)),
                    _others => (CurrencyDto::Unknown, Decimal::new(0, 0)),
                }
            };
            assert_eq!((cm.label.clone(), cm.rate.clone()), expect);
        }).count();
    }
    result.unwrap()
} // end of fn ut_setup_order_replica

fn ut_setup_payment_method_stripe() -> PaymentMethodReqDto {
    PaymentMethodReqDto::Stripe(StripeCheckoutSessionReqDto {
        customer_id: Some("ut-stripe-customer-id".to_string()),
        success_url: Some("https://3aw.au".to_string()),
        return_url: Some("https://4ec.au".to_string()),
        cancel_url: None,
        ui_mode: StripeCheckoutUImodeDto::RedirectPage,
    })
}

#[actix_web::test]
async fn order_replica_convert_ok() {
    let (mock_usr_id, mock_oid) = (456, "xyz987".to_string());
    let reserved_until = Local::now().fixed_offset() + Duration::minutes(3);
    let out = ut_setup_order_replica(mock_usr_id, mock_oid, reserved_until);
    assert_eq!(out.lines.len(), 4);
}

#[actix_web::test]
async fn order_replica_convert_empty_line() {
    let (mock_usr_id, mock_oid) = (456, "xyz987".to_string());
    let mock_lines = Vec::new();
    let mock_currency_snapshot = ut_default_currency_snapshot_dto(vec![]);
    let result =
        OrderLineModelSet::try_from((mock_oid, mock_usr_id, mock_lines, mock_currency_snapshot));
    assert!(result.is_err());
    if let Err(e) = result {
        assert_eq!(e.len(), 1);
        let cond = matches!(e[0], OrderModelError::EmptyLine);
        assert!(cond);
    }
}

#[actix_web::test]
async fn order_replica_convert_qty_zero() {
    let (mock_usr_id, mock_oid) = (456, "xyz987".to_string());
    let reserved_until = Local::now().fixed_offset() + Duration::minutes(3);
    let mock_lines = [
        (140, ProductType::Item, 1005, 0, 17, 85),
        (141, ProductType::Package, 1006, 11, 21, 231),
        (142, ProductType::Item, 1007, 10, 23, 230),
    ]
    .into_iter()
    .map(|d| OrderLinePayDto {
        seller_id: d.0,
        product_id: d.2,
        product_type: d.1,
        reserved_until: reserved_until.to_rfc3339(),
        quantity: d.3,
        amount: PayAmountDto {
            unit: d.4.to_string(),
            total: d.5.to_string(),
        },
    })
    .collect::<Vec<_>>();
    let mock_currency_snapshot = ut_default_currency_snapshot_dto(vec![140, 141, 142]);
    let result =
        OrderLineModelSet::try_from((mock_oid, mock_usr_id, mock_lines, mock_currency_snapshot));
    assert!(result.is_err());
    if let Err(e) = result {
        assert_eq!(e.len(), 1);
        if let OrderModelError::ZeroQuantity(pid) = &e[0] {
            assert_eq!(pid.store_id, 140);
            assert_eq!(pid.product_type, ProductType::Item);
            assert_eq!(pid.product_id, 1005);
        } else {
            assert!(false);
        }
    }
} // end of fn order_replica_convert_qty_zero

#[rustfmt::skip]
#[actix_web::test]
async fn order_replica_convert_line_expired() {
    let (mock_usr_id, mock_oid) = (456, "xyz987".to_string());
    let now = Local::now().fixed_offset();
    let mock_lines = [
        (140, ProductType::Item, 1005, 5, 17, 85, Duration::minutes(3)),
        (141, ProductType::Package, 1006, 11, 21, 231, Duration::seconds(19)),
        (142, ProductType::Item, 1007, 10, 23, 230, Duration::seconds(-2)),
    ]
    .into_iter()
    .map(|d| OrderLinePayDto {
        seller_id: d.0,  product_id: d.2,  product_type: d.1,
        reserved_until: (now + d.6).to_rfc3339(),  quantity: d.3,
        amount: PayAmountDto {unit: d.4.to_string(), total: d.5.to_string()},
    })
    .collect::<Vec<_>>();
    let mock_currency_snapshot = ut_default_currency_snapshot_dto(vec![140,141,142]);
    let result = OrderLineModelSet::try_from(
        (mock_oid, mock_usr_id, mock_lines, mock_currency_snapshot)
    );
    assert!(result.is_err());
    if let Err(e) = result {
        assert_eq!(e.len(), 1);
        if let Some(OrderModelError::RsvExpired(pid)) = e.first() {
            assert_eq!(pid.store_id, 142);
            assert_eq!(pid.product_type, ProductType::Item);
            assert_eq!(pid.product_id, 1007);
        } else {
            assert!(false);
        }
    }
} // end of fn order_replica_convert_line_expired

#[rustfmt::skip]
#[actix_web::test]
async fn order_replica_convert_missing_currency_1() {
    let (mock_buyer_id, mock_oid) = (149, "9a800f71b".to_string());
    let now = Local::now().fixed_offset();
    let mock_lines = [
        (141, ProductType::Package, 1006, 11, "21.0", "231.0", Duration::minutes(4)),
        (142, ProductType::Item, 1007, 25, "22.0", "550.0", Duration::minutes(10)),
        (143, ProductType::Package, 1008, 18, "10.0", "180.0", Duration::minutes(9)),
    ]
    .into_iter()
    .map(|d| OrderLinePayDto {
        seller_id: d.0, product_id: d.2, product_type: d.1,
        reserved_until: (now + d.6).to_rfc3339(), quantity: d.3,
        amount: PayAmountDto {unit: d.4.to_string(), total: d.5.to_string()},
    })
    .collect::<Vec<_>>();
    let mock_currency_snapshot = {
        let mut cs = ut_default_currency_snapshot_dto(vec![143,141,142]);
        let _ = cs.sellers.iter_mut()
            .find(|v| 142 == v.seller_id)
            .map(|v| { v.currency = CurrencyDto::USD; });
        cs
    };
    let result = OrderLineModelSet::try_from(
        (mock_oid, mock_buyer_id, mock_lines, mock_currency_snapshot)
    );
    assert!(result.is_err());
    if let Err(e) = result {
        assert_eq!(e.len(), 1);
        if let Some(OrderModelError::MissingExRate(c)) = e.first() {
            assert_eq!(c, &CurrencyDto::USD);
        } else {
            assert!(false);
        }
    }
} // end of fn order_replica_convert_missing_currency_1

#[rustfmt::skip]
#[actix_web::test]
async fn order_replica_convert_missing_currency_2() {
    let (mock_buyer_id, mock_oid) = (158, "9a800f71b".to_string());
    let now = Local::now().fixed_offset();
    let mock_lines = [
        (143, ProductType::Package, 1006, 11, "21.0", "231.0", Duration::minutes(4)),
        (144, ProductType::Item, 1007, 25, "22.0", "550.0", Duration::minutes(10)),
        (145, ProductType::Package, 1008, 18, "10.0", "180.0", Duration::minutes(9)),
    ]
    .into_iter()
    .map(|d| OrderLinePayDto {
        seller_id: d.0, product_id: d.2, product_type: d.1,
        reserved_until: (now + d.6).to_rfc3339(), quantity: d.3,
        amount: PayAmountDto {unit: d.4.to_string(), total: d.5.to_string()},
    })
    .collect::<Vec<_>>();
    let mock_currency_snapshot = ut_default_currency_snapshot_dto(vec![143,145]);
    let result = OrderLineModelSet::try_from(
        (mock_oid, mock_buyer_id, mock_lines, mock_currency_snapshot)
    );
    assert!(result.is_err());
    if let Err(e) = result {
        assert_eq!(e.len(), 1);
        if let Some(OrderModelError::MissingActorsCurrency(c)) = e.first() {
            assert_eq!(c.len(), 1);
            assert_eq!(c[0], 144);
        } else {
            assert!(false);
        }
    }
} // end of fn  fn order_replica_convert_missing_currency_2()

#[rustfmt::skip]
#[actix_web::test]
async fn order_replica_convert_corrupted_currency_rate() {
    let (mock_buyer_id, mock_oid) = (558, "9a800f71b".to_string());
    let now = Local::now().fixed_offset();
    let mock_lines = [
        (146, ProductType::Item, 1007, 25, "22.0", "550.0", Duration::minutes(10)),
        (147, ProductType::Package, 1008, 18, "10.0", "180.0", Duration::minutes(9)),
    ]
    .into_iter()
    .map(|d| OrderLinePayDto {
        seller_id: d.0, product_id: d.2, product_type: d.1,
        reserved_until: (now + d.6).to_rfc3339(), quantity: d.3,
        amount: PayAmountDto {unit: d.4.to_string(), total: d.5.to_string()},
    })
    .collect::<Vec<_>>();
    let mock_currency_snapshot = {
        let mut cs = ut_default_currency_snapshot_dto(vec![146,147]);
        let _ = cs.snapshot.first_mut()
            .map(|v| { v.rate = "32.ky8".to_string(); });
        cs
    };
    let result = OrderLineModelSet::try_from(
        (mock_oid, mock_buyer_id, mock_lines, mock_currency_snapshot)
    );
    assert!(result.is_err());
    if let Err(e) = result {
        assert_eq!(e.len(), 2);
        if let Some(OrderModelError::CorruptedExRate(c, _reason)) = e.first() {
            assert_eq!(c, &CurrencyDto::TWD);
        } else {
            assert!(false);
        }
    }
} // end of fn order_replica_convert_corrupted_currency_rate

#[rustfmt::skip]
#[actix_web::test]
async fn order_replica_convert_invalid_amount() {
    let (mock_usr_id, mock_oid) = (456, "xyz987".to_string());
    let reserved_until = Local::now().fixed_offset() + Duration::minutes(3);
    let mock_lines = [
        (140, ProductType::Item, 1005, 5, "L7", "85"),
        (141, ProductType::Package, 1006, 11, "24", "261"),
        (142, ProductType::Item, 1007, 10, "23", "230s"),
        (143, ProductType::Package, 1011, u32::MAX, "1230045600789001230045600", "2900"),
    ]
    .into_iter()
    .map(|d| OrderLinePayDto {
        seller_id: d.0, product_id: d.2, product_type: d.1,
        reserved_until: reserved_until.to_rfc3339(), quantity: d.3,
        amount: PayAmountDto {unit: d.4.to_string(), total: d.5.to_string()},
    })
    .collect::<Vec<_>>();
    let mock_currency_snapshot = ut_default_currency_snapshot_dto(vec![140, 141, 142, 143]);
    let result =
        OrderLineModelSet::try_from((mock_oid, mock_usr_id, mock_lines, mock_currency_snapshot));
    assert!(result.is_err());
    if let Err(es) = result {
        assert_eq!(es.len(), 4);
        es.into_iter().map(|e| match e {
            OrderModelError::InvalidAmount(pid, pe) => {
                let BaseProductIdentity { store_id, product_type, product_id } = pid;
                match (store_id, product_type, product_id) {
                    (141, ProductType::Package, 1006) =>
                        if let PayLineAmountError::Mismatch(amount, qty) = pe {
                            assert_eq!(amount.unit.as_str(), "24");
                            assert_eq!(amount.total.as_str(), "261");
                            assert_eq!(qty, 11u32);
                        } else { assert!(false); },
                    (140, ProductType::Item, 1005) =>
                        if let PayLineAmountError::ParseUnit(value, _reason) = pe {
                            assert_eq!(value.as_str(), "L7");
                        } else { assert!(false); },
                    (142, ProductType::Item, 1007) =>
                        if let PayLineAmountError::ParseTotal(value, _reason) = pe {
                            assert_eq!(value.as_str(), "230s");
                        } else { assert!(false); },
                    (143, ProductType::Package, 1011) =>
                        if let PayLineAmountError::Overflow(amt_unit, qty) = pe {
                            assert_eq!(qty, u32::MAX);
                            assert_eq!(amt_unit.as_str(), "1230045600789001230045600");
                        } else { assert!(false); },
                    _others => { assert!(false); },
                }
            },
            _others => { assert!(false); },
        }).count();
    }
} // end of fn order_replica_convert_invalid_amount

#[actix_web::test]
async fn charge_buyer_convert_ok_1() {
    let (mock_usr_id, mock_oid) = (582, "phidix".to_string());
    let reserved_until = Local::now().fixed_offset() + Duration::minutes(2);
    let mock_order = ut_setup_order_replica(mock_usr_id, mock_oid.clone(), reserved_until);
    let mock_lines = [
        (140, ProductType::Item, 1005, 6, "17.015", "102.09"),
        (141, ProductType::Package, 1006, 1, "21", "21"),
        (143, ProductType::Package, 1008, 4, "10", "40"),
    ]
    .into_iter()
    .map(|d| ChargeAmountOlineDto {
        seller_id: d.0,
        product_id: d.2,
        product_type: d.1,
        quantity: d.3,
        amount: PayAmountDto {
            unit: d.4.to_string(),
            total: d.5.to_string(),
        },
    })
    .collect();
    let mock_new_req = ChargeReqDto {
        order_id: mock_oid.clone(),
        lines: mock_lines,
        method: ut_setup_payment_method_stripe(),
        currency: CurrencyDto::TWD,
    };
    let result = ChargeBuyerModel::try_from((mock_order, mock_new_req));
    assert!(result.is_ok());
    if let Ok(v) = result {
        // println!("token generated , {:?}", &v.token);
        assert_eq!(v.oid, mock_oid);
        assert_eq!(v.owner, mock_usr_id);
        assert!(matches!(v.state, BuyerPayInState::Initialized));
        v.lines
            .into_iter()
            .map(|l| {
                let (actual_pid, actual_amount) = (l.pid, l.amount);
                let BaseProductIdentity {
                    store_id,
                    product_type,
                    product_id,
                } = actual_pid;
                let PayLineAmountModel { unit, total, qty } = actual_amount;
                let expect = match (store_id, product_type, product_id) {
                    (140, ProductType::Item, 1005) => (6u32, (17015i64, 3u32), (10209i64, 2u32)),
                    (141, ProductType::Package, 1006) => (1, (21, 0), (21, 0)),
                    (143, ProductType::Package, 1008) => (4, (10, 0), (40, 0)),
                    _others => (0, (0, 0), (0, 0)),
                };
                let expect = (
                    expect.0,
                    Decimal::new(expect.1 .0, expect.1 .1),
                    Decimal::new(expect.2 .0, expect.2 .1),
                );
                assert_eq!((qty, unit, total), expect);
            })
            .count();
    }
} // end of fn charge_buyer_convert_ok_1

#[actix_web::test]
async fn charge_buyer_convert_ok_2() {
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
        (140, ProductType::Item, 1005, 9, "17.015", "153.135"),
        (142, ProductType::Item, 1007, 14, "22.09", "309.26"),
        (143, ProductType::Package, 1008, 7, "10", "70"),
    ]
    .into_iter()
    .map(|d| ChargeAmountOlineDto {
        seller_id: d.0,
        product_id: d.2,
        product_type: d.1,
        quantity: d.3,
        amount: PayAmountDto {
            unit: d.4.to_string(),
            total: d.5.to_string(),
        },
    })
    .collect();
    let mock_new_req = ChargeReqDto {
        order_id: mock_oid.clone(),
        lines: mock_lines,
        method: ut_setup_payment_method_stripe(),
        currency: CurrencyDto::TWD,
    };
    let result = ChargeBuyerModel::try_from((mock_order, mock_new_req));
    assert!(result.is_ok());
    if let Ok(v) = result {
        assert_eq!(v.oid, mock_oid);
        assert_eq!(v.owner, mock_usr_id);
        v.lines
            .into_iter()
            .map(|l| {
                let (actual_pid, actual_amount) = (l.pid, l.amount);
                let BaseProductIdentity {
                    store_id,
                    product_type,
                    product_id,
                } = actual_pid;
                let PayLineAmountModel { unit, total, qty } = actual_amount;
                let expect = match (store_id, product_type, product_id) {
                    (140, ProductType::Item, 1005) => (9u32, 17015i64, 153135i64),
                    (142, ProductType::Item, 1007) => (14, 22090, 309260),
                    (143, ProductType::Package, 1008) => (7, 10000, 70000),
                    _others => (0, 0, 0),
                };
                let expect = (
                    expect.0,
                    Decimal::new(expect.1, 3),
                    Decimal::new(expect.2, 3),
                );
                assert_eq!((qty, unit, total), expect);
            })
            .count();
    }
} // end of fn charge_buyer_convert_ok_2

#[actix_web::test]
async fn charge_buyer_convert_oid_mismatch() {
    let mock_usr_id = 585;
    let reserved_until = Local::now().fixed_offset() + Duration::minutes(2);
    let mock_order = ut_setup_order_replica(mock_usr_id, "lemon".to_string(), reserved_until);
    let mock_new_req = ChargeReqDto {
        order_id: "lime".to_string(),
        lines: Vec::new(),
        method: ut_setup_payment_method_stripe(),
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
} // end of fn charge_buyer_convert_oid_mismatch

#[actix_web::test]
async fn charge_buyer_convert_currency_mismatch() {
    let mock_usr_id = 585;
    let reserved_until = Local::now().fixed_offset() + Duration::minutes(2);
    let mock_order = ut_setup_order_replica(mock_usr_id, "lemon".to_string(), reserved_until);
    let mock_new_req = ChargeReqDto {
        order_id: "lemon".to_string(),
        lines: Vec::new(),
        method: ut_setup_payment_method_stripe(),
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
} // end of fn charge_buyer_convert_currency_mismatch

#[actix_web::test]
async fn charge_buyer_convert_expired() {
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
        (140, ProductType::Item, 1005, 1, "17.015", "17.015"),
        (141, ProductType::Package, 1006, 1, "21", "21"),
        (142, ProductType::Item, 1007, 1, "22.09", "22.09"),
        (143, ProductType::Package, 1008, 1, "10", "10"),
    ]
    .into_iter()
    .map(|d| ChargeAmountOlineDto {
        seller_id: d.0,
        product_id: d.2,
        product_type: d.1,
        quantity: d.3,
        amount: PayAmountDto {
            unit: d.4.to_string(),
            total: d.5.to_string(),
        },
    })
    .collect();
    let mock_new_req = ChargeReqDto {
        order_id: mock_oid.clone(),
        lines: mock_lines,
        method: ut_setup_payment_method_stripe(),
        currency: CurrencyDto::TWD,
    };
    let result = ChargeBuyerModel::try_from((mock_order, mock_new_req));
    assert!(result.is_err());
    if let Err(e) = result {
        if let Some(detail) = e.lines {
            detail
                .into_iter()
                .map(|l| {
                    let expect_pid = [
                        (140u32, ProductType::Item, 1005u64),
                        (141, ProductType::Package, 1006),
                    ];
                    let actual_pid = (l.seller_id, l.product_type, l.product_id);
                    assert!(expect_pid.contains(&actual_pid));
                    assert!(l.expired.unwrap());
                })
                .count();
        } else {
            assert!(false);
        }
    }
} // end of fn charge_buyer_convert_expired

#[actix_web::test]
async fn charge_buyer_convert_qty_exceed_limit() {
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
        (140, ProductType::Item, 1005, 8, "17.015", "136.12"),
        (141, ProductType::Package, 1006, 2, "21", "42"),
        (142, ProductType::Item, 1007, 15, "22.09", "331.35"),
        (143, ProductType::Package, 1008, 7, "10", "70"),
    ]
    .into_iter()
    .map(|d| ChargeAmountOlineDto {
        seller_id: d.0,
        product_id: d.2,
        product_type: d.1,
        quantity: d.3,
        amount: PayAmountDto {
            unit: d.4.to_string(),
            total: d.5.to_string(),
        },
    })
    .collect();
    let mock_new_req = ChargeReqDto {
        order_id: mock_oid.clone(),
        lines: mock_lines,
        method: ut_setup_payment_method_stripe(),
        currency: CurrencyDto::TWD,
    };
    let result = ChargeBuyerModel::try_from((mock_order, mock_new_req));
    assert!(result.is_err());
    if let Err(e) = result {
        if let Some(detail) = e.lines {
            detail
                .into_iter()
                .map(|l| {
                    let actual_pid = (l.seller_id, l.product_type, l.product_id);
                    let expect = match actual_pid {
                        (142u32, ProductType::Item, 1007u64) => (14u16, 15u32),
                        (141, ProductType::Package, 1006) => (0, 2),
                        _others => (0, 0),
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
} // end of  fn charge_buyer_convert_qty_exceed_limit

#[actix_web::test]
async fn charge_buyer_convert_amount_mismatch_1() {
    let (mock_usr_id, mock_oid) = (589, "nova".to_string());
    let reserved_until = Local::now().fixed_offset() + Duration::minutes(2);
    let mock_order = ut_setup_order_replica(mock_usr_id, mock_oid.clone(), reserved_until);
    let mock_lines = [
        (140, ProductType::Item, 1005, 9, "17.01", "159"),
        (141, ProductType::Package, 1006, 2, "21", "42"),
        (142, ProductType::Item, 1007, 15, "22.09", "333"),
        (143, ProductType::Package, 1008, 7, "10", "70"),
    ]
    .into_iter()
    .map(|d| ChargeAmountOlineDto {
        seller_id: d.0,
        product_id: d.2,
        product_type: d.1,
        quantity: d.3,
        amount: PayAmountDto {
            unit: d.4.to_string(),
            total: d.5.to_string(),
        },
    })
    .collect();
    let mock_new_req = ChargeReqDto {
        order_id: mock_oid.clone(),
        lines: mock_lines,
        method: ut_setup_payment_method_stripe(),
        currency: CurrencyDto::TWD,
    };
    let result = ChargeBuyerModel::try_from((mock_order, mock_new_req));
    assert!(result.is_err());
    if let Err(e) = result {
        if let Some(detail) = e.lines {
            detail
                .into_iter()
                .map(|l| {
                    let actual_pid = (l.seller_id, l.product_type, l.product_id);
                    let expect = match actual_pid {
                        (140u32, ProductType::Item, 1005u64) => (9u32, "17.01", "159"),
                        (142, ProductType::Item, 1007) => (15, "22.09", "333"),
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
} // end of fn charge_buyer_convert_amount_mismatch_1

#[actix_web::test]
async fn charge_buyer_convert_amount_mismatch_2() {
    let (mock_usr_id, mock_oid) = (587, "hijurie3k7".to_string());
    let reserved_until = Local::now().fixed_offset() + Duration::minutes(2);
    let mock_order = ut_setup_order_replica(mock_usr_id, mock_oid.clone(), reserved_until);
    let mock_lines = [
        (141, ProductType::Package, 1006, 2, "21", "42"),
        (142, ProductType::Item, 1007, 15, "22.09", "331.35"),
        (143, ProductType::Package, 1008, 7, "11", "77"),
    ]
    .into_iter()
    .map(|d| ChargeAmountOlineDto {
        seller_id: d.0,
        product_id: d.2,
        product_type: d.1,
        quantity: d.3,
        amount: PayAmountDto {
            unit: d.4.to_string(),
            total: d.5.to_string(),
        },
    })
    .collect();
    let mock_new_req = ChargeReqDto {
        order_id: mock_oid.clone(),
        lines: mock_lines,
        method: ut_setup_payment_method_stripe(),
        currency: CurrencyDto::TWD,
    };
    let result = ChargeBuyerModel::try_from((mock_order, mock_new_req));
    assert!(result.is_err());
    if let Err(e) = result {
        if let Some(detail) = e.lines {
            detail
                .into_iter()
                .map(|l| {
                    let actual_pid = (l.seller_id, l.product_type, l.product_id);
                    let expect = match actual_pid {
                        (143u32, ProductType::Package, 1008u64) => {
                            (11u32.to_string(), 77u32.to_string())
                        }
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
} // end of fn charge_buyer_convert_amount_mismatch_2

#[test]
fn charge_token_encode_ok() {
    #[rustfmt::skip]
    [
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
    ]
    .into_iter()
    .map(|(mock_usr_id, time_serial, expect_encoded)| {
        let mock_ctime = DateTime::parse_from_rfc3339(time_serial).unwrap().to_utc();
        let actual = ChargeToken::encode(mock_usr_id, mock_ctime);
        assert_eq!(actual.0, expect_encoded);
    })
    .count();
}
