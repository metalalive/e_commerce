use chrono::DateTime;
use std::collections::HashMap;
use std::vec;
use std::vec::Vec;

use ecommerce_common::api::dto::CurrencyDto;
use ecommerce_common::error::AppErrorCode;

use order::api::dto::ProdAttrValueDto;
use order::api::rpc::dto::{ProdAttrPriceSetDto, ProductAttrPriceDto, ProductPriceEditDto};
use order::model::{ProductPriceModel, ProductPriceModelSet};

#[rustfmt::skip]
fn setup_mocked_saved_items() -> Vec<ProductPriceModel> {
    [
        (2003u64, 28379u32, "2023-07-31T10:16:54+05:00", "2023-10-10T09:01:31+02:00", None),
        (2004, 3008, "2022-07-30T11:16:55-01:00", "2023-10-10T09:01:31+03:00", None),
        (2005, 1389, "2023-07-29T10:17:54+05:00", "2023-10-06T09:01:32+07:00",None),
        (2006, 183, "2023-06-29T11:18:54+04:00", "2023-10-05T08:14:05+09:00",None),
        (2007, 524, "2023-07-03T11:19:53+07:00", "2023-11-28T19:05:14+09:00",
         Some(vec![("gh0st-true".to_string(), 2)]) ),
    ]
    .into_iter()
    .map(|d| {
        let product_id = d.0;
        let price = d.1;
        let t0 = DateTime::parse_from_rfc3339(d.2).unwrap();
        let t1 = DateTime::parse_from_rfc3339(d.3).unwrap();
        let attr_last_update = DateTime::parse_from_rfc3339("2022-10-09T01:03:55+08:00").unwrap();
        let attr_pricing_map = d.4.map(
            |v: Vec<(String, i32)>| HashMap::from_iter(v.into_iter())
        );
        let ts = [t0, t1, attr_last_update];
        let args = (product_id, price, ts, attr_pricing_map);
        ProductPriceModel::from(args)
    })
    .collect::<Vec<_>>()
}

#[rustfmt::skip]
fn setup_expect_updated_items() -> Vec<ProductPriceModel> {
    let mock_attr_lastupdate = DateTime::parse_from_rfc3339("2022-10-09T01:03:55+08:00").unwrap();
    let mut out = setup_mocked_saved_items();

    out[1] = {
        let attrmap_iter = [
            ("aG1ng-567", 25i32),
            ("maru-NodeMCU", 0),
            ("baker-true", -26),
        ].into_iter().map(| d| (d.0.to_string(), d.1));
        let attrmap = HashMap::from_iter(attrmap_iter );
        let t0 = DateTime::parse_from_rfc3339("2022-11-25T09:13:39+06:00").unwrap();
        let t1 = DateTime::parse_from_rfc3339("2023-09-12T21:23:00+05:00").unwrap();
        let args = (2005, 389, [t0, t1, mock_attr_lastupdate], Some(attrmap));
        ProductPriceModel::from(args)
    };
    out[2] = {
        let t0 = DateTime::parse_from_rfc3339("2022-11-24T09:25:39+05:00").unwrap();
        let t1 = DateTime::parse_from_rfc3339("2023-09-12T21:13:01+11:00").unwrap();
        let args = (2004, 51, [t0, t1, mock_attr_lastupdate], None);
        ProductPriceModel::from(args)
    };
    [
        (
            2388u32, 2018u64, "2022-11-21T23:09:05+09:00", "2023-10-13T02:54:00-09:00",
            vec![("ikKe", ProdAttrValueDto::Str("aRon".to_string()), 3)]
        ),
        (20550, 2019, "2022-11-29T09:13:39+06:00", "2023-08-30T21:19:00+10:00", vec![]),
    ]
    .into_iter()
    .map(|raw| {
        let extra_charge = raw.4.into_iter().map(|v| ProductAttrPriceDto {
            label_id: v.0.to_string(), value: v.1, price: v.2,
        }).collect::<Vec<_>>();
        let d = ProductPriceEditDto {
            price: raw.0,
            product_id: raw.1,
            start_after: DateTime::parse_from_rfc3339(raw.2).unwrap(),
            end_before: DateTime::parse_from_rfc3339(raw.3).unwrap(),
            attributes: ProdAttrPriceSetDto {
                extra_charge, last_update: mock_attr_lastupdate,
            },
        };
        let m = ProductPriceModel::try_from(&d).unwrap();
        out.insert(0, m);
    })
    .count();
    out
} // end of fn setup_expect_updated_items

#[test]
fn update_instance_ok() {
    let (store_id, currency, saved) = (1234, CurrencyDto::USD, setup_mocked_saved_items());
    let ms = ProductPriceModelSet {
        store_id,
        currency,
        items: saved,
    };
    #[rustfmt::skip]
    let mut product_data = [
        (389, 2005, "2022-11-25T09:13:39+06:00", "2023-09-12T21:23:00+05:00"),
        (51, 2004, "2022-11-24T09:25:39+05:00", "2023-09-12T21:13:01+11:00"),
        (2388, 2018, "2022-11-21T23:09:05+09:00", "2023-10-13T02:54:00-09:00"),
        (20550, 2019, "2022-11-29T09:13:39+06:00", "2023-08-30T21:19:00+10:00"),
    ]
    .iter()
    .map(|&(price, product_id, start_after, end_before)| ProductPriceEditDto {
        price,
        product_id,
        start_after: DateTime::parse_from_rfc3339(start_after).unwrap(),
        end_before: DateTime::parse_from_rfc3339(end_before).unwrap(),
        attributes: ProdAttrPriceSetDto {
            extra_charge: Vec::new(),
            last_update: DateTime::parse_from_rfc3339("2022-10-09T01:03:55+08:00").unwrap(),
        },
    })
    .collect::<Vec<_>>();
    let data_create = {
        let mut pd = product_data.split_off(2);
        let d = ProductAttrPriceDto {
            label_id: "ikKe".to_string(),
            value: ProdAttrValueDto::Str("aRon".to_string()),
            price: 3,
        };
        pd[0].attributes.extra_charge.push(d);
        pd
    };
    let data_update = {
        [
            ("aG1ng", ProdAttrValueDto::Int(567), 25),
            ("maru", ProdAttrValueDto::Str("NodeMCU".to_string()), 0),
            ("baker", ProdAttrValueDto::Bool(true), -26),
        ]
        .into_iter()
        .map(|d| {
            let d = ProductAttrPriceDto {
                label_id: d.0.to_string(),
                value: d.1,
                price: d.2,
            };
            product_data[0].attributes.extra_charge.push(d);
        })
        .count();
        product_data
    };

    let result = ms.update(data_update, data_create, CurrencyDto::TWD);
    assert_eq!(result.is_ok(), true);
    let actual_ms = result.unwrap();
    assert_eq!(actual_ms.items.len(), 7);
    assert!(matches!(actual_ms.currency, CurrencyDto::TWD));
    let expect_ms = setup_expect_updated_items();
    // it might not be worthy implementing all traits required by HashSet just for test,
    // in this test, check equality by the nested loop
    let num_matched = actual_ms
        .items
        .iter()
        .filter(|ma| expect_ms.iter().any(|me| &me == ma))
        .count();
    assert_eq!(num_matched, 7usize);
} // end of fn update_instance_ok

#[test]
fn update_error_nonexist_product() {
    let (store_id, currency, saved) = (1234, CurrencyDto::IDR, setup_mocked_saved_items());
    let ms = ProductPriceModelSet {
        store_id,
        currency,
        items: saved,
    };
    let data_update = vec![
        ProductPriceEditDto {
            price: 51,
            product_id: 2004,
            start_after: DateTime::parse_from_rfc3339("2022-11-24T09:25:39+05:00").unwrap(),
            end_before: DateTime::parse_from_rfc3339("2023-09-12T21:13:01+11:00").unwrap(),
            attributes: ProdAttrPriceSetDto {
                extra_charge: Vec::new(),
                last_update: DateTime::parse_from_rfc3339("2022-10-09T01:03:55+08:00").unwrap(),
            },
        },
        ProductPriceEditDto {
            price: 2388,
            product_id: 2018, // this will cause error due to non-exist product
            start_after: DateTime::parse_from_rfc3339("2022-11-21T23:09:05+09:00").unwrap(),
            end_before: DateTime::parse_from_rfc3339("2023-10-13T02:54:00-09:00").unwrap(),
            attributes: ProdAttrPriceSetDto {
                extra_charge: Vec::new(),
                last_update: DateTime::parse_from_rfc3339("2022-10-09T01:03:55+08:00").unwrap(),
            },
        },
    ];
    let data_create = vec![];
    let result = ms.update(data_update, data_create, CurrencyDto::IDR);
    assert_eq!(result.is_err(), true);
    if let Err(e) = result {
        assert_eq!(e.code, AppErrorCode::InvalidInput);
    }
} // end of fn update_error_nonexist_product

#[test]
fn update_error_dup_attri_value() {
    let ms = ProductPriceModelSet {
        store_id: 1234,
        currency: CurrencyDto::IDR,
        items: setup_mocked_saved_items(),
    };
    let data_update = vec![];
    let data_create = {
        let create_extra_charge = [
            ("aG1ng", ProdAttrValueDto::Int(567), 25),
            ("baker", ProdAttrValueDto::Bool(true), -26),
            ("aG1ng", ProdAttrValueDto::Int(567), 23),
        ]
        .into_iter()
        .map(|d| ProductAttrPriceDto {
            label_id: d.0.to_string(),
            value: d.1,
            price: d.2,
        })
        .collect::<Vec<_>>();
        let d = ProductPriceEditDto {
            price: 389,
            product_id: 2005,
            start_after: DateTime::parse_from_rfc3339("2022-11-25T09:13:39+06:00").unwrap(),
            end_before: DateTime::parse_from_rfc3339("2023-09-12T21:23:00+05:00").unwrap(),
            attributes: ProdAttrPriceSetDto {
                extra_charge: create_extra_charge,
                last_update: DateTime::parse_from_rfc3339("2022-10-09T01:03:55+08:00").unwrap(),
            },
        };
        vec![d]
    };
    let result = ms.update(data_update, data_create, CurrencyDto::IDR);
    assert!(result.is_err());
    if let Err(e) = result {
        assert_eq!(e.code, AppErrorCode::InvalidInput);
        let msg = e.detail.unwrap();
        assert!(msg.contains("prod-price-dup-attrval"));
    }
} // end of fn update_error_dup_attri_value
