use std::collections::HashMap;

use chrono::{DateTime, Duration, Local, Utc};
use rust_decimal::Decimal;

use ecommerce_common::api::dto::{CurrencyDto, PayAmountDto};
use ecommerce_common::api::rpc::dto::OrderLineReplicaRefundDto;
use ecommerce_common::constant::ProductType;
use ecommerce_common::model::BaseProductIdentity;
use payment::api::web::dto::{
    RefundCompletionOlineReqDto, RefundCompletionReqDto, RefundLineApprovalDto,
    RefundRejectReasonDto,
};
use payment::model::{
    BuyerPayInState, Charge3partyModel, ChargeBuyerModel, OrderCurrencySnapshot, OrderRefundModel,
    PayLineAmountError, RefundErrorParseOline, RefundModelError, RefundReqResolutionModel,
    StripeCheckoutPaymentStatusModel,
};

use super::{ut_default_charge_method_stripe, ut_setup_buyer_charge, UTestChargeLineRawData};

#[rustfmt::skip]
fn ut_setup_olines_refund_dto(time_base: DateTime<Utc>) -> Vec<OrderLineReplicaRefundDto> {
    [
        (37, 982, ProductType::Package, 41, 1671, 8355, 5),
        (37, 982, ProductType::Item,    87, 1650, 16500, 10),
        (37, 982, ProductType::Package, 87, 1671, 33420, 20),
        (37, 982, ProductType::Package, 113, 1671, 5013, 3),
        (50, 982, ProductType::Item, 51, 2222, 15554, 7),
        (50, 591, ProductType::Package, 54, 805, 7245, 9),
        (37, 999, ProductType::Item, 144, 1900, 9500, 5),
        (37, 999, ProductType::Package, 62, 3333, 36663, 11),
    ]
        .into_iter().map(|d| OrderLineReplicaRefundDto {
            seller_id: d.0, product_id: d.1, product_type: d.2,
            create_time: (time_base - Duration::minutes(d.3)).to_rfc3339() ,
            amount: PayAmountDto {
                unit: Decimal::new(d.4, 1).to_string(),
                total: Decimal::new(d.5, 1).to_string(),
            },
            qty: d.6,
        })
        .collect::<Vec<_>>()
}

#[rustfmt::skip]
fn ut_setup_refund_cmplt_dto(
    time_base: DateTime<Utc>,
    raw: Vec<(u64, ProductType, i64, i64, u32, u32, u32)>
) -> RefundCompletionReqDto {
    let lines = raw.into_iter()
        .map(|d| {
            let time_issued = time_base - Duration::minutes(d.2);
            let approval = RefundLineApprovalDto {
                amount_total: Decimal::new(d.3, 1).to_string(),
                quantity: d.4,
            };
            let reject = HashMap::from([
                (RefundRejectReasonDto::Damaged, d.5),
                (RefundRejectReasonDto::Fraudulent, d.6),
            ]);
            RefundCompletionOlineReqDto {
                product_id: d.0, product_type: d.1, time_issued,
                reject, approval
            }
        }).collect::<Vec<_>>() ;
    RefundCompletionReqDto { lines }
}

#[test]
fn convert_from_dto_ok() {
    let mock_oid = "d1e5390dd2".to_string();
    let mock_data = ut_setup_olines_refund_dto(Local::now().to_utc());
    let result = OrderRefundModel::try_from((mock_oid, mock_data));
    assert!(result.is_ok());
} // end of fn convert_from_dto_ok

#[test]
fn convert_from_dto_error_amount() {
    let mock_oid = "d1e5390dd2".to_string();
    let mock_data = {
        let mut d = ut_setup_olines_refund_dto(Local::now().to_utc());
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

#[rustfmt::skip]
#[test]
fn validate_unresolved_reqs_ok() {
    let time_now = Local::now().to_utc();
    let mock_oid = "d1e5390dd2".to_string();
    let mock_data = ut_setup_olines_refund_dto(time_now);
    let rfd_req = OrderRefundModel::try_from((mock_oid, mock_data)).unwrap();
    let mock_merchant_id = 37u32;
    let mock_cmplt_req = {
        let lines = vec![
            (982, ProductType::Package, 41, 8355, 5, 0, 0),
            (982, ProductType::Item,    87, 1650, 1, 2, 0),
            (982, ProductType::Package, 87, 16710, 10, 3, 4),
            (999, ProductType::Item, 144, 7600, 4, 0, 1),
            (999, ProductType::Package, 62, 36663, 11, 0, 0),
        ];
        ut_setup_refund_cmplt_dto(time_now, lines)
    };
    let result = rfd_req.validate(mock_merchant_id, &mock_cmplt_req);
    assert!(result.is_ok());
    if let Ok(vs) = result {
        assert_eq!(vs.len(), 5);
        vs.into_iter().map(|d| {
            let actual = (d.3, d.4);
            let expect = match (d.0, d.1, (time_now - d.2).num_minutes()) {
                (ProductType::Package, 982, 41) => (0u32, Decimal::ZERO),
                (ProductType::Item,  982,   87) => (7, Decimal::new(11550, 1)),
                (ProductType::Package, 982, 87) => (3, Decimal::new(5013, 1)),
                (ProductType::Item, 999, 144) => (0, Decimal::ZERO),
                (ProductType::Package, 999, 62) => (0, Decimal::ZERO),
                _others => (9999, Decimal::NEGATIVE_ONE),
            };
            assert_eq!(actual, expect);
        }).count();
    }
} // end of fn validate_unresolved_reqs_ok

#[rustfmt::skip]
#[test]
fn validate_unresolved_reqs_exceed_limit() {
    let time_now = Local::now().to_utc();
    let mock_oid = "d1e5390dd2".to_string();
    let mock_data = ut_setup_olines_refund_dto(time_now);
    let rfd_req = OrderRefundModel::try_from((mock_oid, mock_data)).unwrap();
    let mock_merchant_id = 37u32;
    let mock_cmplt_req = {
        let lines = vec![
            (982, ProductType::Package, 87, 1671, 1, 5, 6),
            (999, ProductType::Item, 144, 7600, 4, 1, 1),
            // assume the total amount in the request is corrupted 
            (999, ProductType::Package, 62, 39999, 11, 0, 0),
        ];
        ut_setup_refund_cmplt_dto(time_now, lines)
    };
    let result = rfd_req.validate(mock_merchant_id, &mock_cmplt_req);
    assert!(result.is_err());
    if let Err(es) = result {
        assert_eq!(es.len(), 2);
        es.into_iter().map(|e| match e {
            RefundModelError::QtyInsufficient { pid, num_avail, num_req } => {
                assert_eq!(pid.store_id, mock_merchant_id);
                assert_eq!(pid.product_id, 999u64);
                assert_eq!(pid.product_type, ProductType::Item);
                assert!(num_avail < num_req);
            }
            RefundModelError::AmountInsufficient { pid, num_avail, num_req } => {
                assert_eq!(pid.store_id, mock_merchant_id);
                assert_eq!(pid.product_id, 999u64);
                assert_eq!(pid.product_type, ProductType::Package);
                assert!(num_avail < num_req);
            }
            _others => {
                assert!(false);
            }
        }).count();
    }
} // end of fn validate_unresolved_reqs_exceed_limit

fn ut_setup_buyer_charge_inner(
    time_base: DateTime<Utc>,
    merchant_id: u32,
    buyer_usr_id: u32,
    charge_dlines: Vec<UTestChargeLineRawData>,
) -> ChargeBuyerModel {
    let mock_oid = "d1e5390dd2".to_string();
    let charge_ctime = time_base - Duration::minutes(86);
    let paymethod = {
        let mut mthd = ut_default_charge_method_stripe(&charge_ctime);
        if let Charge3partyModel::Stripe(s) = &mut mthd {
            s.payment_state = StripeCheckoutPaymentStatusModel::paid;
        }
        mthd
    };
    let currency_snapshot = {
        let iter = [
            (buyer_usr_id, CurrencyDto::TWD, (3185i64, 2u32)),
            (merchant_id, CurrencyDto::IDR, (123451, 1)),
        ]
        .map(|(usr_id, label, ratescalar)| {
            let rate = Decimal::new(ratescalar.0, ratescalar.1);
            let obj = OrderCurrencySnapshot { label, rate };
            (usr_id, obj)
        });
        HashMap::from_iter(iter)
    };
    ut_setup_buyer_charge(
        buyer_usr_id,
        charge_ctime,
        mock_oid.clone(),
        BuyerPayInState::OrderAppSynced(time_base),
        paymethod,
        charge_dlines,
        currency_snapshot,
    )
} // end of fn ut_setup_buyer_charge_inner

#[rustfmt::skip]
#[test]
fn create_resolution_model_ok() {
    let mock_buyer_id = 9802u32;
    let mock_merchant_id = 37u32;
    let time_now = Local::now().to_utc();
    let mock_cmplt_req = {
        let lines = vec![
            (982, ProductType::Package, 41, 8355, 5, 0, 0),
            (982, ProductType::Package, 87, 16710, 10, 1, 2),
            (982, ProductType::Package, 129, 1671, 1, 0, 0),
            (982, ProductType::Package, 138, 0, 0, 1, 0),
            (982, ProductType::Package, 113, 1671, 1, 0, 1),
            (983, ProductType::Item,    87, 1650, 1, 2, 0),
            (983, ProductType::Item,   106, 3300, 2, 0, 5),
            (985, ProductType::Package, 35, 2500, 1, 0, 10),
            (999, ProductType::Item,  43, 7600, 4, 1, 1),
            (999, ProductType::Item,  49, 3800, 2, 0, 0),
            (999, ProductType::Item, 144, 0, 0, 7, 0),
        ];
        ut_setup_refund_cmplt_dto(time_now, lines)
    };
    let charge_rawlines = vec![
        (mock_merchant_id, ProductType::Package, 982u64, (1671i64, 1u32), (20052i64, 1u32), 12u32,
         (0i64, 0u32), (0i64, 0u32), 0u32),
        (mock_merchant_id, ProductType::Item, 983, (1650, 1), (29700, 1), 18, (0, 0), (0, 0), 0),
        (mock_merchant_id, ProductType::Item, 999, (1900, 1), (9500, 1), 5, (1900, 1), (3800, 1), 2),
        (mock_merchant_id, ProductType::Package, 603, (990, 1), (2990, 1), 3, (0, 0), (0, 0), 0),
    ];
    let mock_charge_m = ut_setup_buyer_charge_inner(
        time_now, mock_merchant_id, mock_buyer_id, charge_rawlines
    );
    let arg = (mock_merchant_id, &mock_charge_m ,&mock_cmplt_req);
    let result = RefundReqResolutionModel::try_from(arg);
    assert!(result.is_ok());
    if let Ok(v) = result {
        [
            (982, ProductType::Package, 41, 8355, 5, 0, 0),
            (982, ProductType::Package, 87, 11697, 7, 1, 2),
            (982, ProductType::Package, 138, 0, 0, 1, 0),
            (982, ProductType::Package, 113, 0, 0, 0, 1),
            (983, ProductType::Item,    87, 1650, 1, 2, 0),
            (983, ProductType::Item,   106, 3300, 2, 0, 5),
            (999, ProductType::Item,    43, 5700, 3, 1, 1),
            (999, ProductType::Item,   144, 0, 0, 7, 0),
        ].into_iter().map(|d| {
            let t_req = time_now - Duration::minutes(d.2);
            let (reject_rslv, amt_rslv) =
                v.get_status(mock_merchant_id, d.1, d.0, t_req).unwrap();
            assert_eq!(amt_rslv.curr_round().total, Decimal::new(d.3, 1));
            assert_eq!(amt_rslv.curr_round().qty, d.4);
            // assert_eq!(amt_rslv.accumulated().total, Decimal::ZERO);
            // assert_eq!(amt_rslv.accumulated().qty, 0);
            let num_damage = *reject_rslv.inner_map()
                .get(&RefundRejectReasonDto::Damaged).unwrap();
            assert_eq!(num_damage, d.5);
            let num_fraud = *reject_rslv.inner_map()
                .get(&RefundRejectReasonDto::Fraudulent).unwrap();
            assert_eq!(num_fraud, d.6);
        }).count();
        [
            (982, ProductType::Package, 129),
            (985, ProductType::Package, 35),
            (999, ProductType::Item,  49),
        ].into_iter().map(|d| {
            let t_req = time_now - Duration::minutes(d.2);
            let result = v.get_status(mock_merchant_id, d.1, d.0, t_req);
            assert!(result.is_none());
        }).count();
    }
} // end of fn create_resolution_model_ok

#[rustfmt::skip]
#[test]
fn create_resolution_model_err() {
    let mock_buyer_id = 9802u32;
    let mock_merchant_ids = [37u32, 64];
    let time_now = Local::now().to_utc();
    let mock_cmplt_req = {
        let lines = vec![
            (982, ProductType::Package, 41, 8355, 5, 0, 0),
            (982, ProductType::Package, 87, 16710, 10, 1, 2),
        ];
        ut_setup_refund_cmplt_dto(time_now, lines)
    };
    let charge_rawlines = vec![
        (
            mock_merchant_ids[0], ProductType::Package, 982u64,
            (1671i64, 1u32), (20052i64, 1u32), 12u32,
            (0i64, 0u32), (0i64, 0u32), 0u32,
        ),
        (mock_merchant_ids[0], ProductType::Item, 983, (1650, 1), (29700, 1), 18, (0, 0), (0, 0), 0),
    ];
    let mock_charge_m = ut_setup_buyer_charge_inner(
        time_now,
        mock_merchant_ids[0],
        mock_buyer_id,
        charge_rawlines,
    ); // merchant ID mismatch leads to the conversion error
    let arg = (mock_merchant_ids[1], &mock_charge_m, &mock_cmplt_req);
    let result = RefundReqResolutionModel::try_from(arg);
    if let Err(RefundModelError::MissingCurrency(label, actor_id)) = result {
        assert_eq!(label.as_str(), "merchant-id");
        assert_eq!(actor_id, mock_merchant_ids[1]);
    } else {
        assert!(false);
    }
} // end of fn create_resolution_model_err

#[rustfmt::skip]
#[test]
fn update_refund_req_ok() {
    let mock_buyer_id = 9802u32;
    let mock_merchant_id = 37u32;
    let mock_oid = "d1e5390dd2".to_string();
    let time_now = Local::now().to_utc();
    let mock_data = ut_setup_olines_refund_dto(time_now);
    let mut refund_req_m = OrderRefundModel::try_from((mock_oid, mock_data)).unwrap();
    
    let mock_cmplt_req = {
        let lines = vec![
            (982, ProductType::Package, 41, 8355, 5, 0, 0),
            (982, ProductType::Package, 113, 1671, 1, 0, 1),
            (999, ProductType::Item, 144, 0, 0, 3, 0),
            (999, ProductType::Package,  62, 3333, 1, 0, 0),
        ];
        ut_setup_refund_cmplt_dto(time_now, lines)
    };
    let charge_rawlines = vec![
        (mock_merchant_id, ProductType::Package, 982u64, (1671i64, 1u32), (20052i64, 1u32), 12u32,
         (0i64, 0u32), (0i64, 0u32), 0u32),
        (mock_merchant_id, ProductType::Item, 983, (1650, 1), (29700, 1), 18, (0, 0), (0, 0), 0),
        (mock_merchant_id, ProductType::Item, 999, (1900, 1), (9500, 1), 5, (0, 0), (0, 0), 0),
        (mock_merchant_id, ProductType::Package, 603, (990, 1), (2990, 1), 3, (0, 0), (0, 0), 0),
        (mock_merchant_id, ProductType::Package, 999, (3333, 1), (36663, 1), 11, (0, 0), (0, 0), 0),
    ];
    let mock_charge_m = ut_setup_buyer_charge_inner(
        time_now, mock_merchant_id, mock_buyer_id, charge_rawlines
    );
    let arg = (mock_merchant_id, &mock_charge_m ,&mock_cmplt_req);
    let resolve_m = RefundReqResolutionModel::try_from(arg).unwrap();
    let actual_num_updated = refund_req_m.update(&resolve_m);
    assert_eq!(actual_num_updated, mock_cmplt_req.lines.len());
 
    // --- validate with rest of lines requested but not resolved yet
    let mock_cmplt_req = {
        let lines = vec![
            (982, ProductType::Item,    87, 16500, 10, 0, 0),
            (982, ProductType::Package, 87, 33420, 20, 0, 0),
            (982, ProductType::Package, 113, 1671, 1, 0, 0),
            (999, ProductType::Item, 144, 3800, 2, 0, 0),
            (999, ProductType::Package, 62, 33330, 10, 0, 0),
        ];
        ut_setup_refund_cmplt_dto(time_now, lines)
    };
    let result = refund_req_m.validate(mock_merchant_id, &mock_cmplt_req);
    assert!(result.is_ok());
    let mock_cmplt_req = {
        let lines = vec![(982, ProductType::Package, 113, 1671, 1, 1, 0)];
        ut_setup_refund_cmplt_dto(time_now, lines)
    };
    let result = refund_req_m.validate(mock_merchant_id, &mock_cmplt_req);
    assert!(result.is_err());
    let mock_cmplt_req = {
        let lines = vec![(982, ProductType::Package, 113, 3342, 2, 0, 0)];
        ut_setup_refund_cmplt_dto(time_now, lines)
    };
    let result = refund_req_m.validate(mock_merchant_id, &mock_cmplt_req);
    assert!(result.is_err());
} // end of fn update_refund_req_ok

#[rustfmt::skip]
#[test]
fn reduce_cmplt_req_dto_ok() {
    let mock_buyer_id = 9802u32;
    let mock_merchant_id = 37u32;
    let time_now = Local::now().to_utc();
    let mock_cmplt_req = {
        let lines = vec![
            (982, ProductType::Package, 41, 8355, 5, 0, 0),
            (982, ProductType::Package, 113, 18381, 11, 0, 1),
            (999, ProductType::Item, 144, 0, 0, 3, 0),
            (999, ProductType::Package, 62, 29997, 9, 0, 0),
        ];
        ut_setup_refund_cmplt_dto(time_now, lines)
    };
    let charge_rawlines = vec![
        (
            mock_merchant_id, ProductType::Package, 982u64,
            (1671i64, 1u32), (20052i64, 1u32), 12u32,
            (0i64, 0u32), (0i64, 0u32), 0u32,
        ),
        (mock_merchant_id, ProductType::Item, 983, (1650, 1), (29700, 1), 18, (0, 0), (0, 0), 0),
        (mock_merchant_id, ProductType::Item, 999, (1900, 1), (9500, 1), 5, (0, 0), (0, 0), 0),
        (mock_merchant_id, ProductType::Package, 603, (990, 1), (2990, 1), 3, (0, 0), (0, 0), 0),
        (mock_merchant_id, ProductType::Package, 999, (3333, 1), (6666, 1), 2, (0, 0), (0, 0), 0),
    ];
    let mock_charge_m =
        ut_setup_buyer_charge_inner(time_now, mock_merchant_id, mock_buyer_id, charge_rawlines);
    let arg = (mock_merchant_id, &mock_charge_m, &mock_cmplt_req);
    let resolve_m = RefundReqResolutionModel::try_from(arg).unwrap();
    let mock_cmplt_req = resolve_m.reduce_resolved(mock_merchant_id, mock_cmplt_req);
    assert_eq!(mock_cmplt_req.lines.len(), 2);
    [
        (982, ProductType::Package, 113, 4, "668.4"),
        (999, ProductType::Package, 62, 7, "2333.1"),
    ].into_iter()
    .map(|d| {
        let t_req = time_now - Duration::minutes(d.2);
        let result = mock_cmplt_req.lines.iter()
            .find(|v| v.product_type == d.1 && v.product_id == d.0 && v.time_issued == t_req);
        let rline = result.unwrap();
        assert_eq!(rline.approval.quantity, d.3);
        assert_eq!(rline.approval.amount_total.as_str(), d.4);
        assert_eq!(rline.reject.get(&RefundRejectReasonDto::Damaged).unwrap_or(&0u32), &0u32);
        assert_eq!(rline.reject.get(&RefundRejectReasonDto::Fraudulent).unwrap_or(&0u32), &0u32);
    })
    .count();
} // end of fn reduce_cmplt_req_dto_ok
