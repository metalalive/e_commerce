use chrono::{Duration, Local};
use rust_decimal::Decimal;

use ecommerce_common::api::dto::PayAmountDto;
use ecommerce_common::api::rpc::dto::OrderLineReplicaRefundDto;
use ecommerce_common::constant::ProductType;
use ecommerce_common::model::BaseProductIdentity;
use payment::model::{
    OrderRefundModel, PayLineAmountError, RefundErrorParseOline, RefundModelError,
};

#[rustfmt::skip]
fn ut_setup_olines_refund_dto() -> Vec<OrderLineReplicaRefundDto> {
    [
        (37, 982, ProductType::Package, 41, 1671, 8355, 5),
        (50, 982, ProductType::Item, 51, 2222, 15554, 7),
        (37, 999, ProductType::Package, 62, 3333, 36663, 11),
    ]
        .into_iter().map(|d| OrderLineReplicaRefundDto {
            seller_id: d.0, product_id: d.1, product_type: d.2,
            create_time: (Local::now() - Duration::seconds(d.3)).to_rfc3339() ,
            amount: PayAmountDto {
                unit: Decimal::new(d.4, 1).to_string(),
                total: Decimal::new(d.5, 1).to_string(),
            },
            qty: d.6,
        })
        .collect::<Vec<_>>()
}

#[test]
fn convert_from_dto_ok() {
    let mock_oid = "d1e5390dd2".to_string();
    let mock_data = ut_setup_olines_refund_dto();
    let result = OrderRefundModel::try_from((mock_oid, mock_data));
    assert!(result.is_ok());
} // end of fn convert_from_dto_ok

#[test]
fn convert_from_dto_error_amount() {
    let mock_oid = "d1e5390dd2".to_string();
    let mock_data = {
        let mut d = ut_setup_olines_refund_dto();
        let line = d.last_mut().unwrap();
        line.amount.total = "20o8".to_string();
        d
    };
    let result = OrderRefundModel::try_from((mock_oid, mock_data));
    assert!(result.is_err());
    if let Err(mut es) = result {
        assert_eq!(es.len(), 1);
        let e = es.pop().unwrap();
        #[allow(irrefutable_let_patterns)]
        if let RefundModelError::ParseOline { pid, reason } = e {
            let expect_pid = BaseProductIdentity {
                store_id: 37,
                product_type: ProductType::Package,
                product_id: 999,
            };
            assert_eq!(pid, expect_pid);
            if let RefundErrorParseOline::Amount(PayLineAmountError::ParseTotal(orig, _detail)) =
                reason
            {
                assert_eq!(orig.as_str(), "20o8");
            } else {
                assert!(false);
            }
        } else {
            assert!(false);
        }
    }
} // end of fn convert_from_dto_error_amount

#[test]
fn validate_unresolved_reqs_ok() {
} // end of fn validate_unresolved_reqs_ok

#[test]
fn validate_unresolved_reqs_exceed_limit() {
} // end of fn validate_unresolved_reqs_exceed_limit

