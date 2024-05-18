use chrono::{Local, Duration};
use ecommerce_common::api::dto::{OrderLinePayDto, PayAmountDto};
use ecommerce_common::constant::ProductType;

use payment::model::{OrderLineModelSet, OLineModelError};


#[actix_web::test]
async fn order_replica_convert_ok() {
    let (mock_usr_id, mock_oid) = (456, "xyz987".to_string());
    let reserved_until = Local::now().fixed_offset() + Duration::minutes(3);
    let mock_lines = [
        (140, ProductType::Item, 1005, 5, 17, 85),
        (141, ProductType::Package, 1006, 11, 21, 231),
        (142, ProductType::Item, 1007, 10, 23, 230),
    ].into_iter().map(|d| OrderLinePayDto {
        seller_id: d.0, product_id: d.2, product_type: d.1,
        reserved_until: reserved_until.to_rfc3339(),
        quantity: d.3, amount: PayAmountDto { unit: d.4, total: d.5 }
    }).collect::<Vec<_>>();
    let result = OrderLineModelSet::try_from(
        (mock_oid, mock_usr_id, mock_lines)
    );
    assert!(result.is_ok());
    if let Ok(v) = result {
        assert_eq!(v.lines.len(), 3);
    }
}

#[actix_web::test]
async fn order_replica_convert_empty_line() {
    let (mock_usr_id, mock_oid) = (456, "xyz987".to_string());
    let mock_lines = Vec::new();
    let result = OrderLineModelSet::try_from(
        (mock_oid, mock_usr_id, mock_lines)
    );
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
    ].into_iter().map(|d| OrderLinePayDto {
        seller_id: d.0, product_id: d.2, product_type: d.1,
        reserved_until: reserved_until.to_rfc3339(),
        quantity: d.3, amount: PayAmountDto { unit: d.4, total: d.5 }
    }).collect::<Vec<_>>();
    let result = OrderLineModelSet::try_from(
        (mock_oid, mock_usr_id, mock_lines)
    );
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
        (140, ProductType::Item, 1005, 5, 17, 85, Duration::minutes(3)),
        (141, ProductType::Package, 1006, 11, 21, 231, Duration::seconds(19)),
        (142, ProductType::Item, 1007, 10, 23, 230, Duration::seconds(-2)),
    ].into_iter().map(|d| OrderLinePayDto {
        seller_id: d.0, product_id: d.2, product_type: d.1,
        reserved_until: (now + d.6).to_rfc3339(),
        quantity: d.3, amount: PayAmountDto { unit: d.4, total: d.5 }
    }).collect::<Vec<_>>();
    let result = OrderLineModelSet::try_from(
        (mock_oid, mock_usr_id, mock_lines)
    );
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
    ].into_iter().map(|d| OrderLinePayDto {
        seller_id: d.0, product_id: d.2, product_type: d.1,
        reserved_until: reserved_until.to_rfc3339(),
        quantity: d.3, amount: PayAmountDto { unit: d.4, total: d.5 }
    }).collect::<Vec<_>>();
    let result = OrderLineModelSet::try_from(
        (mock_oid, mock_usr_id, mock_lines)
    );
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
