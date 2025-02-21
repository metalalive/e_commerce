use std::collections::HashMap;
use std::ptr;
use std::sync::Arc;

use chrono::{DateTime, Duration, FixedOffset};
use rust_decimal::Decimal;

use ecommerce_common::api::dto::CurrencyDto;
use ecommerce_common::error::AppErrorCode;

use order::api::rpc::dto::{
    InventoryEditStockLevelDto, StockLevelReturnDto, StockReturnErrorDto, StockReturnErrorReason,
};
use order::api::web::dto::{OrderLineCreateErrorDto, OrderLineCreateErrorReason};
use order::datastore::AppInMemoryDStore;
use order::error::AppError;
use order::model::{
    CurrencyModel, OrderCurrencyModel, OrderLineAppliedPolicyModel, OrderLineIdentity,
    OrderLineModel, OrderLineModelSet, OrderLinePriceModel, OrderLineQuantityModel,
    ProdAttriPriceModel, ProductStockIdentity, ProductStockModel, StockLevelModelSet,
    StockQtyRsvModel, StockQuantityModel, StoreStockModel,
};
use order::repository::{
    AbsOrderRepo, AbsOrderStockRepo, AppStockRepoReserveReturn, AppStockRepoReserveUserFunc,
};

use super::super::MockInMemDeadDataStore;
use super::in_mem_repo_ds_setup;
use crate::model::verify_stocklvl_model;

fn ut_init_data_product() -> [ProductStockModel; 10] {
    let rawdata = [
        // ------ for insertion --------
        (9002, true, "2023-10-05T08:14:05+09:00", 5, 0, 0),
        (9003, true, "2023-11-07T08:12:05.008+02:00", 11, 0, 0),
        (9004, true, "2023-11-09T09:16:01.029-01:00", 15, 0, 0),
        (9005, true, "2024-11-11T09:22:01.005+08:00", 8, 0, 0),
        (9006, true, "2024-11-15T09:23:58.098+01:00", 14, 0, 0),
        // ------ for update --------
        (9004, false, "2023-11-09T09:16:01.029-01:00", 15, 7, 0),
        (9006, false, "2024-11-15T09:23:58.098+01:00", 18, 1, 0),
        (9004, false, "2023-11-09T09:16:01.035-01:00", 22, 8, 0),
        (9004, true, "2023-11-09T12:30:10.035-01:00", 20, 1, 0),
        (9004, true, "2020-03-15T12:55:08.035-11:00", 18, 3, 0),
    ];

    rawdata.map(
        |(id_, is_create, expiry, total, booked, cancelled)| ProductStockModel {
            id_,
            is_create,
            expiry: DateTime::parse_from_rfc3339(expiry).unwrap().into(),
            quantity: StockQuantityModel::new(total, booked, cancelled, None),
        },
    )
} // end of ut_init_data_product

const UT_INIT_DATA_STORE: [StoreStockModel; 4] = [
    StoreStockModel {
        store_id: 1001,
        products: vec![],
    },
    StoreStockModel {
        store_id: 1002,
        products: vec![],
    },
    StoreStockModel {
        store_id: 1003,
        products: vec![],
    },
    StoreStockModel {
        store_id: 1004,
        products: vec![],
    },
]; // end of ut_init_data_store

#[tokio::test]
async fn save_fetch_ok() {
    // this test case verifies product stock level, each has different product ID
    let repo = in_mem_repo_ds_setup::<AppInMemoryDStore>(12, None).await;
    let stockrepo = repo.stock();
    let all_products = ut_init_data_product();
    let expect_slset = {
        let mut stores = UT_INIT_DATA_STORE[..2].to_vec();
        assert_eq!(stores.len(), 2);
        stores[0].products.extend_from_slice(&all_products[0..2]);
        stores[1].products.extend_from_slice(&all_products[2..5]);
        assert_eq!(stores[0].products.len(), 2);
        assert_eq!(stores[1].products.len(), 3);
        StockLevelModelSet { stores }
    };
    let result = stockrepo.save(expect_slset.clone()).await;
    assert!(result.is_ok());
    let pids = expect_slset
        .stores
        .iter()
        .flat_map(|m1| {
            m1.products.iter().map(|m2| ProductStockIdentity {
                store_id: m1.store_id,
                product_id: m2.id_,
                expiry: m2.expiry_without_millis(),
            })
        })
        .collect();
    let result = stockrepo.fetch(pids).await;
    assert!(result.is_ok());
    if let Ok(actual) = result {
        assert!(!ptr::eq(&actual, &expect_slset));
        assert_eq!(actual.stores.len(), expect_slset.stores.len());
        verify_stocklvl_model(&actual, &expect_slset, [1, 1], true);
        verify_stocklvl_model(&actual, &expect_slset, [0, 1], true);
        verify_stocklvl_model(&actual, &expect_slset, [1, 2], true);
        verify_stocklvl_model(&actual, &expect_slset, [0, 0], true);
        let result = verify_stocklvl_model(&actual, &expect_slset, [1, 0], true);
        let product = result.unwrap();
        assert!(product.quantity.rsv_detail.is_none());
    }
} // end of  save_fetch_ok

#[tokio::test]
async fn update_existing_ok() {
    let repo = in_mem_repo_ds_setup::<AppInMemoryDStore>(8, None).await;
    let stockrepo = repo.stock();
    let all_products = ut_init_data_product();
    let mut expect_slset = {
        let mut stores = UT_INIT_DATA_STORE[0..1].to_vec();
        stores[0].products.extend_from_slice(&all_products[..5]);
        assert_eq!(stores[0].products.len(), 5);
        StockLevelModelSet { stores }
    };
    let result = stockrepo.save(expect_slset.clone()).await;
    assert!(result.is_ok());
    let pids = {
        let chosen_store = &expect_slset.stores[0];
        vec![
            ProductStockIdentity {
                store_id: chosen_store.store_id,
                product_id: chosen_store.products[2].id_,
                expiry: chosen_store.products[2].expiry,
            },
            ProductStockIdentity {
                store_id: chosen_store.store_id,
                product_id: chosen_store.products[4].id_,
                expiry: chosen_store.products[4].expiry,
            },
        ]
    };
    let result = stockrepo.fetch(pids.clone()).await;
    assert!(result.is_ok());
    if let Ok(actual) = result {
        assert!(!ptr::eq(&actual, &expect_slset));
        assert_eq!(actual.stores.len(), 1);
        assert_eq!(actual.stores[0].products.len(), 2);
        verify_stocklvl_model(&actual, &expect_slset, [0, 4], true);
        verify_stocklvl_model(&actual, &expect_slset, [0, 2], true);
    }
    // ------  subcase 2, start updating
    let expect_slset_ks2 = {
        let mut stores = UT_INIT_DATA_STORE[0..1].to_vec();
        stores[0].products.extend_from_slice(&all_products[5..7]);
        assert_eq!(stores[0].products.len(), 2);
        StockLevelModelSet { stores }
    };
    let result = stockrepo.save(expect_slset_ks2.clone()).await;
    assert!(result.is_ok());
    let result = stockrepo.fetch(pids).await;
    assert!(result.is_ok());
    if let Ok(actual) = result {
        assert!(!ptr::eq(&actual, &expect_slset_ks2));
        assert_eq!(actual.stores[0].products.len(), 2);
        verify_stocklvl_model(&actual, &expect_slset_ks2, [0, 0], true);
        verify_stocklvl_model(&actual, &expect_slset_ks2, [0, 1], true);
        // discard other items except idx 2 and 4
        expect_slset.stores[0].products.remove(0);
        expect_slset.stores[0].products.remove(0);
        expect_slset.stores[0].products.remove(1);
        verify_stocklvl_model(&actual, &expect_slset, [0, 0], false);
        verify_stocklvl_model(&actual, &expect_slset, [0, 1], false);
    }
} // end of fn update_existing_ok

#[tokio::test]
async fn same_product_diff_expiry() {
    let repo = in_mem_repo_ds_setup::<AppInMemoryDStore>(8, None).await;
    let stockrepo = repo.stock();
    let all_products = {
        let out = ut_init_data_product();
        assert_eq!(out[2].id_, out[7].id_);
        assert_eq!(out[2].id_, out[8].id_);
        out // return only if all pre-conditions hold true
    };
    let expect_slset = {
        let mut store = UT_INIT_DATA_STORE[0].clone();
        store.products.push(all_products[2].clone());
        StockLevelModelSet {
            stores: vec![store],
        }
    };
    let result = stockrepo.save(expect_slset.clone()).await;
    assert!(result.is_ok());
    let pids = {
        let chosen_store = &expect_slset.stores[0];
        vec![ProductStockIdentity {
            store_id: chosen_store.store_id,
            product_id: chosen_store.products[0].id_,
            expiry: chosen_store.products[0].expiry,
        }]
    };
    let result = stockrepo.fetch(pids).await;
    assert!(result.is_ok());
    if let Ok(actual) = result {
        assert!(!ptr::eq(&actual, &expect_slset));
        assert_eq!(actual.stores.len(), 1);
        assert_eq!(actual.stores[0].products.len(), 1);
        verify_stocklvl_model(&actual, &expect_slset, [0, 0], true);
    }
    // ------  subcase 2, start updating
    let expect_slset_ks2 = {
        let mut store = UT_INIT_DATA_STORE[0].clone();
        store.products.extend_from_slice(&all_products[7..9]);
        assert_eq!(store.products.len(), 2);
        StockLevelModelSet {
            stores: vec![store],
        }
    };
    let result = stockrepo.save(expect_slset_ks2.clone()).await;
    assert!(result.is_ok());
    let pids = {
        let chosen_store = &expect_slset_ks2.stores[0];
        vec![
            ProductStockIdentity {
                store_id: chosen_store.store_id,
                product_id: chosen_store.products[0].id_,
                expiry: chosen_store.products[0].expiry,
            },
            ProductStockIdentity {
                store_id: chosen_store.store_id,
                product_id: chosen_store.products[1].id_,
                expiry: chosen_store.products[1].expiry,
            },
        ]
    };
    let result = stockrepo.fetch(pids).await;
    assert!(result.is_ok());
    if let Ok(actual) = result {
        assert!(!ptr::eq(&actual, &expect_slset_ks2));
        assert_eq!(actual.stores[0].products.len(), 2);
        verify_stocklvl_model(&actual, &expect_slset_ks2, [0, 1], true);
        verify_stocklvl_model(&actual, &expect_slset_ks2, [0, 0], true);
        verify_stocklvl_model(&actual, &expect_slset, [0, 0], false);
    }
} // end of fn same_product_diff_expiry

#[tokio::test]
async fn save_dstore_error() {
    let repo = in_mem_repo_ds_setup::<MockInMemDeadDataStore>(4, None).await;
    let stockrepo = repo.stock();
    let pids = vec![ProductStockIdentity {
        store_id: 1001,
        product_id: 9001,
        expiry: DateTime::parse_from_rfc3339("2023-11-09T09:16:01.035-01:00")
            .unwrap()
            .into(),
    }];
    let result = stockrepo.fetch(pids).await;
    assert!(result.is_err());
    let error = result.err().unwrap();
    assert_eq!(error.code, AppErrorCode::AcquireLockFailure);
    assert_eq!(error.detail, Some("utest".to_string()));
}

#[tokio::test]
async fn fetch_dstore_error() {
    let repo = in_mem_repo_ds_setup::<MockInMemDeadDataStore>(4, None).await;
    let stockrepo = repo.stock();
    let all_products = ut_init_data_product();
    let pids = {
        let chosen_store = &UT_INIT_DATA_STORE[0];
        vec![ProductStockIdentity {
            store_id: chosen_store.store_id,
            product_id: all_products[0].id_,
            expiry: all_products[0].expiry,
        }]
    };
    let result = stockrepo.fetch(pids).await;
    assert_eq!(result.is_err(), true);
    let error = result.err().unwrap();
    assert_eq!(error.code, AppErrorCode::AcquireLockFailure);
    assert_eq!(error.detail, Some("utest".to_string()));
}

pub(crate) async fn ut_retrieve_stocklvl_qty(
    stockrepo: Arc<Box<dyn AbsOrderStockRepo>>,
    store_id: u32,
    product: &ProductStockModel,
) -> (u32, u32, u32) {
    let pid = ProductStockIdentity {
        store_id,
        product_id: product.id_,
        expiry: product.expiry,
    };
    let result = stockrepo.fetch(vec![pid]).await;
    assert!(result.is_ok());
    if let Ok(actual) = result {
        assert_eq!(actual.stores.len(), 1);
        assert_eq!(actual.stores[0].products.len(), 1);
        let product_sold = &actual.stores[0].products[0];
        (
            product_sold.quantity.booked,
            product_sold.quantity.cancelled,
            product_sold.quantity.total,
        )
    } else {
        (0, 0, 0)
    }
}

fn mock_reserve_usr_cb_0(
    ms: &mut StockLevelModelSet,
    req: &OrderLineModelSet,
) -> AppStockRepoReserveReturn {
    assert_eq!(ms.stores.len(), 1);
    assert_eq!(req.lines().len(), 1);
    let saved_store = &mut ms.stores[0];
    let id_combo = (req.lines()[0].id().store_id, req.lines()[0].id().product_id);
    let product = match id_combo {
        (1001, 9004) | (1001, 9005) => {
            assert_eq!(saved_store.products.len(), 1);
            let o = &mut saved_store.products[0];
            Some(o)
        }
        (1003, 9004) => {
            assert!(saved_store.products.len() >= 1);
            let o = saved_store
                .products
                .iter_mut()
                .find(|p| p.quantity.total == 22 && p.quantity.cancelled == 8)
                .unwrap();
            Some(o)
        }
        _others => None,
    };
    let product = product.unwrap();
    assert!(product.quantity.rsv_detail.is_none());
    product.quantity.rsv_detail = Some(StockQtyRsvModel {
        oid: req.id().clone(),
        reserved: req.lines()[0].qty.reserved,
    });
    Ok(())
} // end of mock_reserve_usr_cb_0

pub(crate) fn mock_reserve_usr_cb_1(
    ms: &mut StockLevelModelSet,
    req: &OrderLineModelSet,
) -> AppStockRepoReserveReturn {
    for om in req.lines().iter() {
        let result = ms
            .stores
            .iter_mut()
            .find(|m| om.id().store_id == m.store_id);
        assert!(result.is_some());
        if let Some(s) = result {
            let result = s.try_reserve(req.id().as_str(), om);
            assert!(result.is_none());
        }
    }
    Ok(())
} // end of mock_reserve_usr_cb_1

fn ut_setup_order_currency(seller_ids: Vec<u32>) -> OrderCurrencyModel {
    let buyer = CurrencyModel {
        name: CurrencyDto::TWD,
        rate: Decimal::new(32041, 3),
    };
    let seller_c = buyer.clone();
    let kv_pairs = seller_ids
        .into_iter()
        .map(|seller_id| (seller_id, seller_c.clone()));
    OrderCurrencyModel {
        buyer,
        sellers: HashMap::from_iter(kv_pairs),
    }
}

pub(crate) async fn ut_reserve_init_setup(
    stockrepo: Arc<Box<dyn AbsOrderStockRepo>>,
    usr_cb: AppStockRepoReserveUserFunc,
    mock_warranty: DateTime<FixedOffset>,
    store_id: u32,
    product_id: u64,
    num_req: u32,
    order_id: &str,
) {
    let order_req = vec![{
        let id_ = OrderLineIdentity {
            store_id,
            product_id,
        };
        let qty = OrderLineQuantityModel {
            reserved: num_req,
            paid: 0,
            paid_last_update: None,
        };
        let policy = OrderLineAppliedPolicyModel {
            reserved_until: mock_warranty.clone(),
            warranty_until: mock_warranty,
        };
        let price = OrderLinePriceModel::from((4, 4 * num_req));
        let attr_lastupdate = mock_warranty - Duration::days(14);
        let attrs_charge = ProdAttriPriceModel::from((attr_lastupdate, None));
        OrderLineModel::from((id_, price, policy, qty, attrs_charge))
    }];
    let ol_set = {
        let order_id = order_id.to_string();
        let lines = order_req;
        let owner_id = 123;
        let currency = ut_setup_order_currency(vec![store_id]);
        let create_time = DateTime::parse_from_rfc3339("2022-11-07T04:00:00.519-01:00").unwrap();
        OrderLineModelSet::from((order_id, owner_id, create_time, currency, lines))
    };
    let result = stockrepo.try_reserve(usr_cb, &ol_set).await;
    assert!(result.is_ok());
} // end of fn ut_reserve_init_setup

#[tokio::test]
async fn try_reserve_ok() {
    let mock_curr_time = DateTime::parse_from_rfc3339("2022-01-01T18:49:08.035+08:00").unwrap();
    let mock_warranty = DateTime::parse_from_rfc3339("2024-11-28T18:46:08.519-08:00").unwrap();
    let attr_lastupdate = mock_warranty - Duration::days(9);
    let repo = in_mem_repo_ds_setup::<AppInMemoryDStore>(30, Some(mock_curr_time)).await;
    let stockrepo = repo.stock();
    let all_products = ut_init_data_product();
    let expect_slset = {
        let mut stores = UT_INIT_DATA_STORE[..3].to_vec();
        stores[0].products.extend_from_slice(&all_products[0..3]);
        stores[1].products.extend_from_slice(&all_products[3..5]);
        // skip product-item-5, expiry time at product idx 5 is the same as idx 7
        stores[2].products.extend_from_slice(&all_products[6..]);
        StockLevelModelSet { stores }
    };
    let result = stockrepo.save(expect_slset.clone()).await;
    assert!(result.is_ok());

    let reservations = [
        (1001, 9004, 3, "AceMan"),
        (1001, 9004, 1, "BatMan"),
        (1001, 9004, 2, "SpiderMan"),
        (1003, 9004, 2, "Joker"),
        (1003, 9004, 3, "DarkLord"),
    ];

    for (store_id, product_id, num_req, order_id) in reservations {
        ut_reserve_init_setup(
            stockrepo.clone(),
            mock_reserve_usr_cb_0,
            mock_warranty,
            store_id,
            product_id,
            num_req,
            order_id,
        )
        .await;
    }

    {
        // before reservation
        assert_eq!(
            ut_retrieve_stocklvl_qty(stockrepo.clone(), 1001, &all_products[2]).await,
            (3 + 1 + 2, 0, 15)
        );
        assert_eq!(
            ut_retrieve_stocklvl_qty(stockrepo.clone(), 1002, &all_products[3]).await,
            (0, 0, 8)
        );
        assert_eq!(
            ut_retrieve_stocklvl_qty(stockrepo.clone(), 1003, &all_products[7]).await,
            ((3 + 2), 8, 22)
        );
        assert_eq!(
            ut_retrieve_stocklvl_qty(stockrepo.clone(), 1003, &all_products[8]).await,
            (0, 1, 20)
        );
    }

    let order_req_data = [
        (1001, all_products[2].id_, 2, 3, 6),
        (1002, all_products[3].id_, 1, 4, 4),
        (1003, all_products[7].id_, 13, 20, 190),
    ];
    let order_req: Vec<_> = order_req_data
        .iter()
        .map(|&(store_id, product_id, reserved, unit, total)| {
            let id_ = OrderLineIdentity {
                store_id,
                product_id,
            };
            let qty = OrderLineQuantityModel {
                reserved,
                paid: 0,
                paid_last_update: None,
            };
            let policy = OrderLineAppliedPolicyModel {
                reserved_until: mock_warranty,
                warranty_until: mock_warranty,
            };
            let price = OrderLinePriceModel::from((unit, total));
            let attrs_charge = ProdAttriPriceModel::from((attr_lastupdate, None));
            OrderLineModel::from((id_, price, policy, qty, attrs_charge))
        })
        .collect();

    let ol_set = {
        let order_id = "AnotherMan".to_string();
        let lines = order_req;
        let owner_id = 123;
        let currency = ut_setup_order_currency(vec![1001, 1002, 1003]);
        let create_time = DateTime::parse_from_rfc3339("2022-11-07T04:00:00.519-01:00").unwrap();
        OrderLineModelSet::from((order_id, owner_id, create_time, currency, lines))
    };
    let result = stockrepo.try_reserve(mock_reserve_usr_cb_1, &ol_set).await;
    assert!(result.is_ok());
    {
        // after reservation
        assert_eq!(
            ut_retrieve_stocklvl_qty(stockrepo.clone(), 1001, &all_products[2]).await,
            (3 + 1 + 2 + 2, 0, 15)
        );
        assert_eq!(
            ut_retrieve_stocklvl_qty(stockrepo.clone(), 1002, &all_products[3]).await,
            (1, 0, 8)
        );
        let mut total_rsved: u32 = 0;
        for idx in [7usize, 8, 9].into_iter() {
            let opt = ut_retrieve_stocklvl_qty(stockrepo.clone(), 1003, &all_products[idx]).await;
            total_rsved += opt.0;
            // println!("booked, opt : {}", opt.0);
        }
        assert_eq!(total_rsved, (3 + 2 + 13));
    }
} // end of try_reserve_ok

fn mock_reserve_usr_cb_2(
    ms: &mut StockLevelModelSet,
    req: &OrderLineModelSet,
) -> AppStockRepoReserveReturn {
    assert_eq!(ms.stores.len(), 1);
    assert_eq!(ms.stores[0].products.len(), 2);
    let mut out = vec![];
    let result = ms.stores[0]
        .products
        .iter_mut()
        .find(|p| req.lines()[0].id().product_id == p.id_);
    assert!(result.is_some());
    if let Some(p) = result {
        let num_avail = p.quantity.num_avail();
        assert!(p.quantity.total > req.lines()[0].qty.reserved);
        assert!(num_avail > 0);
        assert!(num_avail < req.lines()[0].qty.reserved);
        let err = OrderLineCreateErrorDto {
            seller_id: req.lines()[0].id().store_id,
            product_id: req.lines()[0].id().product_id,
            reason: OrderLineCreateErrorReason::NotEnoughToClaim,
            nonexist: None,
            shortage: None,
            rsv_limit: None,
        };
        out.push(err);
    }
    let result = ms.stores[0]
        .products
        .iter_mut()
        .find(|p| req.lines()[1].id().product_id == p.id_);
    assert!(result.is_some());
    if let Some(p) = result {
        let num_avail = p.quantity.num_avail();
        assert!(p.quantity.total > req.lines()[1].qty.reserved);
        assert!(num_avail == 0);
        assert!(num_avail < req.lines()[1].qty.reserved);
        let err = OrderLineCreateErrorDto {
            seller_id: req.lines()[1].id().store_id,
            product_id: req.lines()[1].id().product_id,
            reason: OrderLineCreateErrorReason::OutOfStock,
            nonexist: None,
            shortage: None,
            rsv_limit: None,
        };
        out.push(err);
    }
    Err(Ok(out))
} // end of mock_reserve_usr_cb_2

#[tokio::test]
async fn try_reserve_shortage() {
    let mock_curr_time = DateTime::parse_from_rfc3339("2022-01-01T18:49:08.035+08:00").unwrap();
    let mock_warranty = DateTime::parse_from_rfc3339("2024-11-28T18:46:08.519-08:00").unwrap();
    let attr_lastupdate = mock_warranty - Duration::days(8);
    let repo = in_mem_repo_ds_setup::<AppInMemoryDStore>(30, Some(mock_curr_time)).await;
    let stockrepo = repo.stock();
    let all_products = ut_init_data_product();
    let expect_slset = {
        let mut stores = UT_INIT_DATA_STORE[..1].to_vec();
        stores[0].products.extend_from_slice(&all_products[0..4]);
        let qty_edit = &mut stores[0].products[0].quantity;
        qty_edit.reserve("CustomerTwo", qty_edit.total - qty_edit.cancelled - 1);
        let qty_edit = &mut stores[0].products[1].quantity;
        qty_edit.reserve("CustomerThree", qty_edit.total - qty_edit.cancelled);
        StockLevelModelSet { stores }
    }; // assume someone already booked for some items
    let result = stockrepo.save(expect_slset.clone()).await;
    assert!(result.is_ok());
    let order_req: Vec<_> = [
        (1001, all_products[0].id_, 3, (4, 11)),
        (1001, all_products[1].id_, 9, (20, 179)),
    ]
    .into_iter()
    .map(|(store_id, product_id, reserved, (unit, total))| {
        OrderLineModel::from((
            OrderLineIdentity {
                store_id,
                product_id,
            },
            OrderLinePriceModel::from((unit, total)),
            OrderLineAppliedPolicyModel {
                reserved_until: mock_warranty,
                warranty_until: mock_warranty,
            },
            OrderLineQuantityModel {
                reserved,
                paid: 0,
                paid_last_update: None,
            },
            ProdAttriPriceModel::from((attr_lastupdate, None)),
        ))
    })
    .collect();
    let ol_set = {
        let order_id = "xx1".to_string();
        let lines = order_req;
        let owner_id = 123;
        let currency = ut_setup_order_currency(vec![1001]);
        let create_time = DateTime::parse_from_rfc3339("2022-11-07T04:00:00.519-01:00").unwrap();
        OrderLineModelSet::from((order_id, owner_id, create_time, currency, lines))
    };
    let result = stockrepo.try_reserve(mock_reserve_usr_cb_2, &ol_set).await;
    assert!(result.is_err());
    if let Err(e) = result {
        assert!(e.is_ok());
        if let Ok(usr_e) = e {
            assert_eq!(usr_e.len(), 2);
            assert!(matches!(
                usr_e[0].reason,
                OrderLineCreateErrorReason::NotEnoughToClaim
            ));
            assert!(matches!(
                usr_e[1].reason,
                OrderLineCreateErrorReason::OutOfStock
            ));
        }
    }
    {
        // after reservation, nothing is changed
        assert_eq!(
            ut_retrieve_stocklvl_qty(stockrepo.clone(), 1001, &all_products[0]).await,
            (4, 0, 5)
        );
        assert_eq!(
            ut_retrieve_stocklvl_qty(stockrepo.clone(), 1001, &all_products[1]).await,
            (11, 0, 11)
        );
    }
} // end of in_mem_try_reserve_shortage

fn mock_reserve_usr_cb_3(
    _ms: &mut StockLevelModelSet,
    _req: &OrderLineModelSet,
) -> AppStockRepoReserveReturn {
    let detail = Some(format!("unit-test"));
    let e = AppError {
        code: AppErrorCode::InvalidInput,
        detail,
    };
    Err(Err(e))
}

#[tokio::test]
async fn try_reserve_user_cb_err() {
    let mock_curr_time = DateTime::parse_from_rfc3339("2022-01-01T18:49:08.035+08:00").unwrap();
    let mock_warranty = DateTime::parse_from_rfc3339("2024-11-28T18:46:08.519-08:00").unwrap();
    let attr_lastupdate = mock_warranty - Duration::minutes(6);
    let repo = in_mem_repo_ds_setup::<AppInMemoryDStore>(30, Some(mock_curr_time)).await;
    let stockrepo = repo.stock();
    let all_products = ut_init_data_product();
    let expect_slset = {
        let mut stores = UT_INIT_DATA_STORE[..1].to_vec();
        stores[0].products.extend_from_slice(&all_products[2..6]);
        StockLevelModelSet { stores }
    }; // assume someone already booked for some items
    let result = stockrepo.save(expect_slset.clone()).await;
    assert!(result.is_ok());
    let order_req = vec![{
        let id_ = OrderLineIdentity {
            store_id: expect_slset.stores[0].store_id,
            product_id: all_products[2].id_,
        };
        let qty = OrderLineQuantityModel {
            reserved: 9,
            paid: 0,
            paid_last_update: None,
        };
        let policy = OrderLineAppliedPolicyModel {
            reserved_until: mock_warranty,
            warranty_until: mock_warranty,
        };
        let price = OrderLinePriceModel::from((20, 179));
        let attrs_charge = ProdAttriPriceModel::from((attr_lastupdate, None));
        OrderLineModel::from((id_, price, policy, qty, attrs_charge))
    }];
    let ol_set = {
        let order_id = "xx1".to_string();
        let lines = order_req;
        let owner_id = 321;
        let currency = ut_setup_order_currency(vec![expect_slset.stores[0].store_id]);
        let create_time = DateTime::parse_from_rfc3339("2022-11-07T04:00:00.519-01:00").unwrap();
        OrderLineModelSet::from((order_id, owner_id, create_time, currency, lines))
    };
    let result = stockrepo.try_reserve(mock_reserve_usr_cb_3, &ol_set).await;
    assert!(result.is_err());
    if let Err(e) = result {
        assert!(e.is_err());
        if let Err(internal_e) = e {
            assert_eq!(internal_e.code, AppErrorCode::InvalidInput);
            assert_eq!(internal_e.detail.as_ref().unwrap().as_str(), "unit-test");
        }
    }
} // end of try_reserve_user_cb_err

fn mock_return_usr_cb_1(
    ms: &mut StockLevelModelSet,
    data: StockLevelReturnDto,
) -> Vec<StockReturnErrorDto> {
    let d_item = &data.items[0];
    assert_eq!(ms.stores.len(), 1);
    assert_eq!(d_item.store_id, ms.stores[0].store_id);
    let result = ms.stores[0]
        .products
        .iter_mut()
        .find(|p| d_item.product_id == p.id_);
    assert!(result.is_some());
    if let Some(v) = result {
        let num_req = d_item.qty_add as u32;
        let num_returned = v.quantity.try_return(num_req);
        assert!(num_req > 0);
        assert_eq!(num_req, num_returned);
        let rsv_detail = v.quantity.rsv_detail.as_ref().unwrap();
        assert_eq!(rsv_detail.oid, data.order_id);
        let expect = match rsv_detail.oid.as_str() {
            "AceMan" => (7, 3),
            "BatMan" => (6, 0),
            _others => (9999, 9999),
        };
        assert_eq!((v.quantity.booked, rsv_detail.reserved), expect);
    }
    vec![]
}

#[tokio::test]
async fn try_return_ok() {
    let mock_curr_time = DateTime::parse_from_rfc3339("2022-11-28T18:46:08.519-08:00").unwrap();
    let mock_rsv_expiry = DateTime::parse_from_rfc3339("2022-11-28T19:15:12.101-08:00").unwrap();
    let mock_warranty = mock_rsv_expiry + Duration::minutes(10);
    let repo = in_mem_repo_ds_setup::<AppInMemoryDStore>(30, Some(mock_curr_time)).await;
    let stockrepo = repo.stock();
    let all_products = ut_init_data_product();
    let expect_slset = {
        let mut stores = UT_INIT_DATA_STORE[..1].to_vec();
        stores[0].products.extend_from_slice(&all_products[0..4]);
        StockLevelModelSet { stores }
    };
    let result = stockrepo.save(expect_slset.clone()).await;
    assert!(result.is_ok());

    let reservations = [
        (1001, 9004, 4, "AceMan"),
        (1001, 9004, 1, "BatMan"),
        (1001, 9004, 3, "SpiderMan"),
    ];
    for (store_id, product_id, num_req, order_id) in reservations {
        ut_reserve_init_setup(
            stockrepo.clone(),
            mock_reserve_usr_cb_0,
            mock_warranty,
            store_id,
            product_id,
            num_req,
            order_id,
        )
        .await;
    }

    let data = StockLevelReturnDto {
        order_id: format!("AceMan"),
        items: vec![InventoryEditStockLevelDto {
            qty_add: 1,
            expiry: mock_rsv_expiry,
            store_id: 1001,
            product_id: 9004,
        }],
    };
    let result = stockrepo.try_return(mock_return_usr_cb_1, data).await;
    assert!(result.is_ok());
    if let Ok(v) = result {
        assert_eq!(v.len(), 0);
    }
    let data = StockLevelReturnDto {
        order_id: format!("BatMan"),
        items: vec![InventoryEditStockLevelDto {
            qty_add: 1,
            expiry: mock_rsv_expiry,
            store_id: 1001,
            product_id: 9004,
        }],
    };
    let result = stockrepo.try_return(mock_return_usr_cb_1, data).await;
    assert!(result.is_ok());
    if let Ok(v) = result {
        assert_eq!(v.len(), 0);
    }
    {
        // after stock return
        let pid = ProductStockIdentity {
            store_id: 1001,
            product_id: 9004,
            expiry: expect_slset.stores[0].products[2].expiry.clone(),
        };
        let result = stockrepo.fetch(vec![pid]).await;
        assert!(result.is_ok());
        if let Ok(ms) = result {
            let actual_booked = ms.stores[0].products[0].quantity.booked;
            assert_eq!(actual_booked, 6u32);
        }
    }
} // end of fn  try_return_ok

fn mock_return_usr_cb_2(
    ms: &mut StockLevelModelSet,
    data: StockLevelReturnDto,
) -> Vec<StockReturnErrorDto> {
    assert_eq!(ms.stores.len(), 2);
    assert_eq!(data.items.len(), 2);
    vec![
        StockReturnErrorDto {
            seller_id: data.items[0].store_id,
            product_id: data.items[0].product_id,
            reason: StockReturnErrorReason::NotExist,
        },
        StockReturnErrorDto {
            seller_id: data.items[1].store_id,
            product_id: data.items[1].product_id,
            reason: StockReturnErrorReason::InvalidQuantity,
        },
    ]
}

#[tokio::test]
async fn try_return_input_err() {
    let mock_curr_time = DateTime::parse_from_rfc3339("2022-11-28T18:46:08.519-08:00").unwrap();
    let mock_warranty = DateTime::parse_from_rfc3339("2022-11-28T17:15:12.101-08:00").unwrap();
    let repo = in_mem_repo_ds_setup::<AppInMemoryDStore>(30, Some(mock_curr_time)).await;
    let stockrepo = repo.stock();
    let all_products = ut_init_data_product();
    let expect_slset = {
        let mut stores = UT_INIT_DATA_STORE[..3].to_vec();
        stores[0].products.extend_from_slice(&all_products[0..4]);
        stores[2].products.extend_from_slice(&all_products[4..8]);
        StockLevelModelSet { stores }
    };
    let result = stockrepo.save(expect_slset.clone()).await;
    assert!(result.is_ok());
    ut_reserve_init_setup(
        stockrepo.clone(),
        mock_reserve_usr_cb_0,
        mock_warranty,
        1001,
        9005,
        2,
        "AceMan",
    )
    .await;
    ut_reserve_init_setup(
        stockrepo.clone(),
        mock_reserve_usr_cb_0,
        mock_warranty,
        1003,
        9004,
        5,
        "AceMan",
    )
    .await;
    let data = StockLevelReturnDto {
        order_id: format!("AceMan"),
        items: vec![
            InventoryEditStockLevelDto {
                qty_add: 7,
                expiry: mock_warranty,
                store_id: 1001,
                product_id: expect_slset.stores[0].products[3].id_,
            },
            InventoryEditStockLevelDto {
                qty_add: 8,
                expiry: mock_warranty,
                store_id: 1003,
                product_id: expect_slset.stores[2].products[3].id_,
            },
        ],
    };
    let result = stockrepo.try_return(mock_return_usr_cb_2, data).await;
    assert!(result.is_ok());
    if let Ok(v) = result {
        assert_eq!(v.len(), 2);
        let pid = ProductStockIdentity {
            store_id: 1003,
            product_id: 9004,
            expiry: expect_slset.stores[2].products[3].expiry.clone(),
        };
        let result = stockrepo.fetch(vec![pid]).await;
        assert!(result.is_ok());
        if let Ok(ms) = result {
            let actual_booked = ms.stores[0].products[0].quantity.booked;
            assert_eq!(actual_booked, 5);
        } // should not be modified
    }
} // end of fn  try_return_input_err
