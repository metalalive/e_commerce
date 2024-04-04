use chrono::DateTime;
use std::vec;
use std::vec::Vec;

use order::api::rpc::dto::ProductPriceEditDto;
use order::constant::ProductType;
use order::error::AppErrorCode;
use order::model::{ProductPriceModel, ProductPriceModelSet};

fn setup_mocked_saved_items() -> Vec<ProductPriceModel> {
    vec![
        ProductPriceModel {
            is_create: false,
            product_type: ProductType::Item,
            product_id: 2003,
            price: 28379,
            start_after: DateTime::parse_from_rfc3339("2023-07-31T10:16:54+05:00")
                .unwrap()
                .into(),
            end_before: DateTime::parse_from_rfc3339("2023-10-10T09:01:31+02:00")
                .unwrap()
                .into(),
        },
        ProductPriceModel {
            is_create: false,
            product_type: ProductType::Package,
            product_id: 2004,
            price: 3008,
            start_after: DateTime::parse_from_rfc3339("2022-07-30T11:16:55-01:00")
                .unwrap()
                .into(),
            end_before: DateTime::parse_from_rfc3339("2023-10-10T09:01:31+03:00")
                .unwrap()
                .into(),
        },
        ProductPriceModel {
            is_create: false,
            product_type: ProductType::Item,
            product_id: 2005,
            price: 1389,
            start_after: DateTime::parse_from_rfc3339("2023-07-29T10:17:54+05:00")
                .unwrap()
                .into(),
            end_before: DateTime::parse_from_rfc3339("2023-10-06T09:01:32+07:00")
                .unwrap()
                .into(),
        },
        ProductPriceModel {
            is_create: false,
            product_type: ProductType::Package,
            product_id: 2006,
            price: 183,
            start_after: DateTime::parse_from_rfc3339("2023-06-29T11:18:54+04:00")
                .unwrap()
                .into(),
            end_before: DateTime::parse_from_rfc3339("2023-10-05T08:14:05+09:00")
                .unwrap()
                .into(),
        },
    ]
}

fn setup_expect_updated_items() -> Vec<ProductPriceModel> {
    let mut out = setup_mocked_saved_items();
    out[1] = ProductPriceModel {
        is_create: false,
        price: 389,
        product_type: ProductType::Item,
        product_id: 2005,
        start_after: DateTime::parse_from_rfc3339("2022-11-25T09:13:39+06:00")
            .unwrap()
            .into(),
        end_before: DateTime::parse_from_rfc3339("2023-09-12T21:23:00+05:00")
            .unwrap()
            .into(),
    };
    out[2] = ProductPriceModel {
        is_create: false,
        price: 51,
        product_type: ProductType::Package,
        product_id: 2004,
        start_after: DateTime::parse_from_rfc3339("2022-11-24T09:25:39+05:00")
            .unwrap()
            .into(),
        end_before: DateTime::parse_from_rfc3339("2023-09-12T21:13:01+11:00")
            .unwrap()
            .into(),
    };
    out.insert(
        0,
        ProductPriceModel {
            is_create: true,
            price: 2388,
            product_type: ProductType::Package,
            product_id: 2018,
            start_after: DateTime::parse_from_rfc3339("2022-11-21T23:09:05+09:00")
                .unwrap()
                .into(),
            end_before: DateTime::parse_from_rfc3339("2023-10-13T02:54:00-09:00")
                .unwrap()
                .into(),
        },
    );
    out.insert(
        0,
        ProductPriceModel {
            is_create: true,
            price: 20550,
            product_type: ProductType::Item,
            product_id: 2019,
            start_after: DateTime::parse_from_rfc3339("2022-11-29T09:13:39+06:00")
                .unwrap()
                .into(),
            end_before: DateTime::parse_from_rfc3339("2023-08-30T21:19:00+10:00")
                .unwrap()
                .into(),
        },
    );
    out
}

#[test]
fn update_instance_ok() {
    let (store_id, saved) = (1234, setup_mocked_saved_items());
    let ms = ProductPriceModelSet {
        store_id,
        items: saved,
    };
    let data_update = vec![
        ProductPriceEditDto {
            price: 389,
            product_type: ProductType::Item,
            product_id: 2005,
            start_after: DateTime::parse_from_rfc3339("2022-11-25T09:13:39+06:00")
                .unwrap()
                .into(),
            end_before: DateTime::parse_from_rfc3339("2023-09-12T21:23:00+05:00")
                .unwrap()
                .into(),
        },
        ProductPriceEditDto {
            price: 51,
            product_type: ProductType::Package,
            product_id: 2004,
            start_after: DateTime::parse_from_rfc3339("2022-11-24T09:25:39+05:00")
                .unwrap()
                .into(),
            end_before: DateTime::parse_from_rfc3339("2023-09-12T21:13:01+11:00")
                .unwrap()
                .into(),
        },
    ];
    let data_create = vec![
        ProductPriceEditDto {
            price: 2388,
            product_type: ProductType::Package,
            product_id: 2018,
            start_after: DateTime::parse_from_rfc3339("2022-11-21T23:09:05+09:00")
                .unwrap()
                .into(),
            end_before: DateTime::parse_from_rfc3339("2023-10-13T02:54:00-09:00")
                .unwrap()
                .into(),
        },
        ProductPriceEditDto {
            price: 20550,
            product_type: ProductType::Item,
            product_id: 2019,
            start_after: DateTime::parse_from_rfc3339("2022-11-29T09:13:39+06:00")
                .unwrap()
                .into(),
            end_before: DateTime::parse_from_rfc3339("2023-08-30T21:19:00+10:00")
                .unwrap()
                .into(),
        },
    ];
    let result = ms.update(data_update, data_create);
    assert_eq!(result.is_ok(), true);
    let actual_ms = result.unwrap();
    assert_eq!(actual_ms.items.len(), 6);
    let expect_ms = setup_expect_updated_items();
    // it might not be worthy implementing all traits required by HashSet just for test,
    // in this test, check equality by the nested loop
    let num_matched = actual_ms
        .items
        .iter()
        .filter_map(|ma| {
            let matched = expect_ms.iter().any(|me| me == ma);
            if matched {
                Some(())
            } else {
                None
            }
        })
        .count();
    assert_eq!(num_matched, 6usize);
} // end of fn update_instance_ok

#[test]
fn update_instance_error() {
    let (store_id, saved) = (1234, setup_mocked_saved_items());
    let ms = ProductPriceModelSet {
        store_id,
        items: saved,
    };
    let data_update = vec![
        ProductPriceEditDto {
            price: 51,
            product_type: ProductType::Package,
            product_id: 2004,
            start_after: DateTime::parse_from_rfc3339("2022-11-24T09:25:39+05:00")
                .unwrap()
                .into(),
            end_before: DateTime::parse_from_rfc3339("2023-09-12T21:13:01+11:00")
                .unwrap()
                .into(),
        },
        ProductPriceEditDto {
            price: 2388,
            product_type: ProductType::Package,
            product_id: 2018,
            start_after: DateTime::parse_from_rfc3339("2022-11-21T23:09:05+09:00")
                .unwrap()
                .into(),
            end_before: DateTime::parse_from_rfc3339("2023-10-13T02:54:00-09:00")
                .unwrap()
                .into(),
        }, // this will cause error
    ];
    let data_create = vec![];
    let result = ms.update(data_update, data_create);
    assert_eq!(result.is_err(), true);
    if let Err(e) = result {
        assert_eq!(e.code, AppErrorCode::InvalidInput);
    }
} // end of fn update_instance_error
