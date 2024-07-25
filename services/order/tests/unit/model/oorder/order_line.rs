use chrono::{DateTime, Duration, FixedOffset, Local as LocalTime};
use rust_decimal::Decimal;

use ecommerce_common::api::dto::CurrencyDto;
use ecommerce_common::constant::ProductType;
use ecommerce_common::error::AppErrorCode;

use order::api::rpc::dto::{OrderLinePaidUpdateDto, OrderLinePayUpdateErrorReason};
use order::api::web::dto::OrderLineReqDto;
use order::model::{
    CurrencyModel, OrderLineAppliedPolicyModel, OrderLineIdentity, OrderLineModel,
    OrderLinePriceModel, OrderLineQuantityModel, ProductPolicyModel, ProductPriceModel,
};

#[rustfmt::skip]
pub(super) fn ut_setup_order_lines(
    data : Vec<(
        u32, u64, ProductType,
        u32, u32, u32, u32,
        Option<DateTime<FixedOffset>>,
        DateTime<FixedOffset>,
        DateTime<FixedOffset>
    )>
) -> Vec<OrderLineModel> {
    data.into_iter()
        .map(|d| OrderLineModel {
            id_: OrderLineIdentity {
                store_id: d.0,
                product_id: d.1,
                product_type: d.2,
            },
            price: OrderLinePriceModel {unit: d.3, total: d.4},
            qty: OrderLineQuantityModel {
                reserved: d.5,
                paid: d.6,
                paid_last_update: d.7,
            },
            policy: OrderLineAppliedPolicyModel {
                reserved_until: d.8,
                warranty_until: d.9,
            },
        })
        .collect::<Vec<_>>()
} // end of fn ut_setup_order_lines

#[test]
fn convert_from_req_dto_without_rsv_limit_ok() {
    let (seller_id, product_id, product_type) = (19, 146, ProductType::Item);
    let policym = ProductPolicyModel {
        product_type: product_type.clone(),
        product_id,
        is_create: false,
        auto_cancel_secs: 69,
        warranty_hours: 23,
        max_num_rsv: 0,
        min_num_rsv: 0,
    };
    let pricem = ProductPriceModel {
        product_id,
        product_type: product_type.clone(),
        price: 1015,
        is_create: false,
        start_after: DateTime::parse_from_rfc3339("2023-07-31T10:16:54+05:00")
            .unwrap()
            .into(),
        end_before: DateTime::parse_from_rfc3339("2023-09-10T09:01:31+02:00")
            .unwrap()
            .into(),
    };
    let data = OrderLineReqDto {
        seller_id,
        product_id,
        product_type,
        quantity: 26,
    };
    let result = OrderLineModel::try_from(data, &policym, &pricem);
    let m = result.unwrap();
    assert_eq!(m.price.unit, 1015u32);
    assert_eq!(m.price.total, 1015u32 * 26u32);
    assert_eq!(m.qty.reserved, 26);
    let timenow = LocalTime::now().fixed_offset();
    let expect_reserved_time = timenow + Duration::seconds(69i64);
    assert!(m.policy.reserved_until <= expect_reserved_time);
}

#[test]
fn convert_from_req_dto_with_rsv_limit_ok() {
    let (seller_id, product_id, product_type) = (19, 146, ProductType::Item);
    let policym = ProductPolicyModel {
        product_type: product_type.clone(),
        product_id,
        is_create: false,
        auto_cancel_secs: 69,
        warranty_hours: 23,
        max_num_rsv: 10,
        min_num_rsv: 2,
    };
    let pricem = ProductPriceModel {
        product_id,
        product_type: product_type.clone(),
        price: 987,
        is_create: false,
        start_after: DateTime::parse_from_rfc3339("2022-10-28T10:16:54+05:00")
            .unwrap()
            .into(),
        end_before: DateTime::parse_from_rfc3339("2022-10-31T06:11:50+02:00")
            .unwrap()
            .into(),
    };
    let data = OrderLineReqDto {
        seller_id,
        product_id,
        product_type,
        quantity: 9,
    };
    let result = OrderLineModel::try_from(data, &policym, &pricem);
    let m = result.unwrap();
    assert_eq!(m.price.unit, 987u32);
    assert_eq!(m.price.total, 987u32 * 9u32);
    assert_eq!(m.qty.reserved, 9u32);
}

#[test]
fn convert_from_req_dto_violate_rsv_limit() {
    let (seller_id, product_id, product_type) = (19, 146, ProductType::Item);
    let policym = ProductPolicyModel {
        product_type: product_type.clone(),
        product_id,
        is_create: false,
        auto_cancel_secs: 180,
        warranty_hours: 48,
        max_num_rsv: 10,
        min_num_rsv: 0,
    };
    let pricem = ProductPriceModel {
        product_id,
        product_type: product_type.clone(),
        price: 987,
        is_create: false,
        start_after: DateTime::parse_from_rfc3339("2022-10-28T10:16:54+05:00")
            .unwrap()
            .into(),
        end_before: DateTime::parse_from_rfc3339("2022-10-31T06:11:50+02:00")
            .unwrap()
            .into(),
    };
    let data = OrderLineReqDto {
        seller_id,
        product_id,
        product_type,
        quantity: 11,
    };
    let result = OrderLineModel::try_from(data, &policym, &pricem);
    assert!(result.is_err());
    if let Err(e) = result {
        assert_eq!(e.code, AppErrorCode::ExceedingMaxLimit);
    }
}

#[test]
fn convert_from_req_dto_product_id_mismatch() {
    let (seller_id, product_id, product_type) = (19, 146, ProductType::Item);
    let policym = ProductPolicyModel {
        product_type: product_type.clone(),
        product_id,
        is_create: false,
        auto_cancel_secs: 180,
        warranty_hours: 48,
        max_num_rsv: 10,
        min_num_rsv: 0,
    };
    let pricem = ProductPriceModel {
        product_id: 1466,
        product_type: product_type.clone(),
        price: 60,
        is_create: false,
        start_after: DateTime::parse_from_rfc3339("2022-10-28T10:16:54+05:00")
            .unwrap()
            .into(),
        end_before: DateTime::parse_from_rfc3339("2022-10-31T06:11:50+02:00")
            .unwrap()
            .into(),
    };
    let data = OrderLineReqDto {
        seller_id,
        product_id,
        product_type,
        quantity: 2,
    };
    let result = OrderLineModel::try_from(data, &policym, &pricem);
    assert!(result.is_err());
    if let Err(e) = result {
        assert_eq!(e.code, AppErrorCode::DataCorruption);
    }
}

#[rustfmt::skip]
#[test]
fn convert_to_pay_dto_ok() {
    let reserved_until = DateTime::parse_from_rfc3339("2023-01-15T09:23:50+08:00").unwrap();
    let warranty_until = DateTime::parse_from_rfc3339("2023-04-24T13:39:41+08:00").unwrap();
    let mocked_data = vec![
        (123u32, 124u64, ProductType::Package, 7u32, 173u32,
         25u32, 4u32, Some(reserved_until.clone()),
         reserved_until, warranty_until,)
    ];
    let oline_m = ut_setup_order_lines(mocked_data).remove(0);
    let ex_rate = CurrencyModel {
        name: CurrencyDto::TWD,
        rate: Decimal::new(4601809, 5),
    }; // this test case assumes seller labels prices in different currency
    let payline = oline_m.into_paym_dto(ex_rate);
    assert_eq!(payline.seller_id, 123);
    assert_eq!(payline.product_type, ProductType::Package);
    assert_eq!(payline.product_id, 124);
    assert_eq!(payline.quantity, 25);
    assert_eq!(payline.amount.unit.as_str(), "322.12"); // "322.12663000"
    assert_eq!(payline.amount.total.as_str(), "7961.12"); // "7961.12957000"
} // end of fn convert_to_pay_dto_ok

#[test]
fn gen_order_id_seq() {
    use std::collections::hash_map::RandomState;
    use std::collections::HashSet;
    let num_ids = 10;
    let machine_code = 1;
    let iter = (0..num_ids).into_iter().map(|_d| {
        let oid = OrderLineModel::generate_order_id(machine_code);
        // println!("generated ID : {oid}");
        oid
    });
    let hs: HashSet<String, RandomState> = HashSet::from_iter(iter);
    assert_eq!(hs.len(), num_ids);
}

#[test]
fn update_payments_ok() {
    let dt_now = LocalTime::now().fixed_offset();
    let reserved_until = dt_now + Duration::hours(1);
    let warranty_until = dt_now + Duration::days(1);
    let paid_last_update = dt_now - Duration::minutes(10);
    let seller_id = 123;
    #[cfg_attr(rustfmt, rustfmt_skip)]
    let mocked_data = vec![
        (seller_id, 812u64, ProductType::Package, 7u32, 69u32,
         10u32, 0u32, None, reserved_until, warranty_until),
        (seller_id, 890, ProductType::Item, 10, 90,
         9, 1, Some(paid_last_update), reserved_until, warranty_until,)
    ];
    let mut models = ut_setup_order_lines(mocked_data);
    let last_updates = [dt_now - Duration::minutes(3), dt_now - Duration::minutes(5)];
    let data = vec![
        OrderLinePaidUpdateDto {
            seller_id,
            product_id: 890,
            product_type: ProductType::Item,
            time: last_updates[1],
            qty: 7,
        },
        OrderLinePaidUpdateDto {
            seller_id,
            product_id: 812,
            product_type: ProductType::Package,
            time: last_updates[0],
            qty: 4,
        },
    ];
    let errors = OrderLineModel::update_payments(&mut models, data);
    assert_eq!(errors.len(), 0);
    assert_eq!(models[0].id_.product_id, 812);
    assert_eq!(models[0].qty.paid, 4);
    assert!(models[0].qty.paid_last_update.is_some());
    assert_eq!(
        models[0].qty.paid_last_update.as_ref().unwrap(),
        &last_updates[0]
    );
    assert_eq!(models[1].id_.product_id, 890);
    assert_eq!(models[1].qty.paid, 7);
    assert_eq!(
        models[1].qty.paid_last_update.as_ref().unwrap(),
        &last_updates[1]
    );
} // end of fn update_payments_ok

#[test]
fn update_payments_nonexist() {
    let dt_now = LocalTime::now().fixed_offset();
    let reserved_until = dt_now + Duration::hours(1);
    let warranty_until = dt_now + Duration::days(1);
    let paid_last_update = dt_now - Duration::minutes(10);
    let seller_id = 123;
    #[cfg_attr(rustfmt, rustfmt_skip)]
    let mocked_data = vec![
        (seller_id, 812u64, ProductType::Package, 7u32, 69u32,
         10u32, 0u32, None, reserved_until, warranty_until),
        (seller_id, 890, ProductType::Item, 10, 90,
         9, 1, Some(paid_last_update), reserved_until, warranty_until,)
    ];
    let mut models = ut_setup_order_lines(mocked_data);
    let last_updates = [dt_now - Duration::minutes(3), dt_now - Duration::minutes(5)];
    let data = vec![
        OrderLinePaidUpdateDto {
            seller_id,
            product_id: 889,
            product_type: ProductType::Item,
            time: last_updates[1],
            qty: 7,
        },
        OrderLinePaidUpdateDto {
            seller_id,
            product_id: 812,
            product_type: ProductType::Package,
            time: last_updates[0],
            qty: 4,
        },
    ];
    let errors = OrderLineModel::update_payments(&mut models, data);
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].product_id, 889);
    assert!(matches!(
        errors[0].reason,
        OrderLinePayUpdateErrorReason::NotExist
    ));
    assert_eq!(models[0].id_.product_id, 812);
    assert_eq!(models[0].qty.paid, 4);
    assert!(models[0].qty.paid_last_update.is_some());
} // end of fn update_payments_nonexist

#[test]
fn update_payments_rsv_expired() {
    let dt_now = LocalTime::now().fixed_offset();
    let warranty_until = dt_now + Duration::days(1);
    let paid_last_update = dt_now - Duration::minutes(10);
    let seller_id = 123;
    #[cfg_attr(rustfmt, rustfmt_skip)]
    let mocked_data = vec![
        (seller_id, 812u64, ProductType::Package, 7u32, 69u32,
         10u32, 0u32, None,
         dt_now + Duration::minutes(2), warranty_until),
        (seller_id, 890, ProductType::Item, 10, 90,
         9, 1, Some(paid_last_update),
         dt_now - Duration::seconds(30), warranty_until),
    ];
    let mut models = ut_setup_order_lines(mocked_data);
    let last_updates = [dt_now - Duration::minutes(3), dt_now - Duration::minutes(5)];
    let data = vec![
        OrderLinePaidUpdateDto {
            seller_id,
            product_id: 890,
            product_type: ProductType::Item,
            time: last_updates[1],
            qty: 7,
        },
        OrderLinePaidUpdateDto {
            seller_id,
            product_id: 812,
            product_type: ProductType::Package,
            time: last_updates[0],
            qty: 4,
        },
    ];
    let errors = OrderLineModel::update_payments(&mut models, data);
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].product_id, 890);
    assert!(matches!(
        errors[0].reason,
        OrderLinePayUpdateErrorReason::ReservationExpired
    ));
    assert_eq!(models[0].id_.product_id, 812);
    assert_eq!(models[0].qty.paid, 4);
    assert!(models[0].qty.paid_last_update.is_some());
    assert_eq!(
        models[0].qty.paid_last_update.as_ref().unwrap(),
        &last_updates[0]
    );
    assert_eq!(models[1].id_.product_id, 890);
    assert_eq!(models[1].qty.reserved, 9);
    assert_eq!(models[1].qty.paid, 1); // not modified
    assert_eq!(
        models[1].qty.paid_last_update.as_ref().unwrap(),
        &paid_last_update
    );
} // end of fn update_payments_rsv_expired

#[test]
fn update_payments_invalid_quantity() {
    let dt_now = LocalTime::now().fixed_offset();
    let reserved_until = dt_now + Duration::hours(1);
    let warranty_until = dt_now + Duration::days(1);
    let paid_last_update = dt_now - Duration::minutes(10);
    let seller_id = 123;
    #[cfg_attr(rustfmt, rustfmt_skip)]
    let mocked_data = vec![
        (seller_id, 812u64, ProductType::Package, 7u32, 69u32,
         10u32, 0u32, None, reserved_until, warranty_until),
        (seller_id, 890, ProductType::Item, 10, 90,
         9, 1, Some(paid_last_update), reserved_until, warranty_until,)
    ];
    let mut models = ut_setup_order_lines(mocked_data);
    let last_updates = [dt_now - Duration::minutes(3), dt_now - Duration::minutes(5)];
    let data = vec![
        OrderLinePaidUpdateDto {
            seller_id,
            product_id: 890,
            product_type: ProductType::Item,
            time: last_updates[1],
            qty: 8,
        },
        OrderLinePaidUpdateDto {
            seller_id,
            product_id: 812,
            product_type: ProductType::Package,
            time: last_updates[0],
            qty: models[0].qty.reserved + 1,
        },
    ];
    let errors = OrderLineModel::update_payments(&mut models, data);
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].product_id, 812);
    assert!(matches!(
        errors[0].reason,
        OrderLinePayUpdateErrorReason::InvalidQuantity
    ));
    assert_eq!(models[0].id_.product_id, 812);
    assert_eq!(models[0].qty.paid, 0); // not modified
    assert!(models[0].qty.paid_last_update.is_none());
    assert_eq!(models[1].id_.product_id, 890);
    assert_eq!(models[1].qty.paid, 8);
    assert_eq!(
        models[1].qty.paid_last_update.as_ref().unwrap(),
        &last_updates[1]
    );
} // end of fn update_payments_invalid_quantity

#[test]
fn update_payments_old_record_omitted() {
    let dt_now = LocalTime::now().fixed_offset();
    let reserved_until = dt_now + Duration::hours(1);
    let warranty_until = dt_now + Duration::days(1);
    let paid_last_update = dt_now - Duration::minutes(10);
    let seller_id = 123;
    #[cfg_attr(rustfmt, rustfmt_skip)]
    let mocked_data = vec![
        (seller_id, 812u64, ProductType::Package, 7u32, 69u32,
         10u32, 0u32, None, reserved_until, warranty_until),
        (seller_id, 890, ProductType::Item, 10, 90,
         9, 1, Some(paid_last_update), reserved_until, warranty_until,)
    ];
    let mut models = ut_setup_order_lines(mocked_data);
    let last_updates = [
        dt_now - Duration::minutes(1),
        paid_last_update - Duration::seconds(15),
    ];
    let data = vec![
        OrderLinePaidUpdateDto {
            seller_id,
            product_id: 890,
            product_type: ProductType::Item,
            time: last_updates[1],
            qty: 7,
        },
        OrderLinePaidUpdateDto {
            seller_id,
            product_id: 812,
            product_type: ProductType::Package,
            time: last_updates[0],
            qty: 4,
        },
    ];
    let errors = OrderLineModel::update_payments(&mut models, data);
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].product_id, 890);
    assert!(matches!(
        errors[0].reason,
        OrderLinePayUpdateErrorReason::Omitted
    ));
    assert_eq!(models[0].id_.product_id, 812);
    assert_eq!(models[0].qty.paid, 4);
    assert!(models[0].qty.paid_last_update.is_some());
    assert_eq!(
        models[0].qty.paid_last_update.as_ref().unwrap(),
        &last_updates[0]
    );
    assert_eq!(models[1].id_.product_id, 890);
    assert_eq!(models[1].qty.paid, 1); // not modified
    assert_eq!(
        models[1].qty.paid_last_update.as_ref().unwrap(),
        &paid_last_update
    );
} // end of fn update_payments_old_record_omitted
