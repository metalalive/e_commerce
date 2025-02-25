use std::collections::HashMap;

use chrono::{DateTime, Duration, FixedOffset, Local as LocalTime};

use ecommerce_common::api::dto::CurrencyDto;
use ecommerce_common::api::rpc::dto::{OrderLinePaidUpdateDto, OrderLinePayUpdateErrorReason};
use ecommerce_common::error::AppErrorCode;

use order::api::dto::ProdAttrValueDto;
use order::api::web::dto::{
    OlineProductAttrDto, OrderCreateRespOkDto, OrderLineCreateErrorDto, OrderLineCreateErrorReason,
    OrderLineReqDto,
};
use order::model::{
    OrderLineAppliedPolicyModel, OrderLineIdentity, OrderLineModel, OrderLineModelSet,
    OrderLinePriceModel, OrderLineQuantityModel, ProdAttriPriceModel, ProductPolicyModel,
    ProductPriceModel,
};

use super::currency::ut_common_order_currency;

#[rustfmt::skip]
pub(super) fn ut_setup_order_lines(
    data : Vec<(
        (u32, u64, u16), (u32, u32), u32, u32,
        Option<DateTime<FixedOffset>>,
        DateTime<FixedOffset>,
        DateTime<FixedOffset>,
        Option<HashMap<String, i32>>,
    )>
) -> Vec<OrderLineModel> {
    data.into_iter()
        .map(|d| {
            let id_ = OrderLineIdentity::from(d.0);
            let price= OrderLinePriceModel::from(d.1);
            let qty = OrderLineQuantityModel {
                reserved: d.2, paid: d.3, paid_last_update: d.4,
            };
            let policy = OrderLineAppliedPolicyModel {
                reserved_until: d.5, warranty_until: d.6,
            };
            let attr_lastupdate = d.5 - Duration::days(15);
            let attrs_charge = ProdAttriPriceModel::from((attr_lastupdate, d.7));
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
    let rsved_until = dt_now + Duration::hours(1);
    let warranty_until = dt_now + Duration::days(1);
    let paid_last_update = dt_now - Duration::minutes(10);
    let seller_id = 123;
    let mocked_data = vec![
        ((seller_id, 812u64, 0u16), (7u32, 69u32), 10u32, 0u32, None, rsved_until, warranty_until, None),
        ((seller_id, 890, 0), (10, 90), 9, 1, Some(paid_last_update), rsved_until, warranty_until, None),
        ((seller_id, 890, 1), (11, 77), 7, 2, Some(paid_last_update), rsved_until, warranty_until, None),
        ((seller_id, 890, 2), (13, 156), 12, 0, None, rsved_until, warranty_until, None),
    ];
    let mut models = ut_setup_order_lines(mocked_data);
    let d_lines = [
            (seller_id, 890u64, 0u16, 7u32),
            (seller_id, 812, 0, 4),
        ].into_iter()
        .map(|d| OrderLinePaidUpdateDto {
            seller_id: d.0, product_id:  d.1, qty: d.3,
            attr_set_seq: d.2,
        })
        .collect::<Vec<_>>();
    let d_charge_time = [
        paid_last_update + Duration::minutes(5),
        paid_last_update + Duration::minutes(6),
    ];
    let errors = OrderLineModel::update_payments(&mut models, d_lines, d_charge_time[0]);
    assert_eq!(errors.len(), 0);
    models.iter().map(|m| {
        let combo = (m.id().product_id(), m.id().attrs_seq_num());
        let expect = match combo {
            (812u64, 0u16) => (4u32, Some(d_charge_time[0])),
            (890, 0) => (8, Some(d_charge_time[0])),
            (890, 1) => (2, Some(paid_last_update)),
            (890, 2) => (0, None),
            _others => (99999, None),
        };
        let actual = (m.qty.paid, m.qty.paid_last_update);
        assert_eq!(actual, expect);
    }).count();
    
    let d_lines = [
            (seller_id, 812, 0, 1),
            (seller_id, 890, 1, 1),
            (seller_id, 890, 2, 5),
        ].into_iter()
        .map(|d| OrderLinePaidUpdateDto {
            seller_id: d.0, product_id:  d.1, qty: d.3,
            attr_set_seq: d.2,
        })
        .collect::<Vec<_>>();
    let errors = OrderLineModel::update_payments(&mut models, d_lines, d_charge_time[1]);
    assert_eq!(errors.len(), 0);
    models.iter().map(|m| {
        let combo = (m.id().product_id(), m.id().attrs_seq_num());
        let expect = match combo {
            (812u64, 0u16) => (5u32, d_charge_time[1]),
            (890, 0) => (8, d_charge_time[0]),
            (890, 1) => (3, d_charge_time[1]),
            (890, 2) => (5, d_charge_time[1]),
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
        ((seller_id, 812u64, 0u16), (7u32, 69u32), 10u32, 0u32, None, reserved_until, warranty_until, None),
        ((seller_id, 890, 0), (10, 90), 9, 1, Some(paid_last_update), reserved_until, warranty_until, None)
    ];
    let mut models = ut_setup_order_lines(mocked_data);
    let d_lines = [
            (seller_id, 890u64, 2u16, 7u32),
            (seller_id, 812, 0, 4),
        ].into_iter()
        .map(|d| OrderLinePaidUpdateDto {
            seller_id: d.0, product_id:  d.1, qty: d.3,
            attr_set_seq: d.2,
        })
        .collect::<Vec<_>>();
    let d_charge_time = paid_last_update + Duration::minutes(5);
    let errors = OrderLineModel::update_payments(&mut models, d_lines, d_charge_time);
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].product_id, 890);
    assert_eq!(errors[0].attr_set_seq, 2);
    assert!(matches!(errors[0].reason, OrderLinePayUpdateErrorReason::NotExist));
    assert_eq!(models[0].id().product_id(), 812);
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
        ((seller_id, 812u64, 0u16), (8u32, 80u32), 10u32, 0u32, None, reserved_until, warranty_until, None),
        ((seller_id, 812, 1), (7, 69), 10, 0, None, reserved_until, warranty_until, None),
        ((seller_id, 890, 0), (10, 90), 9, 1, Some(paid_last_update), reserved_until, warranty_until, None),
    ];
    let mut models = ut_setup_order_lines(mocked_data);
    let d_lines = [
            (seller_id, 890u64, 0u16, 8u32),
            (seller_id, 812, 1, models[0].qty.reserved + 1),
        ].into_iter()
        .map(|d| OrderLinePaidUpdateDto {
            seller_id: d.0, product_id:  d.1, qty: d.3,
            attr_set_seq: d.2,
        })
        .collect::<Vec<_>>();
    let d_charge_time = paid_last_update + Duration::minutes(2);
    let errors = OrderLineModel::update_payments(&mut models, d_lines, d_charge_time);
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].product_id, 812);
    assert_eq!(errors[0].attr_set_seq, 1);
    assert!(matches!(errors[0].reason, OrderLinePayUpdateErrorReason::InvalidQuantity));
    assert_eq!(models[1].id().product_id(), 812);
    assert_eq!(models[1].id().attrs_seq_num(), 1);
    assert_eq!(models[1].qty.paid, 0); // not modified
    assert!(models[0].qty.paid_last_update.is_none());
    assert_eq!(models[2].id().product_id(), 890);
    assert_eq!(models[2].qty.paid, 9);
    assert_eq!(models[2].qty.paid_last_update.as_ref().unwrap(), &d_charge_time);
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
            (seller_id, 812u64, 0u16), (7u32, 69u32), 10u32, 0u32, Some(dt_now + Duration::minutes(8)),
            reserved_until, warranty_until, None,
        ),
        (
            (seller_id, 890, 0), (10, 90), 9, 1, Some(dt_now + Duration::minutes(10)),
            reserved_until, warranty_until, None,
        ),
    ];
    let mut models = ut_setup_order_lines(mocked_data);
    let d_lines = [(seller_id, 890, 0, 7), (seller_id, 812, 0, 4)]
        .into_iter()
        .map(|d| OrderLinePaidUpdateDto {
            seller_id: d.0,
            product_id: d.1,
            attr_set_seq: d.2,
            qty: d.3,
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
    assert_eq!(models[0].id().product_id(), 812);
    assert_eq!(models[0].qty.paid, 4);
    assert!(models[0].qty.paid_last_update.is_some());
    assert_eq!(
        models[0].qty.paid_last_update.as_ref().unwrap(),
        &d_charge_time
    );
    assert_eq!(models[1].id().product_id(), 890);
    assert_eq!(models[1].qty.paid, 1); // not modified
    assert_eq!(
        models[1].qty.paid_last_update.as_ref().unwrap(),
        &(dt_now + Duration::minutes(10))
    );
} // end of fn update_payments_old_record_omitted

#[rustfmt::skip]
fn convert_to_olset_common() -> OrderLineModelSet {
    let create_time = LocalTime::now().fixed_offset();
    let reserved_t = create_time + Duration::hours(1);
    let warranty_t = create_time + Duration::days(1);
    let mock_usr_id = 299;
    let mock_seller_ids = [123, 124, 125];
    let prod_attr_set_1 = HashMap::from([
        ("4g0t".to_string(), 1),
        ("ejsio".to_string(), 5),
    ]);
    let prod_attr_set_2 = HashMap::from([("4g0t".to_string(), 1)]);
    let prod_attr_set_3 = HashMap::from([("om3n".to_string(), 3)]);
    let mocked_linedata = vec![
        ((mock_seller_ids[0], 812u64, 0u16), (7u32, 69u32), 10u32, 0u32, None, reserved_t, warranty_t, None),
        ((mock_seller_ids[0], 812, 0), (13, 117), 9, 1, None, reserved_t, warranty_t, Some(prod_attr_set_1)),
        ((mock_seller_ids[0], 813, 0), (11, 110), 10, 1, None, reserved_t, warranty_t, None),
        ((mock_seller_ids[1], 890, 0), (10, 90), 9, 1, None, reserved_t, warranty_t, None),
        ((mock_seller_ids[1], 895, 0), (8, 56), 7, 1, None, reserved_t, warranty_t, None),
        ((mock_seller_ids[2], 451, 0), (12, 240), 20, 1, None, reserved_t, warranty_t, None),
        ((mock_seller_ids[2], 451, 0), (13, 13), 1, 1, None, reserved_t, warranty_t, Some(prod_attr_set_2)),
        ((mock_seller_ids[2], 451, 0), (15, 90), 6, 1, None, reserved_t, warranty_t, Some(prod_attr_set_3)),
        ((mock_seller_ids[2], 452, 0), (7, 98), 14, 1, None, reserved_t, warranty_t, None),
    ];
    let olines = ut_setup_order_lines(mocked_linedata);
    let oid = "allahbomarcasm".to_string();
    let currency = ut_common_order_currency(mock_seller_ids);
    let args = (oid, mock_usr_id, create_time, currency, olines);
    let result = OrderLineModelSet::try_from(args);
    let olset = result.unwrap();
    assert_eq!(olset.id().as_str(), "allahbomarcasm");
    assert_eq!(olset.owner(), mock_usr_id);
    assert_eq!(olset.lines().len(), 9);
    olset
} // end of fn convert_to_olset_common

#[rustfmt::skip]
#[test]
fn convert_to_pay_dto_ok() {
    let olset = convert_to_olset_common();
    let result = OrderCreateRespOkDto::try_from(olset);
    assert!(result.is_ok());
    if let Ok(dto) = result {
        assert_eq!(dto.order_id.as_str(), "allahbomarcasm");
        assert_eq!(dto.usr_id, 299u32);
        let expect = dto.currency.sellers.iter().find(|c| c.seller_id == 123u32).unwrap();
        assert_eq!(expect.currency, CurrencyDto::TWD);
        let expect = dto.currency.sellers.iter().find(|c| c.seller_id == 125u32).unwrap();
        assert_eq!(expect.currency, CurrencyDto::INR);
        let expect = dto.currency.snapshot.iter().find(|c| c.name == CurrencyDto::INR).unwrap();
        assert_eq!(expect.rate.as_str(), "83.4095");
        dto.reserved_lines.iter().map(|l| {
            let combo = (l.seller_id, l.product_id, l.attr_set_seq);
            let option_chk: Option<(u32, &str)> = match combo {
                (125, 451, 0) => Some((20, "4.61")),
                (125, 451, 1) => Some((1, "4.99")),
                (125, 451, 2) => Some((6, "5.76")),
                (123, 812, 0) => Some((10, "7.00")),
                (123, 812, 1) => Some((9, "13.00")),
                _others => None,
            };
            if let Some(expect) = option_chk {
                let actual = (l.quantity, l.amount.unit.as_str());
                assert_eq!(expect, actual);
            }
        }).count();
    }
} // end of fn convert_to_pay_dto_ok

#[rustfmt::skip]
#[test]
fn convert_to_olset_dup_error() {
    let create_time = LocalTime::now().fixed_offset();
    let reserved_t = create_time + Duration::hours(1);
    let warranty_t = create_time + Duration::days(1);
    let mock_usr_id = 299;
    let mock_seller_ids = [123, 124, 125];
    let prod_attr_set_1 = HashMap::from([
        ("4g0t".to_string(), 1),
        ("ejsio".to_string(), 5),
    ]);
    let prod_attr_set_2 = HashMap::from([("4g0t".to_string(), 1)]);
    let prod_attr_set_3 = HashMap::from([("om3n".to_string(), 3)]);
    let mocked_linedata = vec![
        ((mock_seller_ids[0], 812u64, 0u16), (7u32, 69u32), 10u32, 0u32, None, reserved_t, warranty_t, None),
        ((mock_seller_ids[0], 812, 0), (13, 117), 9, 1, None, reserved_t, warranty_t, Some(prod_attr_set_1)),
        ((mock_seller_ids[0], 812, 0), (7,  68), 10, 0, None, reserved_t, warranty_t, None),
        ((mock_seller_ids[0], 813, 0), (11, 110), 10, 1, None, reserved_t, warranty_t, None),
        ((mock_seller_ids[0], 814, 0), (8, 56), 7, 1, None, reserved_t, warranty_t, None),
        ((mock_seller_ids[1], 890, 0), (10, 90), 9, 1, None, reserved_t, warranty_t, None),
        ((mock_seller_ids[1], 895, 0), (8, 56), 7, 1, None, reserved_t, warranty_t, None),
        ((mock_seller_ids[2], 451, 0), (12, 240), 20, 1, None, reserved_t, warranty_t, Some(prod_attr_set_2.clone())),
        ((mock_seller_ids[2], 451, 0), (13, 13), 1, 1, None, reserved_t, warranty_t, Some(prod_attr_set_2)),
        ((mock_seller_ids[2], 451, 0), (15, 90), 6, 1, None, reserved_t, warranty_t, Some(prod_attr_set_3)),
        ((mock_seller_ids[2], 452, 0), (7, 98), 14, 1, None, reserved_t, warranty_t, None),
    ];
    let olines = ut_setup_order_lines(mocked_linedata);
    let order_id = "allahbomarcasm".to_string();
    let currency = ut_common_order_currency(mock_seller_ids);
    let args = (order_id, mock_usr_id, create_time, currency, olines);
    let result = OrderLineModelSet::try_from(args);
    assert!(result.is_err());
    if let Err(es) = result {
        assert_eq!(es.len(), 2);
        let ds = es.into_iter().map(OrderLineCreateErrorDto::from).collect::<Vec<_>>();
        let cond = matches!(ds[0].reason, OrderLineCreateErrorReason::DuplicateLines);
        assert!(cond);
        let expect: [(u32,u64); 2] = [(123, 812), (125, 451)];
        let actual = (ds[0].seller_id, ds[0].product_id);
        assert!(expect.contains(&actual));
        let actual = (ds[1].seller_id, ds[1].product_id);
        assert!(expect.contains(&actual));
    }
} // end of fn convert_to_olset_dup_error
