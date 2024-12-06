use std::boxed::Box;
use std::sync::Arc;

use chrono::DateTime;

use ecommerce_common::api::dto::CurrencyDto;
use ecommerce_common::constant::ProductType;
use ecommerce_common::error::AppErrorCode;

use order::api::rpc::dto::ProductPriceDeleteDto;
use order::model::{ProductPriceModel, ProductPriceModelSet};
use order::repository::{app_repo_product_price, AbsProductPriceRepo};

use super::dstore_ctx_setup;
use crate::model::ut_clone_productprice;

fn ut_pprice_data() -> [ProductPriceModel; 10] {
    [
        ProductPriceModel {
            is_create: true,
            product_type: ProductType::Item,
            product_id: 1001,
            price: 87,
            start_after: DateTime::parse_from_rfc3339("2023-09-09T09:12:53.001985+08:00").unwrap(),
            end_before: DateTime::parse_from_rfc3339("2023-10-06T09:00:32.001030+08:00").unwrap(),
        },
        ProductPriceModel {
            is_create: true,
            product_type: ProductType::Package,
            product_id: 1002,
            price: 94555,
            start_after: DateTime::parse_from_rfc3339("2023-09-09T09:13:54+07:00").unwrap(),
            end_before: DateTime::parse_from_rfc3339("2023-10-07T09:01:30+06:00").unwrap(),
        },
        ProductPriceModel {
            is_create: true,
            product_type: ProductType::Item,
            product_id: 1003,
            price: 28379,
            start_after: DateTime::parse_from_rfc3339("2023-07-31T10:16:54+05:00").unwrap(),
            end_before: DateTime::parse_from_rfc3339("2023-10-10T09:01:31+02:00").unwrap(),
        },
        ProductPriceModel {
            is_create: true,
            product_type: ProductType::Package,
            product_id: 1004,
            price: 3008,
            start_after: DateTime::parse_from_rfc3339("2022-07-30T11:16:55.468-01:00").unwrap(),
            end_before: DateTime::parse_from_rfc3339("2023-10-10T09:01:31.3310+03:00").unwrap(),
        },
        ProductPriceModel {
            is_create: true,
            product_type: ProductType::Item,
            product_id: 1005,
            price: 1389,
            start_after: DateTime::parse_from_rfc3339("2023-07-29T10:17:04.1918+05:00").unwrap(),
            end_before: DateTime::parse_from_rfc3339("2023-10-06T09:01:32.00012-06:30").unwrap(),
        },
        ProductPriceModel {
            is_create: true,
            product_type: ProductType::Package,
            product_id: 1006,
            price: 183,
            start_after: DateTime::parse_from_rfc3339("2023-06-29T11:18:54.995+04:00").unwrap(),
            end_before: DateTime::parse_from_rfc3339("2023-10-05T08:14:05.913+09:00").unwrap(),
        },
        ProductPriceModel {
            is_create: true,
            product_type: ProductType::Item,
            product_id: 1007,
            price: 666,
            start_after: DateTime::parse_from_rfc3339("2022-07-28T12:24:47+08:00").unwrap(),
            end_before: DateTime::parse_from_rfc3339("2023-12-26T16:58:00+09:00").unwrap(),
        },
        // -------- update --------
        ProductPriceModel {
            is_create: false,
            product_type: ProductType::Item,
            product_id: 1001,
            price: 94,
            start_after: DateTime::parse_from_rfc3339("2023-09-09T09:12:53.001905+08:30").unwrap(),
            end_before: DateTime::parse_from_rfc3339("2023-10-06T09:00:30.10301+08:30").unwrap(),
        },
        ProductPriceModel {
            is_create: false,
            product_type: ProductType::Package,
            product_id: 1002,
            price: 515,
            start_after: DateTime::parse_from_rfc3339("2023-09-10T11:14:54+07:00").unwrap(),
            end_before: DateTime::parse_from_rfc3339("2023-10-07T09:01:30.000067+06:00").unwrap(),
        },
        ProductPriceModel {
            is_create: false,
            product_type: ProductType::Item,
            product_id: 1003,
            price: 28023,
            start_after: DateTime::parse_from_rfc3339("2023-07-31T10:18:54+05:00").unwrap(),
            end_before: DateTime::parse_from_rfc3339("2023-10-10T06:11:50+02:00").unwrap(),
        },
    ]
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn save_fetch_ok() {
    use ecommerce_common::api::dto::CurrencyDto;

    let ds = dstore_ctx_setup();
    let repo = app_repo_product_price(ds).await.unwrap();
    let data = ut_pprice_data();
    let store_id = 123;
    let items = data[..4]
        .iter()
        .map(ut_clone_productprice)
        .collect::<Vec<_>>();
    let mset = ProductPriceModelSet {
        store_id,
        items,
        currency: CurrencyDto::TWD,
    }; // TODO
    let result = repo.save(mset).await;
    assert!(result.is_ok());
    let result = repo
        .fetch(
            store_id,
            vec![(ProductType::Package, 1002), (ProductType::Item, 1003)],
        )
        .await;
    assert!(result.is_ok());
    if let Ok(ms) = result {
        assert_eq!(ms.items.len(), 2);
        ms.items
            .into_iter()
            .map(|m| {
                let expect = match &m.product_id {
                    1002 => (
                        94555,
                        "2023-09-09T09:13:54+07:00",
                        "2023-10-07T09:01:30+06:00",
                    ),
                    1003 => (
                        28379,
                        "2023-07-31T10:16:54+05:00",
                        "2023-10-10T09:01:31+02:00",
                    ),
                    _others => (
                        0u32,
                        "1997-07-31T23:59:59+00:00",
                        "1997-07-31T23:59:59+00:00",
                    ),
                };
                let actual = (m.price, m.start_after, m.end_before);
                assert_eq!(expect.0, actual.0);
                assert_eq!(DateTime::parse_from_rfc3339(expect.1).unwrap(), actual.1);
                assert_eq!(DateTime::parse_from_rfc3339(expect.2).unwrap(), actual.2);
            })
            .count();
    }
    let items = data[4..]
        .iter()
        .map(ut_clone_productprice)
        .collect::<Vec<_>>();
    let mset = ProductPriceModelSet {
        store_id,
        items,
        currency: CurrencyDto::TWD,
    }; // TODO
    let result = repo.save(mset).await;
    assert!(result.is_ok());
    let result = repo
        .fetch(
            store_id,
            vec![(ProductType::Item, 1005), (ProductType::Package, 1002)],
        )
        .await;
    assert!(result.is_ok());
    if let Ok(ms) = result {
        assert_eq!(ms.items.len(), 2);
        ms.items
            .into_iter()
            .map(|m| {
                let expect = match &m.product_id {
                    1002 => (
                        515,
                        "2023-09-10T11:14:54+07:00",
                        "2023-10-07T09:01:30+06:00",
                    ),
                    1005 => (
                        1389,
                        "2023-07-29T10:17:04+05:00",
                        "2023-10-06T09:01:32-06:30",
                    ),
                    _others => (
                        0u32,
                        "1997-07-31T23:59:59+00:00",
                        "1997-07-31T23:59:59+00:00",
                    ),
                };
                let actual = (m.price, m.start_after, m.end_before);
                assert_eq!(expect.0, actual.0);
                assert_eq!(DateTime::parse_from_rfc3339(expect.1).unwrap(), actual.1);
                assert_eq!(DateTime::parse_from_rfc3339(expect.2).unwrap(), actual.2);
            })
            .count();
    }
} // end of fn save_fetch_ok

#[tokio::test]
async fn fetch_empty() {
    let ds = dstore_ctx_setup();
    let repo = app_repo_product_price(ds).await.unwrap();
    let store_id = 123;
    let result = repo.fetch(store_id, vec![]).await;
    assert!(result.is_err());
    if let Err(e) = result {
        assert_eq!(e.code, AppErrorCode::ProductNotExist);
    }
    let result = repo.fetch_many(vec![]).await;
    assert!(result.is_err());
    if let Err(e) = result {
        assert_eq!(e.code, AppErrorCode::ProductNotExist);
    }
    let result = repo
        .fetch(
            store_id,
            vec![(ProductType::Item, 2005), (ProductType::Package, 2002)],
        )
        .await;
    assert!(result.is_err());
    if let Err(e) = result {
        assert_eq!(e.code, AppErrorCode::ProductNotExist);
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn save_insert_dup() {
    use std::time::Duration;
    use tokio::time::sleep;

    let ds = dstore_ctx_setup();
    let repo = app_repo_product_price(ds).await.unwrap();
    let data = ut_pprice_data();
    let store_id = 124;
    let items = data[..2]
        .iter()
        .map(ut_clone_productprice)
        .collect::<Vec<_>>();
    let mset = ProductPriceModelSet {
        store_id,
        items,
        currency: CurrencyDto::TWD,
    };
    let result = repo.save(mset).await;
    if let Err(e) = result.as_ref() {
        println!("[unit-test] error : {:?}", e);
    }
    assert!(result.is_ok());
    let mut is_dup_err = false;
    for _ in 0..3 {
        let items = data[..2]
            .iter()
            .map(ut_clone_productprice)
            .collect::<Vec<_>>();
        let mset = ProductPriceModelSet {
            store_id,
            items,
            currency: CurrencyDto::TWD,
        };
        let result = repo.save(mset).await;
        assert!(result.is_err());
        if let Err(e) = result {
            println!("[unit-test] error : {:?}", e);
            let dup_err_code = "1062";
            assert_eq!(e.code, AppErrorCode::RemoteDbServerFailure);
            is_dup_err = e.detail.as_ref().unwrap().contains(dup_err_code);
            if is_dup_err {
                break;
            } else {
                sleep(Duration::from_secs(1)).await
            }
        }
    }
    assert!(is_dup_err);
}

async fn ut_delete_common_setup(
    store_id: u32,
    currency: CurrencyDto,
    repo: Arc<Box<dyn AbsProductPriceRepo>>,
) {
    let data = ut_pprice_data();
    let mset = {
        let items = data[..7]
            .iter()
            .map(ut_clone_productprice)
            .collect::<Vec<_>>();
        ProductPriceModelSet {
            store_id,
            items,
            currency,
        }
    };
    let result = repo.save(mset).await;
    assert!(result.is_ok());
    let result = repo
        .fetch(
            store_id,
            vec![
                (ProductType::Item, 1005),
                (ProductType::Package, 1004),
                (ProductType::Package, 1002),
                (ProductType::Item, 1007),
            ],
        )
        .await;
    assert!(result.is_ok());
    if let Ok(ms) = result {
        assert_eq!(ms.items.len(), 4);
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn delete_some_ok() {
    let ds = dstore_ctx_setup();
    let repo = app_repo_product_price(ds).await.unwrap();
    let repo = Arc::new(repo);
    ut_delete_common_setup(125, CurrencyDto::TWD, repo.clone()).await;
    ut_delete_common_setup(126, CurrencyDto::IDR, repo.clone()).await;
    let pids = ProductPriceDeleteDto {
        items: Some(vec![1007, 1005]),
        pkgs: Some(vec![1004, 1002]),
        item_type: ProductType::Item,
        pkg_type: ProductType::Package,
    };
    let result = repo.delete(125, pids).await;
    assert!(result.is_ok());
    let pids = vec![
        (ProductType::Item, 1005),
        (ProductType::Package, 1004),
        (ProductType::Package, 1002),
        (ProductType::Item, 1007),
        (ProductType::Item, 1003),
        (ProductType::Package, 1006),
        (ProductType::Item, 1001),
    ];
    let result = repo.fetch(125, pids.clone()).await;
    assert!(result.is_ok());
    if let Ok(ms) = result {
        assert_eq!(ms.items.len(), 3);
        ms.items
            .into_iter()
            .map(|m| {
                let exists = match &m.product_id {
                    1001 | 1003 | 1006 => true,
                    _others => false,
                };
                assert!(exists);
            })
            .count();
    }
    let result = repo.fetch(126, pids.clone()).await;
    assert!(result.is_ok());
    if let Ok(ms) = result {
        assert_eq!(ms.items.len(), 7);
    }
} // end of fn delete_some_ok

#[tokio::test]
async fn delete_some_empty() {
    let ds = dstore_ctx_setup();
    let repo = app_repo_product_price(ds).await.unwrap();
    let pids = ProductPriceDeleteDto {
        items: Some(vec![]),
        pkgs: None,
        item_type: ProductType::Item,
        pkg_type: ProductType::Package,
    };
    let result = repo.delete(126, pids).await;
    assert!(result.is_err());
    if let Err(e) = result {
        assert_eq!(e.code, AppErrorCode::EmptyInputData);
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn delete_all_ok() {
    let ds = dstore_ctx_setup();
    let repo = app_repo_product_price(ds).await.unwrap();
    let repo = Arc::new(repo);
    ut_delete_common_setup(127, CurrencyDto::USD, repo.clone()).await;
    ut_delete_common_setup(128, CurrencyDto::INR, repo.clone()).await;
    let result = repo.delete_all(128).await;
    assert!(result.is_ok());
    let pids = vec![
        (ProductType::Item, 1005),
        (ProductType::Package, 1004),
        (ProductType::Package, 1002),
        (ProductType::Item, 1007),
        (ProductType::Item, 1003),
        (ProductType::Package, 1006),
    ];
    let result = repo.fetch(128, pids.clone()).await;
    assert!(result.is_err());
    if let Err(e) = result {
        assert_eq!(e.code, AppErrorCode::ProductNotExist);
    }
    let result = repo.fetch(127, pids).await;
    assert!(result.is_ok());
    if let Ok(ms) = result {
        assert_eq!(ms.items.len(), 6);
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn fetch_many_ok() {
    let ds = dstore_ctx_setup();
    let repo = app_repo_product_price(ds).await.unwrap();
    let repo = Arc::new(repo);
    ut_delete_common_setup(129, CurrencyDto::THB, repo.clone()).await;
    ut_delete_common_setup(130, CurrencyDto::TWD, repo.clone()).await;
    let pids = vec![
        (129, ProductType::Item, 1005),
        (130, ProductType::Package, 1004),
        (129, ProductType::Package, 1002),
        (130, ProductType::Item, 1007),
        (129, ProductType::Item, 1003),
        (130, ProductType::Package, 1006),
    ];
    let result = repo.fetch_many(pids).await;
    assert!(result.is_ok());
    if let Ok(msets) = result {
        assert_eq!(msets.len(), 2);
        msets
            .into_iter()
            .map(|mset| {
                let exists = match &mset.store_id {
                    129 => mset
                        .items
                        .into_iter()
                        .map(|m| match &m.product_id {
                            1005 | 1003 | 1002 => true,
                            _others => false,
                        })
                        .all(|b| b),
                    130 => mset
                        .items
                        .into_iter()
                        .map(|m| match &m.product_id {
                            1004 | 1006 | 1007 => true,
                            _others => false,
                        })
                        .all(|b| b),
                    _others => false,
                };
                assert!(exists);
            })
            .count();
    }
} // end of fn fetch_many_ok
