use chrono::{DateTime, Duration, FixedOffset, Local};
use rust_decimal::Decimal;

use ecommerce_common::api::dto::{
    CurrencyDto, CurrencySnapshotDto, OrderCurrencySnapshotDto, OrderLinePayDto,
    OrderSellerCurrencyDto, PayAmountDto,
};
use ecommerce_common::model::BaseProductIdentity;

use payment::model::{OrderLineModelSet, OrderModelError, PayLineAmountError};

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
pub(super) fn ut_setup_order_replica(
    mock_buyer_id: u32,
    mock_oid: String,
    reserved_until: DateTime<FixedOffset>,
) -> OrderLineModelSet {
    let mock_lines = [
        (140, 1005, 0, 20, "17.15", "343", Duration::minutes(6)),
        (141, 1006, 0, 11, "21.0", "231.0", Duration::minutes(4)),
        (142, 1007, 0, 25, "22.09", "552.25", Duration::minutes(10)),
        (142, 1007, 1, 15, "22.56", "338.40", Duration::minutes(10)),
        (143, 1008, 0, 18, "10.0", "180.0", Duration::minutes(9)),
        (143, 1008, 1, 3,  "10.29", "30.87", Duration::minutes(9)),
        (143, 1008, 2, 4,  "10.60", "42.40", Duration::minutes(9)),
    ]
    .into_iter()
    .map(|d| OrderLinePayDto {
        seller_id: d.0, product_id: d.1, attr_set_seq: d.2, quantity: d.3,
        amount: PayAmountDto {unit: d.4.to_string(), total: d.5.to_string()},
        reserved_until: (reserved_until + d.6).to_rfc3339(),
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
    let args = (mock_oid, mock_buyer_id, mock_lines, mock_currency_snapshot);
    let result = OrderLineModelSet::try_from(args);
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

#[test]
fn convert_ok() {
    let (mock_usr_id, mock_oid) = (456, "xyz987".to_string());
    let reserved_until = Local::now().fixed_offset() + Duration::minutes(3);
    let out = ut_setup_order_replica(mock_usr_id, mock_oid, reserved_until);
    assert_eq!(out.lines.len(), 7);
}

#[test]
fn convert_empty_line() {
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

#[test]
fn convert_qty_zero() {
    let (mock_usr_id, mock_oid) = (456, "xyz987".to_string());
    let reserved_until = Local::now().fixed_offset() + Duration::minutes(3);
    let mock_lines = [
        (140, 1005, 1, 0, "17", "85"),
        (141, 1006, 0, 11, "21", "231"),
        (142, 1007, 0, 10, "23", "230"),
    ]
    .into_iter()
    .map(|d| OrderLinePayDto {
        seller_id: d.0,
        product_id: d.1,
        attr_set_seq: d.2,
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
        if let OrderModelError::ZeroQuantity(pid, attrseq) = &e[0] {
            assert_eq!(pid.store_id, 140);
            assert_eq!(pid.product_id, 1005);
            assert_eq!(*attrseq, 1u16);
        } else {
            assert!(false);
        }
    }
} // end of fn convert_qty_zero

#[test]
fn convert_line_expired() {
    let (mock_usr_id, mock_oid) = (456, "xyz987".to_string());
    let now = Local::now().fixed_offset();
    let mock_lines = [
        (140, 1005, 5, "17", "85", 180),
        (141, 1006, 11, "21", "231", 19),
        (142, 1007, 10, "23", "230", -2),
    ]
    .into_iter()
    .map(|d| OrderLinePayDto {
        seller_id: d.0,
        product_id: d.1,
        attr_set_seq: 0,
        quantity: d.2,
        reserved_until: (now + Duration::seconds(d.5)).to_rfc3339(),
        amount: PayAmountDto {
            unit: d.3.to_string(),
            total: d.4.to_string(),
        },
    })
    .collect::<Vec<_>>();
    let mock_currency_snapshot = ut_default_currency_snapshot_dto(vec![140, 141, 142]);
    let result =
        OrderLineModelSet::try_from((mock_oid, mock_usr_id, mock_lines, mock_currency_snapshot));
    assert!(result.is_err());
    if let Err(e) = result {
        assert_eq!(e.len(), 1);
        if let Some(OrderModelError::RsvExpired(pid, attrseq)) = e.first() {
            assert_eq!(pid.store_id, 142);
            assert_eq!(pid.product_id, 1007);
            assert_eq!(attrseq, &0u16);
        } else {
            assert!(false);
        }
    }
} // end of fn convert_line_expired

#[rustfmt::skip]
#[test]
fn convert_missing_currency_1() {
    let (mock_buyer_id, mock_oid) = (149, "9a800f71b".to_string());
    let now = Local::now().fixed_offset();
    let mock_lines = [
        (141, 1006, 11, "21.0", "231.0", Duration::minutes(4)),
        (142, 1007, 25, "22.0", "550.0", Duration::minutes(10)),
        (143, 1008, 18, "10.0", "180.0", Duration::minutes(9)),
    ]
    .into_iter()
    .map(|d| OrderLinePayDto {
        seller_id: d.0, product_id: d.1, attr_set_seq: 0, quantity: d.2,
        reserved_until: (now + d.5).to_rfc3339(),
        amount: PayAmountDto {unit: d.3.to_string(), total: d.4.to_string()},
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
} // end of fn convert_missing_currency_1

#[test]
fn convert_missing_currency_2() {
    let (mock_buyer_id, mock_oid) = (158, "9a800f71b".to_string());
    let now = Local::now().fixed_offset();
    let mock_lines = [
        (143, 1006, 11, "21.0", "231.0", 4),
        (144, 1007, 25, "22.0", "550.0", 10),
        (145, 1008, 18, "10.0", "180.0", 9),
    ]
    .into_iter()
    .map(|d| OrderLinePayDto {
        seller_id: d.0,
        product_id: d.1,
        attr_set_seq: 0,
        quantity: d.2,
        reserved_until: (now + Duration::minutes(d.5)).to_rfc3339(),
        amount: PayAmountDto {
            unit: d.3.to_string(),
            total: d.4.to_string(),
        },
    })
    .collect::<Vec<_>>();
    let mock_currency_snapshot = ut_default_currency_snapshot_dto(vec![143, 145]);
    let result =
        OrderLineModelSet::try_from((mock_oid, mock_buyer_id, mock_lines, mock_currency_snapshot));
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
} // end of fn  fn convert_missing_currency_2()

#[rustfmt::skip]
#[test]
fn convert_corrupted_currency_rate() {
    let (mock_buyer_id, mock_oid) = (558, "9a800f71b".to_string());
    let now = Local::now().fixed_offset();
    let mock_lines = [
        (146, 1007, 25, "22.0", "550.0", Duration::minutes(10)),
        (147, 1008, 18, "10.0", "180.0", Duration::minutes(9)),
    ]
    .into_iter()
    .map(|d| OrderLinePayDto {
        seller_id: d.0, product_id: d.1, attr_set_seq: 0, quantity: d.2,
        reserved_until: (now + d.5).to_rfc3339(),
        amount: PayAmountDto {unit: d.3.to_string(), total: d.4.to_string()},
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
} // end of fn convert_corrupted_currency_rate

#[rustfmt::skip]
#[test]
fn convert_invalid_amount() {
    let (mock_usr_id, mock_oid) = (456, "xyz987".to_string());
    let reserved_until = Local::now().fixed_offset() + Duration::minutes(3);
    let mock_lines = [
        (140, 1005, 0, 5, "L7", "85"),
        (140, 1005, 1, 11, "24", "261"),
        (140, 1007, 0, 10, "23", "230s"),
        (143, 1011, 0, u32::MAX, "1230045600789001230045600", "2900"),
        (143, 1027, 0, 4, "200.013", "800.052"),
    ]
    .into_iter()
    .map(|d| OrderLinePayDto {
        seller_id: d.0, product_id: d.1, attr_set_seq: d.2, quantity: d.3,
        reserved_until: reserved_until.to_rfc3339(),
        amount: PayAmountDto {unit: d.4.to_string(), total: d.5.to_string()},
    })
    .collect::<Vec<_>>();
    let mock_currency_snapshot = ut_default_currency_snapshot_dto(vec![140, 141, 142, 143, 144]);
    let result =
        OrderLineModelSet::try_from((mock_oid, mock_usr_id, mock_lines, mock_currency_snapshot));
    assert!(result.is_err());
    if let Err(es) = result {
        assert_eq!(es.len(), 5);
        es.into_iter().map(|e| {
            print!("[DEBUG] error: {:?}\n", e);
            if let OrderModelError::InvalidAmount(pid, attrseq, pe) = e {
                let BaseProductIdentity { store_id, product_id } = pid;
                match (store_id, product_id, attrseq) {
                    (140, 1005, 1) =>
                        if let PayLineAmountError::Mismatch(amount, qty) = pe {
                            assert_eq!(amount.unit.as_str(), "24");
                            assert_eq!(amount.total.as_str(), "261");
                            assert_eq!(qty, 11u32);
                        } else { assert!(false); },
                    (140, 1005, 0) =>
                        if let PayLineAmountError::ParseUnit(value, _reason) = pe {
                            assert_eq!(value.as_str(), "L7");
                        } else { assert!(false); },
                    (140, 1007, 0) =>
                        if let PayLineAmountError::ParseTotal(value, _reason) = pe {
                            assert_eq!(value.as_str(), "230s");
                        } else { assert!(false); },
                    (143, 1011, 0) =>
                        if let PayLineAmountError::Overflow(amt_unit, qty) = pe {
                            assert_eq!(qty, u32::MAX);
                            assert_eq!(amt_unit.as_str(), "1230045600789001230045600");
                        } else { assert!(false); },
                    (143, 1027, 0) =>
                        if let PayLineAmountError::PrecisionUnit(amt_unit, scale) = pe {
                            assert_eq!(amt_unit.as_str(), "200.013");
                            assert_eq!(scale, (2, 3));
                        } else { assert!(false); },
                    _others => { assert!(false); },
                }
            } else { assert!(false); }
        }).count();
    }
} // end of fn convert_invalid_amount
