use std::collections::HashMap;

use chrono::{DateTime, Duration, Local, Utc};
use rust_decimal::Decimal;

use ecommerce_common::api::dto::{CurrencyDto, PayAmountDto};
use ecommerce_common::api::rpc::dto::OrderLineReplicaRefundDto;
use ecommerce_common::model::BaseProductIdentity;
use payment::api::web::dto::{
    RefundCompletionOlineReqDto, RefundCompletionReqDto, RefundLineApprovalDto,
    RefundRejectReasonDto,
};
use payment::model::{
    BuyerPayInState, Charge3partyModel, ChargeBuyerModel, ChargeRefundMap, OrderCurrencySnapshot,
    OrderRefundModel, PayLineAmountError, RefundErrorParseOline, RefundModelError,
    RefundReqResolutionModel, StripeCheckoutPaymentStatusModel,
};

use super::{ut_default_charge_method_stripe, ut_setup_buyer_charge, UTestChargeLineRawData};

fn ut_setup_olines_refund_dto(time_base: DateTime<Utc>) -> Vec<OrderLineReplicaRefundDto> {
    [
        (37, 1982, 0, 41, 1671, 8355, 5),
        (37, 982, 0, 87, 1650, 16500, 10),
        (37, 1982, 0, 87, 1671, 33420, 20),
        (37, 1982, 0, 113, 1671, 5013, 3),
        (37, 1982, 1, 101, 1675, 3350, 2),
        (37, 1982, 1, 544, 1675, 6700, 4),
        (37, 1982, 1, 199, 1675, 20100, 12),
        (50, 982, 0, 51, 2222, 15554, 7),
        (50, 591, 0, 54, 805, 7245, 9),
        (50, 591, 1, 84, 805, 805, 1),
        (50, 591, 2, 84, 805, 8050, 10),
        (37, 603, 0, 51, 990, 1980, 2),
        (37, 603, 1, 51, 995, 2985, 3),
        (37, 603, 1, 67, 995, 3980, 4),
        (37, 999, 0, 144, 1900, 9500, 5),
        (37, 1999, 0, 62, 3333, 36663, 11),
    ]
    .into_iter()
    .map(|d| OrderLineReplicaRefundDto {
        seller_id: d.0,
        product_id: d.1,
        attr_set_seq: d.2,
        create_time: (time_base - Duration::minutes(d.3)).to_rfc3339(),
        amount: PayAmountDto {
            unit: Decimal::new(d.4, 1).to_string(),
            total: Decimal::new(d.5, 1).to_string(),
        },
        qty: d.6,
    })
    .collect::<Vec<_>>()
} // end of fn ut_setup_olines_refund_dto

#[rustfmt::skip]
pub(crate) type UTestRefundCmpltDtoRawData = ((u64, u16), i64, i64, u32, u32, u32);

pub(crate) fn ut_setup_refund_cmplt_dto(
    time_base: DateTime<Utc>,
    raw: Vec<UTestRefundCmpltDtoRawData>,
) -> RefundCompletionReqDto {
    let lines = raw
        .into_iter()
        .map(|d| {
            let time_issued = time_base - Duration::minutes(d.1);
            let approval = RefundLineApprovalDto {
                amount_total: Decimal::new(d.2, 1).to_string(),
                quantity: d.3,
            };
            let reject = HashMap::from([
                (RefundRejectReasonDto::Damaged, d.4),
                (RefundRejectReasonDto::Fraudulent, d.5),
            ]);
            RefundCompletionOlineReqDto {
                product_id: d.0 .0,
                attr_set_seq: d.0 .1,
                time_issued,
                reject,
                approval,
            }
        })
        .collect::<Vec<_>>();
    RefundCompletionReqDto { lines }
}

#[test]
fn convert_from_dto_ok() {
    let t_base = Local::now().to_utc();
    let mock_oid = "d1e5390dd2".to_string();
    let mock_data = ut_setup_olines_refund_dto(t_base);
    let result = OrderRefundModel::try_from((mock_oid, mock_data));
    assert!(result.is_ok());
    if let Ok(o_ret) = result {
        [
            (37, 603, 0, 51, 1980, 2),
            (37, 603, 1, 51, 2985, 3),
            (37, 1982, 0, 87, 33420, 20),
            (37, 1982, 0, 113, 5013, 3),
        ]
        .into_iter()
        .map(|d| {
            let expect_ctime = t_base - Duration::minutes(d.3);
            let rline_m = o_ret.get_line(d.0, d.1, d.2, expect_ctime).unwrap();
            assert_eq!(rline_m.requested().qty, d.5);
            assert_eq!(rline_m.requested().total, Decimal::new(d.4, 1));
        })
        .count();
    }
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
        if let RefundModelError::ParseOline {
            pid,
            attr_set_seq,
            reason,
        } = e
        {
            let expect_pid = BaseProductIdentity {
                store_id: 37,
                product_id: 1999,
            };
            assert_eq!(pid, expect_pid);
            assert_eq!(attr_set_seq, 0u16);
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
    let time_now = Local::now().to_utc();
    let mock_oid = "d1e5390dd2".to_string();
    let mock_data = ut_setup_olines_refund_dto(time_now);
    let rfd_req = OrderRefundModel::try_from((mock_oid, mock_data)).unwrap();
    let mock_merchant_id = 37u32;
    let mock_cmplt_req = {
        let lines = vec![
            ((1982, 0), 41, 8355, 5, 0, 0),
            ((982, 0), 87, 1650, 1, 2, 0),
            ((1982, 0), 87, 16710, 10, 3, 4),
            ((999, 0), 144, 7600, 4, 0, 1),
            ((603, 1), 67, 2985, 3, 1, 0),
            ((603, 1), 51, 995, 1, 0, 0),
            ((1999, 0), 62, 36663, 11, 0, 0),
        ];
        ut_setup_refund_cmplt_dto(time_now, lines)
    };
    let result = rfd_req.validate(mock_merchant_id, &mock_cmplt_req);
    assert!(result.is_ok());
    if let Ok(vs) = result {
        assert_eq!(vs.len(), 7);
        vs.into_iter()
            .map(|d| {
                let actual = (d.3, d.4);
                let expect = match (d.0, d.1, (time_now - d.2).num_minutes()) {
                    (1982, 0, 41) => (0u32, Decimal::ZERO),
                    (982, 0, 87) => (7, Decimal::new(11550, 1)),
                    (1982, 0, 87) => (3, Decimal::new(5013, 1)),
                    (999, 0, 144) => (0, Decimal::ZERO),
                    (1999, 0, 62) => (0, Decimal::ZERO),
                    (603, 1, 67) => (0, Decimal::ZERO),
                    (603, 1, 51) => (2, Decimal::new(199, 0)),
                    _others => (9999, Decimal::NEGATIVE_ONE),
                };
                assert_eq!(actual, expect);
            })
            .count();
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
            ((1982, 0), 87, 1671, 1, 5, 6),
            ((999, 0), 144, 7600, 4, 1, 1),
            // assume the total amount in the request is corrupted
            ((1999, 0), 62, 39999, 11, 0, 0),
        ];
        ut_setup_refund_cmplt_dto(time_now, lines)
    };
    let result = rfd_req.validate(mock_merchant_id, &mock_cmplt_req);
    assert!(result.is_err());
    if let Err(es) = result {
        assert_eq!(es.len(), 2);
        es.into_iter()
            .map(|e| match e {
                RefundModelError::QtyInsufficient {pid, attr_set_seq, num_avail, num_req} => {
                    assert_eq!(pid.store_id, mock_merchant_id);
                    assert_eq!(pid.product_id, 999u64);
                    assert_eq!(attr_set_seq, 0u16);
                    assert!(num_avail < num_req);
                }
                RefundModelError::AmountInsufficient {pid, attr_set_seq, num_avail, num_req} => {
                    assert_eq!(pid.store_id, mock_merchant_id);
                    assert_eq!(pid.product_id, 1999u64);
                    assert_eq!(attr_set_seq, 0u16);
                    assert!(num_avail < num_req);
                }
                _others => { assert!(false); }
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
            ((1982, 0), 41, 8355, 5, 0, 0),
            ((1982, 0), 87, 16710, 10, 1, 2),
            ((1982, 0), 129, 1671, 1, 0, 0), // completion request will be omitted
            ((1982, 0), 138, 0, 0, 1, 0),
            ((1982, 0), 113, 1671, 1, 0, 1),
            ((1982, 1), 101, 1675, 1, 1, 0),
            ((1982, 1), 544, 1675, 1, 0, 1),
            ((1982, 1), 199, 5250, 3, 4, 5),
            ((1982, 2), 199, 1675, 1, 0, 0),
            ((983, 0),  87, 1650, 1, 2, 0),
            ((983, 0), 106, 3300, 2, 0, 5),
            ((985, 0),  35, 2500, 1, 0, 10), // completion request will be omitted
            ((999, 0),  43, 7600, 4, 1, 1),
            ((999, 0),  49, 3800, 2, 0, 0), // completion request will be omitted
            ((999, 0), 144, 0, 0, 7, 0),
        ];
        ut_setup_refund_cmplt_dto(time_now, lines)
    };
    let charge_rawlines = vec![
        ((mock_merchant_id, 1982, 0), ((1671, 1), (28407, 1), 17), ((0, 0), (0, 0), 0), 5),
        ((mock_merchant_id, 1982, 1), ((1675, 1), (5025, 0), 30), ((1675, 1), (1675, 1), 1), 2),
        ((mock_merchant_id, 983, 0), ((1650, 1), (29700, 1), 18), ((0, 0), (0, 0), 0), 0),
        ((mock_merchant_id, 999, 0), ((1900, 1), (11400, 1), 6), ((1900, 1), (3800, 1), 2), 1),
        ((mock_merchant_id, 603, 0), ((990, 1), (2990, 1), 3), ((0, 0), (0, 0), 0), 1),
    ];
    let mock_charge_m = ut_setup_buyer_charge_inner(
        time_now, mock_merchant_id, mock_buyer_id, charge_rawlines
    );
    let arg = (mock_merchant_id, &mock_charge_m ,&mock_cmplt_req);
    let result = RefundReqResolutionModel::try_from(arg);
    assert!(result.is_ok());
    if let Ok(rfnd_rslv_m) = result {
        [
            (1982, 0, 41, 8355, 5, 0, 0),
            (1982, 0, 87, 11697, 7, 1, 2),
            (1982, 0, 138, 0, 0, 1, 0),
            (1982, 0, 113, 0, 0, 0, 1),
            (1982, 1, 101, 1675, 1, 1, 0),
            (1982, 1, 544, 1675, 1, 0, 1),
            (1982, 1, 199, 5250, 3, 4, 5),
            (983,  0,  87, 1650, 1, 2, 0),
            (983,  0, 106, 3300, 2, 0, 5),
            (999,  0,  43, 5700, 3, 1, 1),
            (999,  0, 144, 0, 0, 7, 0),
        ].into_iter().map(|d| {
            let t_req = time_now - Duration::minutes(d.2);
            let (reject_rslv, amt_rslv) =
                rfnd_rslv_m.get_status(mock_merchant_id, d.0, d.1, t_req).unwrap();
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
        [ // the completion requests  below should be omitted
            (1982, 0, 129),
            (985,  0, 35),
            (999,  0, 49),
            // non-exist requests
            (1982, 0, 9876),
            (1982, 1, 9876),
            (1982, 2, 199),
            (1982, 2, 9876),
        ].into_iter().map(|d| {
            let t_req = time_now - Duration::minutes(d.2);
            let result = rfnd_rslv_m.get_status(mock_merchant_id, d.0, d.1, t_req);
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
            ((1982, 0), 41, 8355, 5, 0, 0),
            ((1982, 0), 87, 16710, 10, 1, 2),
        ];
        ut_setup_refund_cmplt_dto(time_now, lines)
    };
    let charge_rawlines = vec![
        ((mock_merchant_ids[0], 1982, 0), ((1671, 1), (20052, 1), 12), ((0, 0), (0, 0), 0), 0),
        ((mock_merchant_ids[0], 983, 0), ((1650, 1), (29700, 1), 18), ((0, 0), (0, 0), 0), 0),
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
            ((1982, 0), 41, 8355, 5, 0, 0),
            ((1982, 0), 113, 1671, 1, 0, 1),
            ((1982, 1), 544, 1675, 1, 0, 0),
            ((1982, 1), 199, 1675, 1, 5, 0),
            ((999,  0), 144, 0, 0, 3, 0),
            ((1999, 0), 62, 3333, 1, 0, 0),
        ];
        ut_setup_refund_cmplt_dto(time_now, lines)
    };
    let charge_rawlines = vec![
        ((mock_merchant_id, 1982, 0), ((1671, 1), (20052, 1), 12), ((0, 0), (0, 0), 0), 0),
        ((mock_merchant_id, 1982, 1), ((1675, 1), (5025, 0), 30), ((1675, 1), (3350, 1), 2), 2),
        ((mock_merchant_id, 983, 0), ((1650, 1), (29700, 1), 18), ((0, 0), (0, 0), 0), 0),
        ((mock_merchant_id, 999, 0), ((1900, 1), (9500, 1), 5), ((0, 0), (0, 0), 0), 0),
        ((mock_merchant_id, 603, 0), ((990, 1), (2970, 1), 3), ((0, 0), (0, 0), 0), 1),
        ((mock_merchant_id, 1999, 0), ((3333, 1), (43329, 1), 13), ((0, 0), (0, 0), 0), 2),
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
            ((982,  0), 87, 16500, 10, 0, 0),
            ((1982, 0), 87, 33420, 20, 0, 0),
            ((1982, 0), 113, 1671, 1, 0, 0),
            ((1982, 1), 544, 1675, 1, 0, 0), // FIXME, expect ((1982, 1), 544, 5025, 3, 0, 0),
            ((1982, 1), 199, 6700, 4, 0, 0), // FIXME, expect ((1982, 1), 199, 10050, 6, 0, 0),
            ((999,  0), 144, 3800, 2, 0, 0),
            ((1999, 0), 62, 33330, 10, 0, 0),
            ((603,  0), 51, 1980, 2, 0, 0),
        ];
        ut_setup_refund_cmplt_dto(time_now, lines)
    };
    let result = refund_req_m.validate(mock_merchant_id, &mock_cmplt_req);
    assert!(result.is_ok());
    let mock_cmplt_req = {
        let lines = vec![
            ((1982, 1), 544, 0, 0, 0, 1), // FIXME, expect ((1982, 1), 544, 0, 0, 2, 1),
            ((1982, 1), 199, 0, 0, 2, 2), // FIXME, expcet ((1982, 1), 199, 0, 0, 3, 3),
        ];
        ut_setup_refund_cmplt_dto(time_now, lines)
    };
    let result = refund_req_m.validate(mock_merchant_id, &mock_cmplt_req);
    if let Err(es) = &result {
        print!("[DEBUG] refund-validate-failure {:?}", es);
    }
    assert!(result.is_ok());
    let mock_cmplt_req = {
        let lines = vec![
            ((1982, 0), 113, 1671, 1, 1, 0),
            ((1982, 1), 544, 1675, 2, 0, 0),
            //((1982, 1), 199, 1675, 19, 0, 1),
        ];
        ut_setup_refund_cmplt_dto(time_now, lines)
    };
    let result = refund_req_m.validate(mock_merchant_id, &mock_cmplt_req);
    assert!(result.is_err());
    if let Err(es) = result {
        es.into_iter()
            .map(|e| match e {
                RefundModelError::QtyInsufficient {pid, attr_set_seq, num_avail, num_req} => {
                    assert_eq!(pid.store_id, mock_merchant_id);
                    let expect = match (pid.product_id, attr_set_seq) {
                        (1982u64, 0u16) => (0u32, 1u32),
                        (1982, 1) => (1, 2),
                        _ => (9999, 9999),
                    };
                    assert_eq!(num_avail, expect.0);
                    assert_eq!(num_req, expect.1);
                }
                _others => { assert!(false); }
            }).count();
    }
    let mock_cmplt_req = {
        let lines = vec![((1982, 0), 113, 3342, 2, 0, 0)];
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
            ((1982, 0), 41, 8355, 5, 0, 0),
            ((1982, 0), 113, 18381, 11, 0, 1),
            ((999,  0), 144, 0, 0, 3, 0),
            ((1999, 0), 62, 29997, 9, 0, 0),
        ];
        ut_setup_refund_cmplt_dto(time_now, lines)
    };
    let charge_rawlines = vec![
        (
            (mock_merchant_id, 1982u64, 0u16), ((1671i64, 1u32), (20052i64, 1u32), 12u32),
            ((0i64, 0u32), (0i64, 0u32), 0u32), 0u32,
        ),
        ((mock_merchant_id, 983, 0), ((1650, 1), (29700, 1), 18), ((0, 0), (0, 0), 0), 0),
        ((mock_merchant_id, 999, 0), ((1900, 1), (9500, 1), 5), ((0, 0), (0, 0), 0), 0),
        ((mock_merchant_id, 603, 0), ((990, 1), (2990, 1), 3), ((0, 0), (0, 0), 0), 0),
        ((mock_merchant_id, 1999, 0), ((3333, 1), (6666, 1), 2), ((0, 0), (0, 0), 0), 0),
    ];
    let mock_charge_m =
        ut_setup_buyer_charge_inner(time_now, mock_merchant_id, mock_buyer_id, charge_rawlines);
    let arg = (mock_merchant_id, &mock_charge_m, &mock_cmplt_req);
    let resolve_m = RefundReqResolutionModel::try_from(arg).unwrap();
    let mock_cmplt_req = resolve_m.reduce_resolved(mock_merchant_id, mock_cmplt_req);
    assert_eq!(mock_cmplt_req.lines.len(), 2);
    [
        (1982, 113, 4, "668.4"),
        (1999, 62, 7, "2333.1"),
    ].into_iter()
    .map(|d| {
        let t_req = time_now - Duration::minutes(d.1);
        let result = mock_cmplt_req.lines.iter()
            .find(|v| v.product_id == d.0 && v.time_issued == t_req);
        let rline = result.unwrap();
        assert_eq!(rline.approval.quantity, d.2);
        assert_eq!(rline.approval.amount_total.as_str(), d.3);
        assert_eq!(rline.reject.get(&RefundRejectReasonDto::Damaged).unwrap_or(&0u32), &0u32);
        assert_eq!(rline.reject.get(&RefundRejectReasonDto::Fraudulent).unwrap_or(&0u32), &0u32);
    })
    .count();
} // end of fn reduce_cmplt_req_dto_ok

#[rustfmt::skip]
#[test]
fn resolution_to_charge_map() {
    let mock_buyer_id = 9802u32;
    let mock_merchant_id = 37u32;
    let time_now = Local::now().to_utc();
    
    let charge0_rawlines = vec![
        ((mock_merchant_id, 1982, 0), ((1671, 1), (20052, 1), 12), ((0, 0), (0, 0), 0), 0),
        ((mock_merchant_id, 983, 0), ((1650, 1), (29700, 1), 18), ((1650, 1), (4950, 1), 3), 4),
        ((mock_merchant_id, 918, 0), ((5566, 1), (5566, 1), 1), ((0, 0), (0, 0), 0), 0),
    ];
    let charge1_rawlines = vec![
        ((mock_merchant_id, 1982, 0), ((1671, 1), (16710, 1), 10), ((1671, 1), (3342, 1), 2), 0),
        ((mock_merchant_id, 983, 0), ((1650, 1), (29700, 1), 18), ((0, 0), (0, 0), 0), 1),
    ];
    let mock_charge_ms = [
        ut_setup_buyer_charge_inner(
            time_now - Duration::hours(1), mock_merchant_id,
            mock_buyer_id, charge0_rawlines
        ),
        ut_setup_buyer_charge_inner(
            time_now - Duration::hours(2), mock_merchant_id,
            mock_buyer_id, charge1_rawlines
        ),
    ];
    
    let mock_cmplt_req0 = {
        let lines = vec![
            ((1982, 0), 41,  6684, 4, 0, 0),
            ((1982, 0), 113, 5013, 3, 0, 0),
            ((983, 0), 144, 0, 0, 1, 3),
            ((983, 0), 62, 3300, 2, 0, 1),
        ];
        ut_setup_refund_cmplt_dto(time_now - Duration::minutes(20), lines)
    };
    let mock_cmplt_req1 = {
        let lines = vec![
            ((1982, 0), 136, 1671, 1, 1, 1),
            ((1982, 0), 126, 3342, 2, 1, 1),
            ((983, 0), 154, 1650, 1, 0, 0),
            ((983, 0), 162, 8250, 5, 1, 0),
        ];
        ut_setup_refund_cmplt_dto(time_now - Duration::minutes(15), lines)
    };

    let arg = (mock_merchant_id, &mock_charge_ms[0], &mock_cmplt_req0);
    let resolve_m0 = RefundReqResolutionModel::try_from(arg).unwrap();
    let arg = (mock_merchant_id, &mock_charge_ms[1], &mock_cmplt_req1);
    let resolve_m1 = RefundReqResolutionModel::try_from(arg).unwrap();

    let rfd_rslv_ms = [resolve_m0, resolve_m1];
    let actual_map = ChargeRefundMap::build(&rfd_rslv_ms);
    [
        (0, 2, 1982, 7, "167.1", "1169.7", 0),
        (0, 2, 983,  5, "165.0", "825.0", 9),
        (1, 2, 1982, 5, "167.1", "835.5", 4),
        (1, 2, 983,  6, "165.0", "990.0", 2),
    ].into_iter()
        .map(|d| {
            let inner_map = actual_map.get(mock_buyer_id, *mock_charge_ms[d.0].meta.create_time()).unwrap();
            assert_eq!(inner_map.len(), d.1);
            let entry = inner_map.get(mock_merchant_id, d.2, 0).unwrap();
            assert_eq!(entry.0.qty, d.3);
            assert_eq!(entry.0.unit.to_string().as_str(), d.4);
            assert_eq!(entry.0.total.to_string().as_str(), d.5);
            assert_eq!(entry.1, d.6); // num rejrected so far
        })
        .count();
} // end of fn resolution_to_charge_map
