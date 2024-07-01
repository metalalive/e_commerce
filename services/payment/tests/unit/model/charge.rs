use chrono::{DateTime, Duration, FixedOffset, Local};
use ecommerce_common::api::dto::{OrderLinePayDto, PayAmountDto, CurrencyDto};
use ecommerce_common::constant::ProductType;
use ecommerce_common::model::BaseProductIdentity;

use payment::api::web::dto::{
    ChargeAmountOlineDto, ChargeReqDto, OrderErrorReason, PaymentMethodReqDto,
    StripeCheckoutSessionReqDto, StripeCheckoutUImodeDto,
};
use payment::model::{
    BuyerPayInState, ChargeBuyerModel, ChargeToken, OLineModelError, OrderLineModelSet,
    PayLineAmountModel,
};

fn ut_setup_order_replica(
    mock_usr_id: u32,
    mock_oid: String,
    reserved_until: DateTime<FixedOffset>,
) -> OrderLineModelSet {
    let mock_lines = [
        (
            140,
            ProductType::Item,
            1005,
            20,
            17,
            340,
            Duration::minutes(6),
        ),
        (
            141,
            ProductType::Package,
            1006,
            11,
            21,
            231,
            Duration::minutes(4),
        ),
        (
            142,
            ProductType::Item,
            1007,
            25,
            22,
            550,
            Duration::minutes(10),
        ),
        (
            143,
            ProductType::Package,
            1008,
            18,
            10,
            180,
            Duration::minutes(9),
        ),
    ]
    .into_iter()
    .map(|d| OrderLinePayDto {
        seller_id: d.0,
        product_id: d.2,
        product_type: d.1,
        reserved_until: (reserved_until + d.6).to_rfc3339(),
        quantity: d.3,
        amount: PayAmountDto {
            unit: d.4,
            total: d.5,
        },
    })
    .collect::<Vec<_>>();
    let result = OrderLineModelSet::try_from((mock_oid, mock_usr_id, mock_lines));
    assert!(result.is_ok());
    result.unwrap()
}

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
    let result = OrderLineModelSet::try_from((mock_oid, mock_usr_id, mock_lines));
    assert!(result.is_err());
    if let Err(e) = result {
        assert_eq!(e.len(), 1);
        let cond = matches!(e[0], OLineModelError::EmptyLine);
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
            unit: d.4,
            total: d.5,
        },
    })
    .collect::<Vec<_>>();
    let result = OrderLineModelSet::try_from((mock_oid, mock_usr_id, mock_lines));
    assert!(result.is_err());
    if let Err(e) = result {
        assert_eq!(e.len(), 1);
        if let OLineModelError::ZeroQuantity(pid) = &e[0] {
            assert_eq!(pid.store_id, 140);
            assert_eq!(pid.product_type, ProductType::Item);
            assert_eq!(pid.product_id, 1005);
        } else {
            assert!(false);
        }
    }
}

#[actix_web::test]
async fn order_replica_convert_line_expired() {
    let (mock_usr_id, mock_oid) = (456, "xyz987".to_string());
    let now = Local::now().fixed_offset();
    let mock_lines = [
        (
            140,
            ProductType::Item,
            1005,
            5,
            17,
            85,
            Duration::minutes(3),
        ),
        (
            141,
            ProductType::Package,
            1006,
            11,
            21,
            231,
            Duration::seconds(19),
        ),
        (
            142,
            ProductType::Item,
            1007,
            10,
            23,
            230,
            Duration::seconds(-2),
        ),
    ]
    .into_iter()
    .map(|d| OrderLinePayDto {
        seller_id: d.0,
        product_id: d.2,
        product_type: d.1,
        reserved_until: (now + d.6).to_rfc3339(),
        quantity: d.3,
        amount: PayAmountDto {
            unit: d.4,
            total: d.5,
        },
    })
    .collect::<Vec<_>>();
    let result = OrderLineModelSet::try_from((mock_oid, mock_usr_id, mock_lines));
    assert!(result.is_err());
    if let Err(e) = result {
        assert_eq!(e.len(), 1);
        if let Some(OLineModelError::RsvExpired(pid)) = e.first() {
            assert_eq!(pid.store_id, 142);
            assert_eq!(pid.product_type, ProductType::Item);
            assert_eq!(pid.product_id, 1007);
        } else {
            assert!(false);
        }
    }
}

#[actix_web::test]
async fn order_replica_convert_amount_mismatch() {
    let (mock_usr_id, mock_oid) = (456, "xyz987".to_string());
    let reserved_until = Local::now().fixed_offset() + Duration::minutes(3);
    let mock_lines = [
        (140, ProductType::Item, 1005, 5, 17, 85),
        (141, ProductType::Package, 1006, 11, 24, 261),
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
            unit: d.4,
            total: d.5,
        },
    })
    .collect::<Vec<_>>();
    let result = OrderLineModelSet::try_from((mock_oid, mock_usr_id, mock_lines));
    assert!(result.is_err());
    if let Err(e) = result {
        assert_eq!(e.len(), 1);
        if let OLineModelError::AmountMismatch(pid, amount, qty) = &e[0] {
            assert_eq!(pid.store_id, 141);
            assert_eq!(pid.product_type, ProductType::Package);
            assert_eq!(pid.product_id, 1006);
            assert_eq!(amount.unit, 24);
            assert_eq!(amount.total, 261);
            assert_eq!(qty, &11u32);
        } else {
            assert!(false);
        }
    }
} // end of fn order_replica_convert_amount_mismatch

#[actix_web::test]
async fn charge_buyer_convert_ok_1() {
    let (mock_usr_id, mock_oid) = (582, "phidix".to_string());
    let reserved_until = Local::now().fixed_offset() + Duration::minutes(2);
    let mock_order = ut_setup_order_replica(mock_usr_id, mock_oid.clone(), reserved_until);
    let mock_lines = [
        (140, ProductType::Item, 1005, 6, 17, 102),
        (141, ProductType::Package, 1006, 1, 21, 21),
        (143, ProductType::Package, 1008, 4, 10, 40),
    ]
    .into_iter()
    .map(|d| ChargeAmountOlineDto {
        seller_id: d.0,
        product_id: d.2,
        product_type: d.1,
        quantity: d.3,
        amount: PayAmountDto {
            unit: d.4,
            total: d.5,
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
                    (140, ProductType::Item, 1005) => (6, 17, 102),
                    (141, ProductType::Package, 1006) => (1, 21, 21),
                    (143, ProductType::Package, 1008) => (4, 10, 40),
                    _others => (0, 0, 0),
                };
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
            v.paid_total.total = v.paid_total.unit * v.paid_total.qty;
        })
        .count();
    let mock_lines = [
        (140, ProductType::Item, 1005, 9, 17, 153),
        (142, ProductType::Item, 1007, 14, 22, 308),
        (143, ProductType::Package, 1008, 7, 10, 70),
    ]
    .into_iter()
    .map(|d| ChargeAmountOlineDto {
        seller_id: d.0,
        product_id: d.2,
        product_type: d.1,
        quantity: d.3,
        amount: PayAmountDto {
            unit: d.4,
            total: d.5,
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
                    (140, ProductType::Item, 1005) => (9, 17, 153),
                    (142, ProductType::Item, 1007) => (14, 22, 308),
                    (143, ProductType::Package, 1008) => (7, 10, 70),
                    _others => (0, 0, 0),
                };
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
}

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
        (140, ProductType::Item, 1005, 1, 17, 17),
        (141, ProductType::Package, 1006, 1, 21, 21),
        (142, ProductType::Item, 1007, 1, 22, 22),
        (143, ProductType::Package, 1008, 1, 10, 10),
    ]
    .into_iter()
    .map(|d| ChargeAmountOlineDto {
        seller_id: d.0,
        product_id: d.2,
        product_type: d.1,
        quantity: d.3,
        amount: PayAmountDto {
            unit: d.4,
            total: d.5,
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
            v.paid_total.total = v.paid_total.unit * v.paid_total.qty;
        })
        .count();
    let mock_lines = [
        (140, ProductType::Item, 1005, 9, 17, 153),
        (141, ProductType::Package, 1006, 2, 21, 42),
        (142, ProductType::Item, 1007, 15, 22, 330),
        (143, ProductType::Package, 1008, 7, 10, 70),
    ]
    .into_iter()
    .map(|d| ChargeAmountOlineDto {
        seller_id: d.0,
        product_id: d.2,
        product_type: d.1,
        quantity: d.3,
        amount: PayAmountDto {
            unit: d.4,
            total: d.5,
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
                    assert!(l.amount.is_none());
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
        (140, ProductType::Item, 1005, 9, 17, 159),
        (141, ProductType::Package, 1006, 2, 21, 42),
        (142, ProductType::Item, 1007, 15, 22, 333),
        (143, ProductType::Package, 1008, 7, 10, 70),
    ]
    .into_iter()
    .map(|d| ChargeAmountOlineDto {
        seller_id: d.0,
        product_id: d.2,
        product_type: d.1,
        quantity: d.3,
        amount: PayAmountDto {
            unit: d.4,
            total: d.5,
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
                        (140u32, ProductType::Item, 1005u64) => (9u32, 17u32, 159u32),
                        (142, ProductType::Item, 1007) => (15, 22, 333),
                        _others => (0, 0, 0),
                    };
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
        (141, ProductType::Package, 1006, 2, 21, 42),
        (142, ProductType::Item, 1007, 15, 22, 330),
        (143, ProductType::Package, 1008, 7, 11, 77),
    ]
    .into_iter()
    .map(|d| ChargeAmountOlineDto {
        seller_id: d.0,
        product_id: d.2,
        product_type: d.1,
        quantity: d.3,
        amount: PayAmountDto {
            unit: d.4,
            total: d.5,
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
                        (143u32, ProductType::Package, 1008u64) => (11u32, 77u32),
                        _others => (0, 0),
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
    [
        (
            8374u32,
            "1998-10-31T18:38:25+00:00",
            [
                0x0u8,
                0x0,
                0x20,
                0xb6,
                0x1f,
                0x38 | 0x2,
                0x80 | 0x3e | 0x1,
                0x20 | 0x9,
                0x80 | 0x19,
            ],
        ),
        (
            8010095,
            "2012-04-24T23:01:30+00:00",
            [
                0x0,
                0x7a,
                0x39,
                0x6f,
                0x1f,
                0x70 | 0x1,
                0x0 | 0x30 | 0x01,
                0x70 | 0x0,
                0x40 | 0x1e,
            ],
        ),
        (
            100290278,
            "2019-01-17T00:59:35+00:00",
            [
                0x05,
                0xfa,
                0x4e,
                0xe6,
                0x1f,
                0x8c | 0x0,
                0x40 | 0x22 | 0x0,
                0x0 | 0xe,
                0xc0 | 0x23,
            ],
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
