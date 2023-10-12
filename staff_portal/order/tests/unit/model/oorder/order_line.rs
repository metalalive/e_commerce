use chrono::{DateTime, Duration, Local as LocalTime};
use order::api::web::dto::OrderLineReqDto;
use order::constant::ProductType;
use order::model::{OrderLineModel, ProductPolicyModel, ProductPriceModel};

#[test]
fn convert_dto_ok()
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
    assert_eq!(m.price, 1015);
    assert_eq!(m.qty, 26);
    let timenow = LocalTime::now().fixed_offset();
    let expect_reserved_time = timenow + Duration::seconds(69i64);
    assert!(m.policy.reserved_until <= expect_reserved_time);
}

