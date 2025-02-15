use std::boxed::Box;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use chrono::{DateTime, TimeDelta};
use tokio::time::sleep;

use ecommerce_common::api::dto::CurrencyDto;
use ecommerce_common::error::AppErrorCode;

use order::api::rpc::dto::{
    ProdAttrPriceSetDto, ProdAttrValueDto, ProductAttrPriceDto, ProductPriceDeleteDto,
    ProductPriceEditDto,
};
use order::model::{ProductPriceModel, ProductPriceModelSet};
use order::repository::{app_repo_product_price, AbsProductPriceRepo};

use super::dstore_ctx_setup;

#[rustfmt::skip]
fn ut_pprice_data() -> Vec<ProductPriceModel> {
    let rawdata_new = [
        (1001, 87, "2023-09-09T09:12:53.001985+08:00", vec![]),
        (1002, 94555, "2023-09-09T09:13:54+07:00", vec![("wUd0o", ProdAttrValueDto::Bool(true), 2i32)]  ),
        (1003, 28379, "2023-07-31T10:16:54+05:00", vec![]),
        (1004, 3008, "2022-07-30T11:16:55.468-01:00", vec![]),
        (1005, 1389, "2023-07-29T10:17:04.1918+05:00", vec![("boRed", ProdAttrValueDto::Int(57), 9i32)]),
        (1006, 183, "2023-06-29T11:18:54.995+04:00", vec![]),
        (1007, 666, "2022-07-28T12:24:47+08:00", vec![]),
    ];
    let rawdata_saved = [
        (1001, 94, "2023-09-09T09:12:53.001905+08:30", "2023-10-06T09:00:30.10301+08:30"),
        (1002, 515, "2023-09-10T11:14:54+07:00", "2023-10-07T09:01:30.000067+06:00"),
        (1003, 28023, "2023-07-31T10:18:54+05:00", "2023-10-10T06:11:50+02:00"),
    ];
    let mut out = rawdata_new.into_iter().map(
        |(product_id, price, t0, attr_price_raw)| {
            let start_after = DateTime::parse_from_rfc3339(t0).unwrap();
            let end_before = start_after + TimeDelta::days(9);
            let last_update = start_after - TimeDelta::days(1);
            let extra_charge = attr_price_raw.into_iter().map(|(label, value, price)| 
                ProductAttrPriceDto { label_id: label.to_string(), value, price }
            ).collect::<Vec<_>>();
            let d = ProductPriceEditDto {
                product_id, price, start_after, end_before,
                attributes: ProdAttrPriceSetDto { extra_charge, last_update },
            };
            ProductPriceModel::try_from(&d).unwrap()
        })
        .collect::<Vec<_>>();
    let mock_saved_iter = rawdata_saved.into_iter().map(
        |(product_id, price, t0, t1)| {
            let start_after = DateTime::parse_from_rfc3339(t0).unwrap();
            let end_before = DateTime::parse_from_rfc3339(t1).unwrap();
            let last_update = start_after - TimeDelta::days(1);
            let ts = [start_after, end_before, last_update];
            let args = (product_id, price, ts, None);
            ProductPriceModel::from(args)
        });
    out.extend(mock_saved_iter);
    out
} // end of fn ut_pprice_data

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn save_fetch_ok() {
    use ecommerce_common::api::dto::CurrencyDto;

    let ds = dstore_ctx_setup();
    let repo = app_repo_product_price(ds).await.unwrap();
    let mut data = ut_pprice_data();
    let pprice_ms_subsequent = data.split_off(4);
    let store_id = 123;
    let mset = ProductPriceModelSet {
        store_id,
        items: data,
        currency: CurrencyDto::TWD,
    }; // TODO
    let result = repo.save(mset).await;
    assert!(result.is_ok());
    let result = repo.fetch(store_id, vec![1002, 1003]).await;
    assert!(result.is_ok());
    if let Ok(ms) = result {
        assert_eq!(ms.items.len(), 2);
        ms.items
            .into_iter()
            .map(|actual| {
                let expect = match &actual.product_id() {
                    1002 => (
                        94555,
                        "2023-09-09T09:13:54+07:00",
                        "2023-09-18T09:13:54+07:00",
                        "2023-09-08T09:13:54+07:00",
                        Some(HashMap::from([("wUd0o-true".to_string(), 2i32)])),
                    ),
                    1003 => (
                        28379,
                        "2023-07-31T10:16:54+05:00",
                        "2023-08-09T10:16:54+05:00",
                        "2023-07-30T10:16:54+05:00",
                        None,
                    ),
                    _others => (
                        0u32,
                        "1997-07-31T23:59:59+00:00",
                        "1997-07-31T23:59:59+00:00",
                        "1997-07-31T23:59:59+00:00",
                        None,
                    ),
                };
                let t0 = DateTime::parse_from_rfc3339(expect.1).unwrap();
                let t1 = DateTime::parse_from_rfc3339(expect.2).unwrap();
                let t2 = DateTime::parse_from_rfc3339(expect.3).unwrap();
                let args = (actual.product_id(), expect.0, [t0, t1, t2], expect.4);
                let expect_obj = ProductPriceModel::from(args);
                assert_eq!(expect_obj, actual);
            })
            .count();
    }
    let mset = ProductPriceModelSet {
        store_id,
        items: pprice_ms_subsequent,
        currency: CurrencyDto::TWD,
    }; // TODO
    let result = repo.save(mset).await;
    assert!(result.is_ok());
    let result = repo.fetch(store_id, vec![1005, 1002]).await;
    assert!(result.is_ok());
    if let Ok(ms) = result {
        assert_eq!(ms.items.len(), 2);
        ms.items
            .into_iter()
            .map(|actual| {
                let expect = match &actual.product_id() {
                    1002 => (
                        515,
                        "2023-09-10T11:14:54+07:00",
                        "2023-10-07T09:01:30+06:00",
                        "2023-09-09T11:14:54+07:00",
                        None,
                    ),
                    1005 => (
                        1389,
                        "2023-07-29T10:17:04+05:00",
                        "2023-08-07T10:17:04+05:00",
                        "2023-07-28T10:17:04+05:00",
                        Some(HashMap::from([("boRed-57".to_string(), 9i32)])),
                    ),
                    _others => (
                        0u32,
                        "1997-07-31T23:59:59+00:00",
                        "1997-07-31T23:59:59+00:00",
                        "1997-07-31T23:59:59+00:00",
                        None,
                    ),
                };
                let t0 = DateTime::parse_from_rfc3339(expect.1).unwrap();
                let t1 = DateTime::parse_from_rfc3339(expect.2).unwrap();
                let t2 = DateTime::parse_from_rfc3339(expect.3).unwrap();
                let args = (actual.product_id(), expect.0, [t0, t1, t2], expect.4);
                let expect_obj = ProductPriceModel::from(args);
                assert_eq!(expect_obj, actual);
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
    let result = repo.fetch(store_id, vec![2005, 2002]).await;
    assert!(result.is_err());
    if let Err(e) = result {
        assert_eq!(e.code, AppErrorCode::ProductNotExist);
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn save_insert_dup() {
    let ds = dstore_ctx_setup();
    let repo = app_repo_product_price(ds).await.unwrap();
    let mut data = ut_pprice_data();
    let _ = data.split_off(2);
    let store_id = 124;
    let mset = ProductPriceModelSet {
        store_id,
        items: data[..2].iter().map(Clone::clone).collect(),
        currency: CurrencyDto::TWD,
    };
    let result = repo.save(mset).await;
    if let Err(e) = result.as_ref() {
        println!("[unit-test] error : {:?}", e);
    }
    assert!(result.is_ok());
    let mut is_dup_err = false;
    for _ in 0..3 {
        let mset = ProductPriceModelSet {
            store_id,
            items: data[..2].iter().map(Clone::clone).collect(),
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
    let mut data = ut_pprice_data();
    let _ = data.split_off(7); // discard subsequent
    let mset = ProductPriceModelSet {
        store_id,
        items: data,
        currency,
    };
    let result = repo.save(mset).await;
    assert!(result.is_ok());
    let result = repo.fetch(store_id, vec![1005, 1004, 1002, 1007]).await;
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
        items: Some(vec![1004, 1002, 1007, 1005]),
    };
    let result = repo.delete(125, pids).await;
    assert!(result.is_ok());
    let pids = vec![1005, 1004, 1002, 1007, 1003, 1006, 1001];
    let result = repo.fetch(125, pids.clone()).await;
    assert!(result.is_ok());
    if let Ok(ms) = result {
        assert_eq!(ms.items.len(), 3);
        ms.items
            .into_iter()
            .map(|m| {
                let exists = match &m.product_id() {
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
    let pids = vec![1005, 1004, 1002, 1007, 1003, 1006];
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
        (129, 1005),
        (130, 1004),
        (129, 1002),
        (130, 1007),
        (129, 1003),
        (130, 1006),
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
                        .map(|m| match m.product_id() {
                            1005 | 1003 | 1002 => true,
                            _others => false,
                        })
                        .all(|b| b),
                    130 => mset
                        .items
                        .into_iter()
                        .map(|m| match &m.product_id() {
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
