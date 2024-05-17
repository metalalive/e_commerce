use chrono::{DateTime, Duration, Local as LocalTime};

use ecommerce_common::api::dto::OrderLinePayDto;
use ecommerce_common::constant::ProductType;
use ecommerce_common::error::AppErrorCode;

use order::api::rpc::dto::{OrderLinePaidUpdateDto, OrderLinePayUpdateErrorReason};
use order::api::web::dto::OrderLineReqDto;
use order::model::{
    OrderLineAppliedPolicyModel, OrderLineIdentity, OrderLineModel, OrderLinePriceModel,
    OrderLineQuantityModel, ProductPolicyModel, ProductPriceModel,
};

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

#[test]
fn convert_to_pay_dto_ok() {
    let reserved_until = DateTime::parse_from_rfc3339("2023-01-15T09:23:50+08:00").unwrap();
    let warranty_until = DateTime::parse_from_rfc3339("2023-04-24T13:39:41+08:00").unwrap();
    let m = OrderLineModel {
        id_: OrderLineIdentity {
            store_id: 123,
            product_id: 124,
            product_type: ProductType::Package,
        },
        price: OrderLinePriceModel {
            unit: 7,
            total: 173,
        },
        qty: OrderLineQuantityModel {
            reserved: 25,
            paid: 4,
            paid_last_update: Some(reserved_until.clone()),
        },
        policy: OrderLineAppliedPolicyModel {
            reserved_until,
            warranty_until,
        },
    };
    let payline: OrderLinePayDto = m.into();
    assert_eq!(payline.seller_id, 123);
    assert_eq!(payline.product_type, ProductType::Package);
    assert_eq!(payline.product_id, 124);
    assert_eq!(payline.quantity, 25);
    assert_eq!(payline.amount.unit, 7);
    assert_eq!(payline.amount.total, 173);
}

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
    let mut models = vec![
        OrderLineModel {
            id_: OrderLineIdentity {
                store_id: seller_id,
                product_id: 812,
                product_type: ProductType::Package,
            },
            price: OrderLinePriceModel { unit: 7, total: 69 },
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
            id_: OrderLineIdentity {
                store_id: seller_id,
                product_id: 890,
                product_type: ProductType::Item,
            },
            price: OrderLinePriceModel {
                unit: 10,
                total: 90,
            },
            qty: OrderLineQuantityModel {
                reserved: 9,
                paid: 1,
                paid_last_update: Some(paid_last_update),
            },
            policy: OrderLineAppliedPolicyModel {
                reserved_until,
                warranty_until,
            },
        },
    ];
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
    let mut models = vec![
        OrderLineModel {
            id_: OrderLineIdentity {
                store_id: seller_id,
                product_id: 812,
                product_type: ProductType::Package,
            },
            price: OrderLinePriceModel { unit: 7, total: 69 },
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
            id_: OrderLineIdentity {
                store_id: seller_id,
                product_id: 890,
                product_type: ProductType::Item,
            },
            price: OrderLinePriceModel {
                unit: 10,
                total: 90,
            },
            qty: OrderLineQuantityModel {
                reserved: 9,
                paid: 1,
                paid_last_update: Some(paid_last_update),
            },
            policy: OrderLineAppliedPolicyModel {
                reserved_until,
                warranty_until,
            },
        },
    ];
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
    let mut models = vec![
        OrderLineModel {
            id_: OrderLineIdentity {
                store_id: seller_id,
                product_id: 812,
                product_type: ProductType::Package,
            },
            price: OrderLinePriceModel { unit: 7, total: 69 },
            qty: OrderLineQuantityModel {
                reserved: 10,
                paid: 0,
                paid_last_update: None,
            },
            policy: OrderLineAppliedPolicyModel {
                reserved_until: dt_now + Duration::minutes(2),
                warranty_until,
            },
        },
        OrderLineModel {
            id_: OrderLineIdentity {
                store_id: seller_id,
                product_id: 890,
                product_type: ProductType::Item,
            },
            price: OrderLinePriceModel {
                unit: 10,
                total: 90,
            },
            qty: OrderLineQuantityModel {
                reserved: 9,
                paid: 1,
                paid_last_update: Some(paid_last_update),
            },
            policy: OrderLineAppliedPolicyModel {
                reserved_until: dt_now - Duration::seconds(30),
                warranty_until,
            },
        },
    ];
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
    let mut models = vec![
        OrderLineModel {
            id_: OrderLineIdentity {
                store_id: seller_id,
                product_id: 812,
                product_type: ProductType::Package,
            },
            price: OrderLinePriceModel { unit: 7, total: 69 },
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
            id_: OrderLineIdentity {
                store_id: seller_id,
                product_id: 890,
                product_type: ProductType::Item,
            },
            price: OrderLinePriceModel {
                unit: 10,
                total: 90,
            },
            qty: OrderLineQuantityModel {
                reserved: 9,
                paid: 1,
                paid_last_update: Some(paid_last_update),
            },
            policy: OrderLineAppliedPolicyModel {
                reserved_until,
                warranty_until,
            },
        },
    ];
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
    let mut models = vec![
        OrderLineModel {
            id_: OrderLineIdentity {
                store_id: seller_id,
                product_id: 812,
                product_type: ProductType::Package,
            },
            price: OrderLinePriceModel { unit: 7, total: 69 },
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
            id_: OrderLineIdentity {
                store_id: seller_id,
                product_id: 890,
                product_type: ProductType::Item,
            },
            price: OrderLinePriceModel {
                unit: 10,
                total: 90,
            },
            qty: OrderLineQuantityModel {
                reserved: 9,
                paid: 1,
                paid_last_update: Some(paid_last_update),
            },
            policy: OrderLineAppliedPolicyModel {
                reserved_until,
                warranty_until,
            },
        },
    ];
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
