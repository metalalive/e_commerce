use std::collections::HashMap;

use chrono::{DateTime, Duration, FixedOffset, Local};

use order::api::web::dto::{OrderLineReqDto, OrderLineReturnErrorReason};
use order::constant::hard_limit;
use order::model::{
    OrderLineAppliedPolicyModel, OrderLineIdentity, OrderLineModel, OrderLinePriceModel,
    OrderLineQuantityModel, OrderReturnModel,
};

fn ut_saved_orderline_setup(dt_now: DateTime<FixedOffset>, store_id: u32) -> Vec<OrderLineModel> {
    let paid_last_update = dt_now - Duration::days(3);
    let reserved_until = dt_now + Duration::hours(2);
    let warranty_until = dt_now + Duration::hours(8);
    vec![
        OrderLineModel {
            price: OrderLinePriceModel { unit: 7, total: 70 },
            id_: OrderLineIdentity {
                store_id,
                product_id: 812, // type: Package,
            },
            qty: OrderLineQuantityModel {
                reserved: 10,
                paid: 0,
                paid_last_update: None,
            },
            policy: OrderLineAppliedPolicyModel {
                reserved_until,
                warranty_until,
            },
        },
        OrderLineModel {
            price: OrderLinePriceModel {
                unit: 11,
                total: 99,
            },
            id_: OrderLineIdentity {
                store_id,
                product_id: 890, // type: Item,
            },
            qty: OrderLineQuantityModel {
                reserved: 9,
                paid: 7,
                paid_last_update: Some(paid_last_update),
            },
            policy: OrderLineAppliedPolicyModel {
                reserved_until,
                warranty_until,
            },
        },
        OrderLineModel {
            price: OrderLinePriceModel { unit: 5, total: 80 },
            id_: OrderLineIdentity {
                store_id,
                product_id: 574, // type: Package,
            },
            qty: OrderLineQuantityModel {
                reserved: 16,
                paid: 12,
                paid_last_update: Some(paid_last_update),
            },
            policy: OrderLineAppliedPolicyModel {
                reserved_until,
                warranty_until,
            },
        },
        OrderLineModel {
            price: OrderLinePriceModel {
                unit: 13,
                total: 130,
            },
            id_: OrderLineIdentity {
                store_id,
                product_id: 257, // type: Item,
            },
            qty: OrderLineQuantityModel {
                reserved: 10,
                paid: 10,
                paid_last_update: Some(paid_last_update),
            },
            policy: OrderLineAppliedPolicyModel {
                reserved_until,
                warranty_until,
            },
        },
    ]
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
            id_: OrderLineIdentity {
                store_id,
                product_id: 257, // type: Item,
            },
            qty: HashMap::from([
                (
                    last_returned[0].clone(),
                    (
                        2u32,
                        OrderLinePriceModel {
                            unit: 13,
                            total: 26,
                        },
                    ),
                ),
                (
                    last_returned[1].clone(),
                    (
                        1u32,
                        OrderLinePriceModel {
                            unit: 13,
                            total: 13,
                        },
                    ),
                ),
            ]),
        },
        OrderReturnModel {
            id_: OrderLineIdentity {
                store_id,
                product_id: 574, // type: Package,
            },
            qty: HashMap::from([(
                last_returned[0].clone(),
                (1u32, OrderLinePriceModel { unit: 5, total: 5 }),
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
    let data = vec![
        OrderLineReqDto {
            seller_id,
            product_id: 890,
            quantity: 4,
        },
        OrderLineReqDto {
            seller_id,
            product_id: 574,
            quantity: 1,
        },
        OrderLineReqDto {
            seller_id,
            product_id: 257,
            quantity: 3,
        },
    ];
    let result = OrderReturnModel::filter_requests(data, o_lines, o_returns);
    assert!(result.is_ok());
    if let Ok(modified) = result {
        assert_eq!(modified.len(), 3);
        modified
            .iter()
            .map(|m| {
                let num_returned = m.qty.values().map(|d| d.0).sum::<u32>();
                let actual = (m.qty.len(), num_returned);
                let expect = match m.id_.product_id {
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
    let data = vec![
        OrderLineReqDto {
            seller_id,
            product_id: 890,
            quantity: 4,
        },
        OrderLineReqDto {
            seller_id,
            product_id: 574,
            quantity: 1,
        },
        OrderLineReqDto {
            seller_id,
            product_id: 1888,
            quantity: 666,
        },
    ];
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
    let data = vec![
        OrderLineReqDto {
            seller_id,
            product_id: 890,
            quantity: 1,
        },
        OrderLineReqDto {
            seller_id,
            product_id: 574,
            quantity: 1,
        },
        OrderLineReqDto {
            seller_id,
            product_id: 257,
            quantity: 2,
        },
    ];
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
    let data = vec![
        OrderLineReqDto {
            seller_id,
            product_id: 9999,
            quantity: 1,
        },
        OrderLineReqDto {
            seller_id,
            product_id: 890,
            quantity: 3,
        },
        OrderLineReqDto {
            seller_id,
            product_id: 574,
            quantity: 16,
        },
        OrderLineReqDto {
            seller_id,
            product_id: 257,
            quantity: 2,
        },
    ];
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
        let value = (2, OrderLinePriceModel { unit: 5, total: 10 });
        objs.last_mut().unwrap().qty.insert(key, value);
        assert_eq!(objs[1].id_.product_id, 574);
        assert_eq!(objs[1].qty.len(), 2);
        objs
    }; // assume the record is already added to the return model
    let data = vec![
        OrderLineReqDto {
            seller_id,
            product_id: 890,
            quantity: 3,
        },
        OrderLineReqDto {
            seller_id,
            product_id: 574,
            quantity: 1,
        },
        OrderLineReqDto {
            seller_id,
            product_id: 257,
            quantity: 1,
        },
    ];
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
