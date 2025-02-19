use std::collections::HashMap;

use chrono::{DateTime, Duration, FixedOffset, Local as LocalTime};
use rust_decimal::Decimal;

use ecommerce_common::api::dto::CurrencyDto;
use ecommerce_common::api::rpc::dto::{OrderLinePaidUpdateDto, OrderLinePayUpdateErrorReason};
use ecommerce_common::error::AppErrorCode;

use order::api::dto::ProdAttrValueDto;
use order::api::web::dto::{OlineProductAttrDto, OrderLineReqDto};
use order::model::{
    CurrencyModel, OrderLineAppliedPolicyModel, OrderLineIdentity, OrderLineModel,
    OrderLinePriceModel, OrderLineQuantityModel, ProdAttriPriceModel, ProductPolicyModel,
    ProductPriceModel,
};

#[rustfmt::skip]
pub(super) fn ut_setup_order_lines(
    data : Vec<(
        u32, u64, u32, u32, u32, u32,
        Option<DateTime<FixedOffset>>,
        DateTime<FixedOffset>,
        DateTime<FixedOffset>
    )>
) -> Vec<OrderLineModel> {
    data.into_iter()
        .map(|d| {
            let id_ = OrderLineIdentity {store_id: d.0, product_id: d.1};
            let price= OrderLinePriceModel::from((d.2, d.3));
            let qty = OrderLineQuantityModel {
                reserved: d.4, paid: d.5, paid_last_update: d.6,
            };
            let policy = OrderLineAppliedPolicyModel {
                reserved_until: d.7, warranty_until: d.8,
            };
            let attr_lastupdate = d.7 - Duration::days(15);
            let attrs_charge = ProdAttriPriceModel::from((attr_lastupdate, None));
            OrderLineModel::from((id_, price, policy, qty, attrs_charge))
        })
        .collect::<Vec<_>>()
} // end of fn ut_setup_order_lines

#[test]
fn convert_from_req_dto_without_rsv_limit_ok() {
    let (seller_id, product_id) = (19, 146);
    let policym = ProductPolicyModel {
        product_id,
        is_create: false,
        auto_cancel_secs: 69,
        warranty_hours: 23,
        max_num_rsv: 0,
        min_num_rsv: 0,
    };
    let pricem = {
        let start_after = DateTime::parse_from_rfc3339("2023-07-31T10:16:54+05:00").unwrap();
        let end_before = DateTime::parse_from_rfc3339("2023-09-10T09:01:31+02:00").unwrap();
        let attr_lastupdate = DateTime::parse_from_rfc3339("2022-10-03T07:56:04+03:30").unwrap();
        let ts = [start_after, end_before, attr_lastupdate];
        ProductPriceModel::from((product_id, 1015, ts, None))
    };
    let data = OrderLineReqDto {
        seller_id,
        product_id,
        quantity: 26,
        applied_attr: None,
    };
    let result = OrderLineModel::try_from(data, &policym, &pricem);
    let m = result.unwrap();
    assert_eq!(m.price().unit(), 1015u32);
    assert_eq!(m.price().total(), 1015u32 * 26u32);
    assert_eq!(m.qty.reserved, 26);
    let timenow = LocalTime::now().fixed_offset();
    let expect_reserved_time = timenow + Duration::seconds(69i64);
    assert!(m.policy.reserved_until <= expect_reserved_time);
}

#[test]
fn convert_from_req_dto_with_rsv_limit_ok() {
    let (seller_id, product_id) = (19, 146);
    let policym = ProductPolicyModel {
        product_id,
        is_create: false,
        auto_cancel_secs: 69,
        warranty_hours: 23,
        max_num_rsv: 10,
        min_num_rsv: 2,
    };
    let pricem = {
        let start_after = DateTime::parse_from_rfc3339("2022-10-28T10:16:54+05:00").unwrap();
        let end_before = DateTime::parse_from_rfc3339("2022-10-31T06:11:50+02:00").unwrap();
        let attr_lastupdate = DateTime::parse_from_rfc3339("2022-10-03T07:56:04+03:30").unwrap();
        let ts = [start_after, end_before, attr_lastupdate];
        ProductPriceModel::from((product_id, 987, ts, None))
    };
    let data = OrderLineReqDto {
        seller_id,
        product_id,
        quantity: 9,
        applied_attr: None,
    };
    let result = OrderLineModel::try_from(data, &policym, &pricem);
    let m = result.unwrap();
    assert_eq!(m.price().unit(), 987u32);
    assert_eq!(m.price().total(), 987u32 * 9u32);
    assert_eq!(m.qty.reserved, 9u32);
}

#[test]
fn convert_from_req_dto_violate_rsv_limit() {
    let (seller_id, product_id) = (19, 146);
    let policym = ProductPolicyModel {
        product_id,
        is_create: false,
        auto_cancel_secs: 180,
        warranty_hours: 48,
        max_num_rsv: 10,
        min_num_rsv: 0,
    };
    let pricem = {
        let start_after = DateTime::parse_from_rfc3339("2022-10-28T10:16:54+05:00").unwrap();
        let end_before = DateTime::parse_from_rfc3339("2022-10-31T06:11:50+02:00").unwrap();
        let attr_lastupdate = DateTime::parse_from_rfc3339("2022-10-03T07:56:04+03:30").unwrap();
        let ts = [start_after, end_before, attr_lastupdate];
        ProductPriceModel::from((product_id, 987, ts, None))
    };
    let data = OrderLineReqDto {
        seller_id,
        product_id,
        quantity: 11,
        applied_attr: None,
    };
    let result = OrderLineModel::try_from(data, &policym, &pricem);
    assert!(result.is_err());
    if let Err(e) = result {
        assert_eq!(e.code, AppErrorCode::ExceedingMaxLimit);
    }
}

#[test]
fn convert_from_req_with_attributes_ok() {
    let (seller_id, product_id) = (19, 146);
    let policym = ProductPolicyModel {
        product_id,
        is_create: false,
        auto_cancel_secs: 69,
        warranty_hours: 23,
        max_num_rsv: 29,
        min_num_rsv: 5,
    };
    let pricem = {
        let start_after = DateTime::parse_from_rfc3339("2022-10-28T10:16:54+05:00").unwrap();
        let end_before = DateTime::parse_from_rfc3339("2022-10-31T06:11:50+02:00").unwrap();
        let attr_lastupdate = DateTime::parse_from_rfc3339("2022-10-03T07:56:04+03:30").unwrap();
        let ts = [start_after, end_before, attr_lastupdate];
        let attrmap = HashMap::from([
            ("Foxn-87".to_string(), 3),
            ("pln9-995".to_string(), 17),
            ("bonb-154".to_string(), 19),
        ]);
        ProductPriceModel::from((product_id, 487, ts, Some(attrmap)))
    };
    [
        (Vec::new(), 487u32),
        (vec![("Foxn", 87), ("pln9", 995)], 507),
        (vec![("Foxn", 87), ("bonb", 154)], 509),
        (vec![("pln9", 995), ("bonb", 154)], 523),
        (vec![("Foxn", 87), ("pln9", 995), ("bonb", 154)], 526),
    ]
    .into_iter()
    .map(|raw| {
        let (ds, expect_unitprice) = raw;
        let applied_attr = ds
            .into_iter()
            .map(|v| OlineProductAttrDto {
                label_id: v.0.to_string(),
                value: ProdAttrValueDto::Int(v.1),
            })
            .collect::<Vec<_>>();
        let data = OrderLineReqDto {
            seller_id,
            product_id,
            quantity: 11,
            applied_attr: Some(applied_attr),
        };
        let result = OrderLineModel::try_from(data, &policym, &pricem);
        let m = result.unwrap();
        assert_eq!(m.price().unit(), expect_unitprice);
        assert_eq!(m.price().total(), expect_unitprice * 11u32);
        assert_eq!(m.qty.reserved, 11u32);
    })
    .count();
} // end of fn convert_from_req_with_attributes_ok

#[test]
fn convert_from_req_with_attributes_error() {
    let (seller_id, product_id) = (19, 146);
    let policym = ProductPolicyModel {
        product_id,
        is_create: false,
        auto_cancel_secs: 69,
        warranty_hours: 23,
        max_num_rsv: 29,
        min_num_rsv: 5,
    };
    let pricem = {
        let start_after = DateTime::parse_from_rfc3339("2022-10-28T10:16:54+05:00").unwrap();
        let end_before = DateTime::parse_from_rfc3339("2022-10-31T06:11:50+02:00").unwrap();
        let attr_lastupdate = DateTime::parse_from_rfc3339("2022-10-03T07:56:04+03:30").unwrap();
        let ts = [start_after, end_before, attr_lastupdate];
        let attrmap = HashMap::from([
            ("aNzo-871".to_string(), 5),
            ("pln9-995".to_string(), i32::MAX),
            ("lama-230".to_string(), i32::MIN),
            ("jucy-319".to_string(), -50000),
        ]);
        ProductPriceModel::from((product_id, 350, ts, Some(attrmap)))
    };
    [
        (
            vec![("aNzo", 871), ("xxxx", 995)],
            AppErrorCode::InvalidInput,
        ),
        (
            vec![("aNzo", 871), ("pln9", 995)],
            AppErrorCode::DataCorruption,
        ),
        (vec![("pln9", 995)], AppErrorCode::DataCorruption),
        (
            vec![("lama", 230), ("jucy", 319)],
            AppErrorCode::DataCorruption,
        ),
    ]
    .into_iter()
    .map(|raw| {
        let (ds, expect_ecode) = raw;
        let applied_attr = ds
            .into_iter()
            .map(|v| OlineProductAttrDto {
                label_id: v.0.to_string(),
                value: ProdAttrValueDto::Int(v.1),
            })
            .collect::<Vec<_>>();
        let data = OrderLineReqDto {
            seller_id,
            product_id,
            quantity: 11,
            applied_attr: Some(applied_attr),
        };
        let result = OrderLineModel::try_from(data, &policym, &pricem);
        assert!(result.is_err());
        if let Err(e) = result {
            assert_eq!(e.code, expect_ecode);
        }
    })
    .count();
} // end of fn convert_from_req_with_attributes_error

#[test]
fn convert_from_req_dto_product_id_mismatch() {
    let (seller_id, product_id) = (19, 146);
    let policym = ProductPolicyModel {
        product_id,
        is_create: false,
        auto_cancel_secs: 180,
        warranty_hours: 48,
        max_num_rsv: 10,
        min_num_rsv: 0,
    };
    let pricem = {
        let start_after = DateTime::parse_from_rfc3339("2022-10-28T10:16:54+05:00").unwrap();
        let end_before = DateTime::parse_from_rfc3339("2022-10-31T06:11:50+02:00").unwrap();
        let attr_lastupdate = DateTime::parse_from_rfc3339("2022-10-03T07:56:04+03:30").unwrap();
        let ts = [start_after, end_before, attr_lastupdate];
        ProductPriceModel::from((1466, 60, ts, None))
    };
    let data = OrderLineReqDto {
        seller_id,
        product_id,
        quantity: 2,
        applied_attr: None,
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
        (123u32, 124u64, 7u32, 173u32, 25u32, 4u32,
         Some(reserved_until.clone()),  reserved_until, warranty_until,)
    ];
    let oline_m = ut_setup_order_lines(mocked_data).remove(0);
    let ex_rate = CurrencyModel {
        name: CurrencyDto::TWD,
        rate: Decimal::new(4601809, 5),
    }; // this test case assumes seller labels prices in different currency
    let payline = oline_m.into_paym_dto(ex_rate);
    assert_eq!(payline.seller_id, 123);
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

#[rustfmt::skip]
#[test]
fn update_payments_ok() {
    let dt_now = LocalTime::now().fixed_offset();
    let reserved_until = dt_now + Duration::hours(1);
    let warranty_until = dt_now + Duration::days(1);
    let paid_last_update = dt_now - Duration::minutes(10);
    let seller_id = 123;
    let mocked_data = vec![
        (seller_id, 812u64, 7u32, 69u32, 10u32, 0u32, None, reserved_until, warranty_until),
        (seller_id, 890, 10, 90, 9, 1, Some(paid_last_update), reserved_until, warranty_until,)
    ];
    let mut models = ut_setup_order_lines(mocked_data);
    let d_lines = [
            (seller_id, 890u64, 7u32),
            (seller_id, 812, 4),
        ].into_iter()
        .map(|d| OrderLinePaidUpdateDto {
            seller_id: d.0, product_id:  d.1, qty: d.2,
        })
        .collect::<Vec<_>>();
    let d_charge_time = [
        paid_last_update + Duration::minutes(5),
        paid_last_update + Duration::minutes(6),
    ];
    let errors = OrderLineModel::update_payments(&mut models, d_lines, d_charge_time[0]);
    assert_eq!(errors.len(), 0);
    models.iter().map(|m| {
        let expect = match m.id().product_id {
            812u64 => (4u32, d_charge_time[0]),
            890 => (8, d_charge_time[0]),
            _others => (99999, paid_last_update),
        };
        assert_eq!(m.qty.paid, expect.0);
        assert!(m.qty.paid_last_update.is_some());
        assert_eq!(m.qty.paid_last_update.as_ref().unwrap(), &expect.1);
    }).count();
    
    let d_lines = vec![
            (seller_id, 812u64, 1u32),
        ].into_iter()
        .map(|d| OrderLinePaidUpdateDto {
            seller_id: d.0, product_id:  d.1, qty: d.2,
        })
        .collect::<Vec<_>>();
    let errors = OrderLineModel::update_payments(&mut models, d_lines, d_charge_time[1]);
    assert_eq!(errors.len(), 0);
    models.iter().map(|m| {
        let expect = match m.id().product_id {
            812u64 => (5u32, d_charge_time[1]),
            890 => (8, d_charge_time[0]),
            _others => (99999, paid_last_update),
        };
        assert_eq!(m.qty.paid, expect.0);
        assert!(m.qty.paid_last_update.is_some());
        assert_eq!(m.qty.paid_last_update.as_ref().unwrap(), &expect.1);
    }).count();
} // end of fn update_payments_ok

#[rustfmt::skip]
#[test]
fn update_payments_nonexist() {
    let dt_now = LocalTime::now().fixed_offset();
    let reserved_until = dt_now + Duration::hours(1);
    let warranty_until = dt_now + Duration::days(1);
    let paid_last_update = dt_now - Duration::minutes(10);
    let seller_id = 123;
    let mocked_data = vec![
        (seller_id, 812u64, 7u32, 69u32, 10u32, 0u32, None, reserved_until, warranty_until),
        (seller_id, 890, 10, 90, 9, 1, Some(paid_last_update), reserved_until, warranty_until,)
    ];
    let mut models = ut_setup_order_lines(mocked_data);
    let d_lines = [
            (seller_id, 889u64, 7u32),
            (seller_id, 812, 4),
        ].into_iter()
        .map(|d| OrderLinePaidUpdateDto {
            seller_id: d.0, product_id:  d.1, qty: d.2,
        })
        .collect::<Vec<_>>();
    let d_charge_time = paid_last_update + Duration::minutes(5);
    let errors = OrderLineModel::update_payments(&mut models, d_lines, d_charge_time);
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].product_id, 889);
    assert!(matches!(errors[0].reason, OrderLinePayUpdateErrorReason::NotExist));
    assert_eq!(models[0].id().product_id, 812);
    assert_eq!(models[0].qty.paid, 4);
    assert!(models[0].qty.paid_last_update.is_some());
} // end of fn update_payments_nonexist

#[rustfmt::skip]
#[test]
fn update_payments_invalid_quantity() {
    let dt_now = LocalTime::now().fixed_offset();
    let reserved_until = dt_now + Duration::hours(1);
    let warranty_until = dt_now + Duration::days(1);
    let paid_last_update = dt_now - Duration::minutes(10);
    let seller_id = 123;
    let mocked_data = vec![
        (seller_id, 812u64, 7u32, 69u32, 10u32, 0u32, None, reserved_until, warranty_until),
        (seller_id, 890, 10, 90, 9, 1, Some(paid_last_update), reserved_until, warranty_until),
    ];
    let mut models = ut_setup_order_lines(mocked_data);
    let d_lines = [
            (seller_id, 890u64, 8u32),
            (seller_id, 812, models[0].qty.reserved + 1),
        ].into_iter()
        .map(|d| OrderLinePaidUpdateDto {
            seller_id: d.0, product_id:  d.1, qty: d.2,
        })
        .collect::<Vec<_>>();
    let d_charge_time = paid_last_update + Duration::minutes(2);
    let errors = OrderLineModel::update_payments(&mut models, d_lines, d_charge_time);
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].product_id, 812);
    assert!(matches!(errors[0].reason, OrderLinePayUpdateErrorReason::InvalidQuantity));
    assert_eq!(models[0].id().product_id, 812);
    assert_eq!(models[0].qty.paid, 0); // not modified
    assert!(models[0].qty.paid_last_update.is_none());
    assert_eq!(models[1].id().product_id, 890);
    assert_eq!(models[1].qty.paid, 9);
    assert_eq!(models[1].qty.paid_last_update.as_ref().unwrap(), &d_charge_time);
} // end of fn update_payments_invalid_quantity

#[test]
fn update_payments_old_record_omitted() {
    let dt_now = LocalTime::now().fixed_offset();
    let reserved_until = dt_now + Duration::hours(1);
    let warranty_until = dt_now + Duration::days(1);
    let seller_id = 123;
    #[rustfmt::skip]
    let mocked_data = vec![
        (
            seller_id, 812u64, 7u32, 69u32, 10u32, 0u32,
            Some(dt_now + Duration::minutes(8)), reserved_until, warranty_until,
        ),
        (
            seller_id, 890, 10, 90, 9, 1,
            Some(dt_now + Duration::minutes(10)), reserved_until, warranty_until,
        ),
    ];
    let mut models = ut_setup_order_lines(mocked_data);
    let d_lines = [(seller_id, 890u64, 7u32), (seller_id, 812, 4)]
        .into_iter()
        .map(|d| OrderLinePaidUpdateDto {
            seller_id: d.0,
            product_id: d.1,
            qty: d.2,
        })
        .collect::<Vec<_>>();
    let d_charge_time = dt_now + Duration::minutes(9);
    let errors = OrderLineModel::update_payments(&mut models, d_lines, d_charge_time);
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].product_id, 890);
    assert!(matches!(
        errors[0].reason,
        OrderLinePayUpdateErrorReason::Omitted
    ));
    assert_eq!(models[0].id().product_id, 812);
    assert_eq!(models[0].qty.paid, 4);
    assert!(models[0].qty.paid_last_update.is_some());
    assert_eq!(
        models[0].qty.paid_last_update.as_ref().unwrap(),
        &d_charge_time
    );
    assert_eq!(models[1].id().product_id, 890);
    assert_eq!(models[1].qty.paid, 1); // not modified
    assert_eq!(
        models[1].qty.paid_last_update.as_ref().unwrap(),
        &(dt_now + Duration::minutes(10))
    );
} // end of fn update_payments_old_record_omitted
