use chrono::{DateTime, Duration, Local as LocalTime};
use order::api::dto::OrderLinePayDto;
use order::api::web::dto::OrderLineReqDto;
use order::constant::ProductType;
use order::model::{
    OrderLineModel, ProductPolicyModel, ProductPriceModel, OrderLinePriceModel,
    OrderLineAppliedPolicyModel
};

#[test]
fn convert_from_req_dto_ok()
{
    let (seller_id, product_id, product_type) = (19, 146, ProductType::Item);
    let policym = ProductPolicyModel { product_type:product_type.clone(), product_id,
            is_create: false, auto_cancel_secs: 69, warranty_hours: 23
    };
    let pricem  = ProductPriceModel { product_id, product_type:product_type.clone(),
            price: 1015, is_create: false,
            start_after:DateTime::parse_from_rfc3339("2023-07-31T10:16:54+05:00").unwrap().into(),
            end_before:DateTime::parse_from_rfc3339("2023-09-10T09:01:31+02:00").unwrap().into()
    };
    let data = OrderLineReqDto { seller_id, product_id, product_type, quantity:26 };
    let m = OrderLineModel::from(data, &policym, &pricem);
    assert_eq!(m.price.unit, 1015u32);
    assert_eq!(m.price.total, 1015u32 * 26u32);
    assert_eq!(m.qty, 26);
    let timenow = LocalTime::now().fixed_offset();
    let expect_reserved_time = timenow + Duration::seconds(69i64);
    assert!(m.policy.reserved_until <= expect_reserved_time);
}

#[test]
fn convert_to_pay_dto_ok()
{
    let reserved_until = DateTime::parse_from_rfc3339("2023-01-15T09:23:50+08:00").unwrap();
    let warranty_until = DateTime::parse_from_rfc3339("2023-04-24T13:39:41+08:00").unwrap();
    let m = OrderLineModel {seller_id:123, product_type:ProductType::Package,
        product_id:124, qty:25, price:OrderLinePriceModel { unit: 7, total: 173 },
        policy: OrderLineAppliedPolicyModel { reserved_until, warranty_until }
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
    use std::collections::HashSet;
    use std::collections::hash_map::RandomState;
    let num_ids = 10;
    let machine_code = 1;
    let iter = (0 .. num_ids).into_iter().map(|_d| {
        let oid = OrderLineModel::generate_order_id(machine_code);
        // println!("generated ID : {oid}");
        oid
    });
    let hs : HashSet<String, RandomState> = HashSet::from_iter(iter);
    assert_eq!(hs.len(), num_ids);
}

