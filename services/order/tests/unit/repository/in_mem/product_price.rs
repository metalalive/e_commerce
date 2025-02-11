use chrono::DateTime;
use std::boxed::Box;
use std::vec;

use ecommerce_common::api::dto::CurrencyDto;
use ecommerce_common::error::AppErrorCode;

use order::api::rpc::dto::ProductPriceDeleteDto;
use order::datastore::{AbstInMemoryDStore, AppInMemoryDStore};
use order::model::{ProductPriceModel, ProductPriceModelSet};
use order::repository::{AbsProductPriceRepo, ProductPriceInMemRepo};

use super::{in_mem_ds_ctx_setup, MockInMemDeadDataStore};
use crate::model::ut_clone_productprice;

#[rustfmt::skip]
fn pprice_init_data() -> [ProductPriceModel; 7] {
    let rawdata = [
        (1001, 87, "2023-09-09T09:12:53+08:00", "2023-10-06T09:00:30+08:00"),
        (1002, 94555, "2023-09-09T09:13:54+07:00", "2023-10-07T09:01:30+06:00"),
        (1003, 28379, "2023-07-31T10:16:54+05:00", "2023-10-10T09:01:31+02:00"),
        (1004, 3008, "2022-07-30T11:16:55-01:00", "2023-10-10T09:01:31+03:00"),
        (1005, 1389, "2023-07-29T10:17:54+05:00", "2023-10-06T09:01:32+07:00"),
        (1006, 183, "2023-06-29T11:18:54+04:00", "2023-10-05T08:14:05+09:00"),
        (1007, 666, "2022-07-28T12:24:47+08:00", "2023-12-26T16:58:00+09:00"),
    ];

    rawdata.map(
        |(product_id, price, start_after, end_before)| ProductPriceModel {
            is_create: true,
            product_id,
            price,
            start_after: DateTime::parse_from_rfc3339(start_after).unwrap(),
            end_before: DateTime::parse_from_rfc3339(end_before).unwrap(),
        },
    )
} // end of pprice_init_data

async fn in_mem_repo_ds_setup<T: AbstInMemoryDStore + 'static>(
    max_items: u32,
) -> Box<dyn AbsProductPriceRepo> {
    let ds_ctx = in_mem_ds_ctx_setup::<T>(max_items);
    let inmem = ds_ctx.in_mem.as_ref().unwrap().clone();
    let result = ProductPriceInMemRepo::new(inmem).await;
    assert_eq!(result.is_ok(), true);
    Box::new(result.unwrap())
}

#[tokio::test]
async fn in_mem_save_fetch_ok_1() {
    let (mocked_store_id, pprice_data) = (5678, pprice_init_data());
    let repo = in_mem_repo_ds_setup::<AppInMemoryDStore>(15).await;
    // ------ subcase, the first bulk update
    let ppset = {
        let items = pprice_data[..3].iter().map(ut_clone_productprice).collect();
        ProductPriceModelSet {
            store_id: mocked_store_id,
            currency: CurrencyDto::TWD,
            items,
        }
    };
    let result = repo.save(ppset).await;
    assert!(result.is_ok());
    let fetching_ids = vec![1002, 1006, 1001];
    let result = repo.fetch(mocked_store_id, fetching_ids.clone()).await;
    assert!(result.is_ok());
    if let Ok(fetched) = result {
        assert_eq!(fetched.store_id, mocked_store_id);
        assert_eq!(fetched.items.len(), 2);
        let exists = fetched
            .items
            .iter()
            .find(|m| m.product_id == fetching_ids[0]);
        assert_eq!(exists.unwrap(), &pprice_data[1]);
        let exists = fetched
            .items
            .iter()
            .find(|m| m.product_id == fetching_ids[2]);
        assert_eq!(exists.unwrap(), &pprice_data[0]);
        let exists = fetched
            .items
            .iter()
            .any(|m| m.product_id == fetching_ids[1]);
        assert_eq!(exists, false);
    }
    // ------ subcase, the second bulk update
    let ppset = {
        let items = pprice_data[3..].iter().map(ut_clone_productprice).collect();
        ProductPriceModelSet {
            store_id: mocked_store_id,
            currency: CurrencyDto::TWD,
            items,
        }
    };
    let result = repo.save(ppset).await;
    assert!(result.is_ok());
    let fetching_ids = vec![1007, 1006, 1099];
    let result = repo.fetch(mocked_store_id, fetching_ids.clone()).await;
    assert!(result.is_ok());
    if let Ok(fetched) = result {
        let exists = fetched.items.iter().find_map(|m| {
            if m.product_id == fetching_ids[0] {
                Some(m)
            } else {
                None
            }
        });
        assert_eq!(exists.unwrap(), &pprice_data[6]);
        let exists = fetched.items.iter().find_map(|m| {
            if m.product_id == fetching_ids[1] {
                Some(m)
            } else {
                None
            }
        });
        assert_eq!(exists.unwrap(), &pprice_data[5]);
        let exists = fetched
            .items
            .iter()
            .any(|m| m.product_id == fetching_ids[2]);
        assert_eq!(exists, false);
    }
} // end of fn in_mem_save_fetch_ok_1

#[tokio::test]
async fn in_mem_save_fetch_ok_2() {
    let (mocked_store_id, pprice_data) = (5678, pprice_init_data());
    let repo = in_mem_repo_ds_setup::<AppInMemoryDStore>(15).await;
    let ppset = {
        let items = pprice_data[4..6]
            .iter()
            .map(ut_clone_productprice)
            .collect();
        ProductPriceModelSet {
            store_id: mocked_store_id,
            currency: CurrencyDto::USD,
            items,
        }
    };
    let result = repo.save(ppset).await;
    assert!(result.is_ok());
    let fetching_ids = vec![1006, 1005];
    let result = repo.fetch(mocked_store_id, fetching_ids.clone()).await;
    assert!(result.is_ok());
    if let Ok(fetched) = result {
        let exists = fetched
            .items
            .iter()
            .find(|m| m.product_id == fetching_ids[0]);
        assert_eq!(exists.unwrap(), &pprice_data[5]);
        let exists = fetched
            .items
            .iter()
            .find(|m| m.product_id == fetching_ids[1]);
        assert_eq!(exists.unwrap(), &pprice_data[4]);
        assert!(matches!(fetched.currency, CurrencyDto::USD));
    }
    // --------
    let new_5th_elm = ProductPriceModel {
        is_create: false,
        price: 7811,
        product_id: pprice_data[5].product_id,
        start_after: DateTime::parse_from_rfc3339("2023-09-11T15:33:54-07:00").unwrap(),
        end_before: DateTime::parse_from_rfc3339("2023-10-12T09:02:34+06:00").unwrap(),
    };
    let ppset = {
        let items = vec![
            ut_clone_productprice(&pprice_data[6]),
            ut_clone_productprice(&new_5th_elm),
        ];
        ProductPriceModelSet {
            store_id: mocked_store_id,
            currency: CurrencyDto::USD,
            items,
        }
    };
    let result = repo.save(ppset).await;
    assert!(result.is_ok());
    let result = repo.fetch(mocked_store_id, fetching_ids.clone()).await;
    assert!(result.is_ok());
    if let Ok(fetched) = result {
        let exists = fetched.items.iter().find_map(|m| {
            if m.product_id == fetching_ids[0] {
                Some(m)
            } else {
                None
            }
        });
        let actual = exists.unwrap();
        assert_eq!(actual, &new_5th_elm);
        assert_ne!(actual, &pprice_data[5]);
        assert!(matches!(fetched.currency, CurrencyDto::USD));
    }
} // end of fn in_mem_save_fetch_ok_2

#[tokio::test]
async fn in_mem_save_fetch_ok_3() {
    let pprice_data = pprice_init_data();
    let mocked_store_ids = [5566u32, 7788u32];
    let repo = in_mem_repo_ds_setup::<AppInMemoryDStore>(15).await;
    {
        let ppset = {
            let items = pprice_data[0..2]
                .iter()
                .map(ut_clone_productprice)
                .collect();
            ProductPriceModelSet {
                currency: CurrencyDto::TWD,
                store_id: mocked_store_ids[0],
                items,
            }
        };
        let result = repo.save(ppset).await;
        assert!(result.is_ok());
        let ppset = {
            let items = pprice_data[2..5]
                .iter()
                .map(ut_clone_productprice)
                .collect();
            ProductPriceModelSet {
                currency: CurrencyDto::IDR,
                store_id: mocked_store_ids[1],
                items,
            }
        };
        let result = repo.save(ppset).await;
        assert!(result.is_ok());
    }
    let fetching_ids = {
        let mut out = vec![];
        let iter = pprice_data[0..2]
            .iter()
            .map(|d| (mocked_store_ids[0], d.product_id));
        out.extend(iter);
        let iter = pprice_data[2..]
            .iter()
            .map(|d| (mocked_store_ids[1], d.product_id));
        out.extend(iter);
        out
    };
    let result = repo.fetch_many(fetching_ids).await;
    assert!(result.is_ok());
    if let Ok(fetched) = result {
        assert_eq!(fetched.len(), 2);
        let result = fetched.iter().find(|d| d.store_id == mocked_store_ids[0]);
        assert!(result.is_some());
        if let Some(ppset) = result {
            assert!(matches!(ppset.currency, CurrencyDto::TWD));
            assert_eq!(ppset.items.len(), 2);
            let pp = ppset.items.iter().find(|d| d.product_id == 1002);
            assert!(pp.is_some());
        }
        let result = fetched.iter().find(|d| d.store_id == mocked_store_ids[1]);
        assert!(result.is_some());
        if let Some(ppset) = result {
            assert!(matches!(ppset.currency, CurrencyDto::IDR));
            assert_eq!(ppset.items.len(), 3);
        }
    }
} // end of fn in_mem_save_fetch_ok_3

#[tokio::test]
async fn in_mem_save_empty_input() {
    let repo = in_mem_repo_ds_setup::<AppInMemoryDStore>(4).await;
    let ppset = ProductPriceModelSet {
        store_id: 1234,
        currency: CurrencyDto::IDR,
        items: Vec::new(),
    };
    let result = repo.save(ppset).await;
    assert_eq!(result.is_err(), true);
    let error = result.err().unwrap();
    assert_eq!(error.code, AppErrorCode::EmptyInputData);
}

#[tokio::test]
async fn in_mem_save_dstore_error() {
    let (mocked_store_id, pprice_data) = (5678, pprice_init_data());
    let repo = in_mem_repo_ds_setup::<MockInMemDeadDataStore>(5).await;
    let ppset = {
        let item = ut_clone_productprice(&pprice_data[0]);
        ProductPriceModelSet {
            store_id: mocked_store_id,
            currency: CurrencyDto::THB,
            items: vec![item],
        }
    };
    let result = repo.save(ppset).await;
    assert_eq!(result.is_err(), true);
    let error = result.err().unwrap();
    assert_eq!(error.code, AppErrorCode::DataTableNotExist);
    assert_eq!(error.detail, Some("utest".to_string()));
}

#[tokio::test]
async fn in_mem_fetch_dstore_error() {
    let repo = in_mem_repo_ds_setup::<MockInMemDeadDataStore>(4).await;
    let ids = vec![1001];
    let result = repo.fetch(124u32, ids).await;
    assert_eq!(result.is_err(), true);
    let error = result.err().unwrap();
    assert_eq!(error.code, AppErrorCode::AcquireLockFailure);
    assert_eq!(error.detail, Some("utest".to_string()));
}

#[tokio::test]
async fn in_mem_delete_subset_ok() {
    let (mocked_store_id, pprice_data) = (512, pprice_init_data());
    let repo = in_mem_repo_ds_setup::<AppInMemoryDStore>(15).await;
    let ppset = {
        let items = pprice_data.iter().map(ut_clone_productprice).collect();
        ProductPriceModelSet {
            store_id: mocked_store_id,
            currency: CurrencyDto::INR,
            items,
        }
    };
    let result = repo.save(ppset).await;
    assert!(result.is_ok());
    let fetching_ids = vec![1005, 1002];
    let result = repo.fetch(mocked_store_id, fetching_ids.clone()).await;
    assert!(result.is_ok());
    if let Ok(fetched) = result {
        assert_eq!(fetched.items.len(), 2);
    }
    let deleting_req = ProductPriceDeleteDto {
        items: Some(fetching_ids.clone()),
    };
    let result = repo.delete(mocked_store_id, deleting_req).await;
    assert!(result.is_ok());
    let result = repo.fetch(mocked_store_id, fetching_ids).await;
    assert!(result.is_ok());
    if let Ok(fetched) = result {
        assert!(matches!(fetched.currency, CurrencyDto::INR));
        assert_eq!(fetched.items.len(), 0);
    }
    let fetching_ids = vec![1007, 1006, 1004, 1003, 1001];
    let result = repo.fetch(mocked_store_id, fetching_ids).await;
    assert!(result.is_ok());
    if let Ok(fetched) = result {
        assert!(matches!(fetched.currency, CurrencyDto::INR));
        assert_eq!(fetched.items.len(), 5);
    }
} // end of fn in_mem_delete_subset_ok

#[tokio::test]
async fn in_mem_delete_subset_id_empty() {
    let mocked_store_id = 512;
    let repo = in_mem_repo_ds_setup::<AppInMemoryDStore>(4).await;
    let deleting_req = ProductPriceDeleteDto {
        items: Some(Vec::new()),
    };
    let result = repo.delete(mocked_store_id, deleting_req).await;
    assert!(result.is_err());
    let actual_error = result.unwrap_err();
    assert_eq!(actual_error.code, AppErrorCode::EmptyInputData);
}

#[tokio::test]
async fn in_mem_delete_all_ok() {
    let (mocked_store_ids, pprice_data) = ([543u32, 995u32], pprice_init_data());
    let repo = in_mem_repo_ds_setup::<AppInMemoryDStore>(15).await;
    let ppset = ProductPriceModelSet {
        store_id: mocked_store_ids[0],
        currency: CurrencyDto::USD,
        items: pprice_data[..4].iter().map(ut_clone_productprice).collect(),
    };
    let result = repo.save(ppset).await;
    assert!(result.is_ok());
    let ppset = ProductPriceModelSet {
        store_id: mocked_store_ids[1],
        currency: CurrencyDto::TWD,
        items: pprice_data[4..].iter().map(ut_clone_productprice).collect(),
    };
    let result = repo.save(ppset).await;
    assert!(result.is_ok());
    let deleting_id = mocked_store_ids[0];
    let fetching_ids = vec![1001, 1003, 1002, 1004];
    let result = repo.fetch(deleting_id, fetching_ids.clone()).await;
    assert!(result.is_ok());
    if let Ok(fetched) = result {
        assert_eq!(fetched.items.len(), 4);
    }
    let result = repo.delete_all(deleting_id).await;
    assert!(result.is_ok());
    let result = repo.fetch(deleting_id, fetching_ids).await;
    assert!(result.is_err());
    if let Err(e) = result {
        assert_eq!(e.code, AppErrorCode::ProductNotExist);
        assert_eq!(e.detail.unwrap().as_str(), "missing-store");
    }
    let fetching_ids = vec![1005, 1007, 1006];
    let result = repo.fetch(mocked_store_ids[1], fetching_ids).await;
    assert!(result.is_ok());
    if let Ok(fetched) = result {
        assert!(matches!(fetched.currency, CurrencyDto::TWD));
        assert_eq!(fetched.items.len(), 3);
    }
} // end of fn in_mem_delete_all_ok
