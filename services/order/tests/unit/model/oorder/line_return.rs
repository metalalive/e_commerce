use std::collections::HashMap;

use chrono::{DateTime, Duration, FixedOffset, Local};

use order::api::web::dto::{OrderLineReqDto, OrderLineReturnErrorReason};
use order::constant::hard_limit;
use order::model::{
    OrderLineAppliedPolicyModel, OrderLineIdentity, OrderLineModel, OrderLinePriceModel,
    OrderLineQuantityModel, OrderReturnModel, ProdAttriPriceModel,
};

#[rustfmt::skip]
fn ut_saved_orderline_setup(dt_now: DateTime<FixedOffset>, store_id: u32) -> Vec<OrderLineModel> {
    let paid_last_update = dt_now - Duration::days(3);
    let reserved_until = dt_now + Duration::hours(2);
    let warranty_until = dt_now + Duration::hours(8);
    let attr_lastupdate = dt_now - Duration::days(1);
    [
        (812, (7, 70), 10, 0, None),
        (890, (11, 99), 9, 7, Some(paid_last_update)),
        (574, (5, 80), 16, 12, Some(paid_last_update)),
        (257, (13, 130), 10, 10, Some(paid_last_update)),
    ]
    .into_iter()
    .map(
        |(product_id, (unit, total), reserved, paid, paid_last_update)| {
            let args = (
                OrderLineIdentity::from((store_id, product_id, 0)),
                OrderLinePriceModel::from((unit, total)),
                OrderLineAppliedPolicyModel {
                    reserved_until, warranty_until,
                },
                OrderLineQuantityModel {
                    reserved, paid, paid_last_update,
                },
                ProdAttriPriceModel::from((attr_lastupdate, None)),
            );
            OrderLineModel::from(args)
        },
    )
    .collect::<Vec<_>>()
} // end of fn ut_saved_orderline_setup

fn ut_saved_oline_return_setup(
    dt_now: DateTime<FixedOffset>,
    store_id: u32,
) -> Vec<OrderReturnModel> {
    let interval_secs = hard_limit::MIN_SECS_INTVL_REQ as i64;
    let last_returned = [
        OrderReturnModel::dtime_round_secs(&(dt_now - Duration::minutes(11)), interval_secs)
            .unwrap(),
        OrderReturnModel::dtime_round_secs(&(dt_now - Duration::minutes(5)), interval_secs)
            .unwrap(),
    ];
    vec![
        OrderReturnModel {
            id_: OrderLineIdentity::from((store_id, 257, 0)),
            qty: HashMap::from([
                (
                    last_returned[0].clone(),
                    (2u32, OrderLinePriceModel::from((13, 26))),
                ),
                (
                    last_returned[1].clone(),
                    (1u32, OrderLinePriceModel::from((13, 13))),
                ),
            ]),
        },
        OrderReturnModel {
            id_: OrderLineIdentity::from((store_id, 574, 0)),
            qty: HashMap::from([(
                last_returned[0].clone(),
                (1u32, OrderLinePriceModel::from((5, 5))),
            )]),
        },
    ]
} // end of fn ut_saved_oline_return_setup

#[test]
fn filter_request_ok() {
    let seller_id = 145;
    let dt_now = Local::now().fixed_offset();
    let o_lines = ut_saved_orderline_setup(dt_now.clone(), seller_id);
    let o_returns = {
        let objs = ut_saved_oline_return_setup(dt_now.clone(), seller_id);
        let num_returned = objs[0].qty.values().map(|d| d.0).sum::<u32>();
        assert_eq!(num_returned, 3u32);
        let num_returned = objs[1].qty.values().map(|d| d.0).sum::<u32>();
        assert_eq!(num_returned, 1u32);
        objs
    };
    let data = [(890, 4), (574, 1), (257, 3)]
        .into_iter()
        .map(|(product_id, quantity)| OrderLineReqDto {
            seller_id,
            product_id,
            quantity,
            applied_attr: None,
        })
        .collect::<Vec<_>>();
    let result = OrderReturnModel::filter_requests(data, o_lines, o_returns);
    assert!(result.is_ok());
    if let Ok(modified) = result {
        assert_eq!(modified.len(), 3);
        modified
            .iter()
            .map(|m| {
                let num_returned = m.qty.values().map(|d| d.0).sum::<u32>();
                let actual = (m.qty.len(), num_returned);
                let expect = match m.id_.product_id() {
                    890u64 => (1usize, 4u32),
                    574 => (1, 1),
                    257 => (1, 3),
                    _others => (0, 0),
                };
                assert_eq!(actual, expect);
            })
            .count();
    }
} // end of fn filter_request_ok

#[test]
fn filter_request_err_nonexist() {
    let seller_id = 145;
    let dt_now = Local::now().fixed_offset();
    let o_lines = ut_saved_orderline_setup(dt_now.clone(), seller_id);
    let o_returns = ut_saved_oline_return_setup(dt_now.clone(), seller_id);
    let data = [(890, 4), (574, 1), (1888, 666)]
        .into_iter()
        .map(|(product_id, quantity)| OrderLineReqDto {
            seller_id,
            product_id,
            quantity,
            applied_attr: None,
        })
        .collect::<Vec<_>>();
    let result = OrderReturnModel::filter_requests(data, o_lines, o_returns);
    assert!(result.is_err());
    if let Err(es) = result {
        assert_eq!(es.len(), 1);
        assert_eq!(es[0].product_id, 1888);
        assert!(matches!(es[0].reason, OrderLineReturnErrorReason::NotExist));
    }
}

#[test]
fn filter_request_warranty_expired() {
    let seller_id = 145;
    let dt_now = Local::now().fixed_offset();
    let o_lines = {
        let expiry_time = Duration::hours(18);
        let mut objs = ut_saved_orderline_setup(dt_now.clone(), seller_id);
        if let Some(t) = objs[1].qty.paid_last_update.as_mut() {
            *t -= expiry_time;
        };
        objs[1].policy.reserved_until -= expiry_time;
        objs[1].policy.warranty_until -= expiry_time;
        objs
    };
    let o_returns = ut_saved_oline_return_setup(dt_now.clone(), seller_id);
    let data = [(890, 1), (574, 1), (257, 2)]
        .into_iter()
        .map(|(product_id, quantity)| OrderLineReqDto {
            seller_id,
            product_id,
            quantity,
            applied_attr: None,
        })
        .collect::<Vec<_>>();
    let result = OrderReturnModel::filter_requests(data, o_lines, o_returns);
    assert!(result.is_err());
    if let Err(es) = result {
        assert_eq!(es.len(), 1);
        assert_eq!(es[0].product_id, 890);
        assert!(matches!(
            es[0].reason,
            OrderLineReturnErrorReason::WarrantyExpired
        ));
    }
}

#[test]
fn filter_request_invalid_qty() {
    let seller_id = 145;
    let dt_now = Local::now().fixed_offset();
    let o_lines = ut_saved_orderline_setup(dt_now.clone(), seller_id);
    let o_returns = ut_saved_oline_return_setup(dt_now.clone(), seller_id);
    let data = [(9999, 1), (890, 3), (574, 16), (257, 2)]
        .into_iter()
        .map(|(product_id, quantity)| OrderLineReqDto {
            seller_id,
            product_id,
            quantity,
            applied_attr: None,
        })
        .collect::<Vec<_>>();
    let result = OrderReturnModel::filter_requests(data, o_lines, o_returns);
    assert!(result.is_err());
    if let Err(es) = result {
        assert_eq!(es.len(), 2);
        es.iter()
            .map(|m| match m.product_id {
                574 => assert!(matches!(
                    m.reason,
                    OrderLineReturnErrorReason::QtyLimitExceed
                )),
                9999 => assert!(matches!(m.reason, OrderLineReturnErrorReason::NotExist)),
                _others => assert!(false),
            })
            .count();
    }
}

#[test]
fn filter_request_err_duplicate() {
    let seller_id = 145;
    let dt_now = Local::now().fixed_offset();
    let o_lines = ut_saved_orderline_setup(dt_now.clone(), seller_id);
    let o_returns = {
        let mut objs = ut_saved_oline_return_setup(dt_now.clone(), seller_id);
        let interval_secs = hard_limit::MIN_SECS_INTVL_REQ as i64;
        let key = OrderReturnModel::dtime_round_secs(&dt_now, interval_secs).unwrap();
        let value = (2, OrderLinePriceModel::from((5, 10)));
        objs.last_mut().unwrap().qty.insert(key, value);
        assert_eq!(objs[1].id_.product_id(), 574);
        assert_eq!(objs[1].qty.len(), 2);
        objs
    }; // assume the record is already added to the return model
    let data = [(890, 3), (574, 1), (257, 1)]
        .into_iter()
        .map(|(product_id, quantity)| OrderLineReqDto {
            seller_id,
            product_id,
            quantity,
            applied_attr: None,
        })
        .collect::<Vec<_>>();
    let result = OrderReturnModel::filter_requests(data, o_lines, o_returns);
    assert!(result.is_err());
    if let Err(es) = result {
        assert_eq!(es.len(), 1);
        assert_eq!(es[0].product_id, 574);
        assert!(matches!(
            es[0].reason,
            OrderLineReturnErrorReason::DuplicateReturn
        ));
    }
}
