use std::ptr;
use std::sync::Arc;
use chrono::{DateTime, Local, FixedOffset};

use order::api::web::dto::{OrderLineCreateErrorDto, OrderLineErrorReason};
use order::constant::ProductType;
use order::error::{AppErrorCode, AppError};
use order::model::{
    StockLevelModelSet, StoreStockModel, ProductStockModel, StockQuantityModel,
    ProductStockIdentity, OrderLineModel, OrderLinePriceModel, OrderLineAppliedPolicyModel
};
use order::repository::{OrderInMemRepo, AbsOrderRepo, AppStockRepoReserveReturn, AbsOrderStockRepo};
use order::datastore::{AppInMemoryDStore, AbstInMemoryDStore};

use crate::model::verify_stocklvl_model;
use crate::repository::{in_mem_ds_ctx_setup, MockInMemDeadDataStore};

fn in_mem_repo_ds_setup<T:AbstInMemoryDStore + 'static>(
    nitems:u32, mut curr_time:Option<DateTime<FixedOffset>> ) -> OrderInMemRepo
{
    if curr_time.is_none() {
        curr_time = Some(Local::now().into());
    }
    let ds = in_mem_ds_ctx_setup::<T>(nitems);
    let result = OrderInMemRepo::build(ds, curr_time.unwrap());
    assert_eq!(result.is_ok(), true);
    result.unwrap()
}

fn ut_init_data_product() -> [ProductStockModel;10]
{
    [   // ------ for insertion --------
        ProductStockModel { type_:ProductType::Item, id_:9002, is_create:true,
           expiry:DateTime::parse_from_rfc3339("2023-10-05T08:14:05+09:00").unwrap(),
           quantity: StockQuantityModel {total:5, booked:0, cancelled:0}
        },
        ProductStockModel { type_:ProductType::Package, id_:9003, is_create:true,
           expiry:DateTime::parse_from_rfc3339("2023-11-07T08:12:05.008+02:00").unwrap(),
           quantity: StockQuantityModel {total:11, booked:0, cancelled:0}
        },
        ProductStockModel { type_:ProductType::Package, id_:9004, is_create:true,
           expiry:DateTime::parse_from_rfc3339("2023-11-09T09:16:01.029-01:00").unwrap(),
           quantity: StockQuantityModel {total:15, booked:0, cancelled:0}
        },
        ProductStockModel { type_:ProductType::Item, id_:9005, is_create:true,
           expiry:DateTime::parse_from_rfc3339("2024-11-11T09:22:01.005+08:00").unwrap(),
           quantity: StockQuantityModel {total:8, booked:0, cancelled:0}
        },
        ProductStockModel { type_:ProductType::Item, id_:9006, is_create:true,
           expiry:DateTime::parse_from_rfc3339("2024-11-15T09:23:58.098+01:00").unwrap(),
           quantity: StockQuantityModel {total:14, booked:0, cancelled:0}
        },
        // ---------------------
        ProductStockModel { type_:ProductType::Package, id_:9004, is_create:false,
           expiry:DateTime::parse_from_rfc3339("2023-11-09T09:16:01.029-01:00").unwrap(),
           quantity: StockQuantityModel {total:15, booked:0, cancelled:7}
        },
        ProductStockModel { type_:ProductType::Item, id_:9006, is_create:false,
           expiry:DateTime::parse_from_rfc3339("2024-11-15T09:23:58.098+01:00").unwrap(),
           quantity: StockQuantityModel {total:18, booked:0, cancelled:1}
        },
        // ---------------------
        ProductStockModel { type_:ProductType::Package, id_:9004, is_create:false,
           expiry:DateTime::parse_from_rfc3339("2023-11-09T09:16:01.035-01:00").unwrap(),
           quantity: StockQuantityModel {total:22, booked:0, cancelled:8}
        },
        ProductStockModel { type_:ProductType::Package, id_:9004, is_create:true,
           expiry:DateTime::parse_from_rfc3339("2023-11-09T12:30:10.035-01:00").unwrap(),
           quantity: StockQuantityModel {total:20, booked:0, cancelled:1}
        },
        ProductStockModel { type_:ProductType::Package, id_:9004, is_create:true,
           expiry:DateTime::parse_from_rfc3339("2020-03-15T12:55:08.035-11:00").unwrap(),
           quantity: StockQuantityModel {total:18, booked:0, cancelled:3}
        },
    ]
} // end of ut_init_data_product

const UT_INIT_DATA_STORE: [StoreStockModel; 4] = 
[
    StoreStockModel {store_id:1001, products:vec![]},
    StoreStockModel {store_id:1002, products:vec![]},
    StoreStockModel {store_id:1003, products:vec![]},
    StoreStockModel {store_id:1004, products:vec![]},
]; // end of ut_init_data_store

#[tokio::test]
async fn in_mem_save_fetch_ok ()
{ // this test case verifies product stock level, each has different product ID
    let repo = in_mem_repo_ds_setup::<AppInMemoryDStore>(12, None);
    let stockrepo = repo.stock();
    let all_products = ut_init_data_product();
    let expect_slset = {
        let mut stores = UT_INIT_DATA_STORE[..2].to_vec();
        assert_eq!(stores.len(), 2);
        stores[0].products.extend_from_slice(&all_products[0..2]);
        stores[1].products.extend_from_slice(&all_products[2..5]);
        assert_eq!(stores[0].products.len(), 2);
        assert_eq!(stores[1].products.len(), 3);
        StockLevelModelSet {stores}
    };
    let result = stockrepo.save(expect_slset.clone()).await;
    assert!(result.is_ok());
    let pids = expect_slset.stores.iter().flat_map(|m1| {
        m1.products.iter().map(
            |m2| ProductStockIdentity {store_id:m1.store_id, product_type:m2.type_.clone(),
                product_id:m2.id_,  expiry:m2.expiry_without_millis()}
        )
    }).collect();
    let result = stockrepo.fetch(pids).await;
    assert!(result.is_ok());
    if let Ok(actual) = result {
        assert!( ! ptr::eq(&actual, &expect_slset) );
        assert_eq!(actual.stores.len(), expect_slset.stores.len());
        verify_stocklvl_model(&actual, &expect_slset, [1,1], true);
        verify_stocklvl_model(&actual, &expect_slset, [0,1], true);
        verify_stocklvl_model(&actual, &expect_slset, [1,0], true);
        verify_stocklvl_model(&actual, &expect_slset, [1,2], true);
        verify_stocklvl_model(&actual, &expect_slset, [0,0], true);
    }
} // end of in_mem_save_fetch_ok


#[tokio::test]
async fn in_mem_update_existing_ok ()
{
    let repo = in_mem_repo_ds_setup::<AppInMemoryDStore>(8, None);
    let stockrepo = repo.stock();
    let all_products = ut_init_data_product();
    let mut expect_slset = {
        let mut stores = UT_INIT_DATA_STORE[0..1].to_vec();
        stores[0].products.extend_from_slice(&all_products[..5]);
        assert_eq!(stores[0].products.len(), 5);
        StockLevelModelSet {stores}
    };
    let result = stockrepo.save(expect_slset.clone()).await;
    assert!(result.is_ok());
    let pids = {
        let chosen_store = &expect_slset.stores[0];
        vec![
            ProductStockIdentity { store_id: chosen_store.store_id,
                product_type: chosen_store.products[2].type_.clone(),
                product_id:   chosen_store.products[2].id_,
                expiry:    chosen_store.products[2].expiry  },
            ProductStockIdentity { store_id:chosen_store.store_id,
                product_type: chosen_store.products[4].type_.clone(),
                product_id:   chosen_store.products[4].id_,
                expiry:    chosen_store.products[4].expiry  },
        ]
    };
    let result = stockrepo.fetch(pids.clone()).await;
    assert!(result.is_ok());
    if let Ok(actual) = result {
        assert!( ! ptr::eq(&actual, &expect_slset) );
        assert_eq!(actual.stores.len(), 1);
        assert_eq!(actual.stores[0].products.len(), 2);
        verify_stocklvl_model(&actual, &expect_slset, [0,4], true);
        verify_stocklvl_model(&actual, &expect_slset, [0,2], true);
    }
    // ------  subcase 2, start updating
    let expect_slset_ks2 = {
        let mut stores = UT_INIT_DATA_STORE[0..1].to_vec();
        stores[0].products.extend_from_slice(&all_products[5..7]);
        assert_eq!(stores[0].products.len(), 2);
        StockLevelModelSet {stores}
    };
    let result = stockrepo.save(expect_slset_ks2.clone()).await;
    assert!(result.is_ok());
    let result = stockrepo.fetch(pids).await;
    assert!(result.is_ok());
    if let Ok(actual) = result {
        assert!( ! ptr::eq(&actual, &expect_slset_ks2) );
        assert_eq!(actual.stores[0].products.len(), 2);
        verify_stocklvl_model(&actual, &expect_slset_ks2, [0,0], true);
        verify_stocklvl_model(&actual, &expect_slset_ks2, [0,1], true);
        // discard other items except idx 2 and 4
        expect_slset.stores[0].products.remove(0);
        expect_slset.stores[0].products.remove(0);
        expect_slset.stores[0].products.remove(1);
        verify_stocklvl_model(&actual, &expect_slset, [0,0], false);
        verify_stocklvl_model(&actual, &expect_slset, [0,1], false);
    }
} // end of fn in_mem_update_existing_ok


#[tokio::test]
async fn in_mem_same_product_diff_expiry ()
{
    let repo = in_mem_repo_ds_setup::<AppInMemoryDStore>(8, None);
    let stockrepo = repo.stock();
    let all_products = {
        let out = ut_init_data_product();
        assert_eq!((&out[2].type_, out[2].id_), (&out[7].type_, out[7].id_));
        assert_eq!((&out[2].type_, out[2].id_), (&out[8].type_, out[8].id_));
        out // return only if all pre-conditions hold true
    };
    let expect_slset = {
        let mut store = UT_INIT_DATA_STORE[0].clone();
        store.products.push(all_products[2].clone());
        StockLevelModelSet {stores:vec![store]}
    };
    let result = stockrepo.save(expect_slset.clone()).await;
    assert!(result.is_ok());
    let pids = {
        let chosen_store = &expect_slset.stores[0];
        vec![
            ProductStockIdentity { store_id: chosen_store.store_id,
                product_type: chosen_store.products[0].type_.clone(),
                product_id:   chosen_store.products[0].id_,
                expiry:    chosen_store.products[0].expiry  },
        ]
    };
    let result = stockrepo.fetch(pids).await;
    assert!(result.is_ok());
    if let Ok(actual) = result {
        assert!( ! ptr::eq(&actual, &expect_slset) );
        assert_eq!(actual.stores.len(), 1);
        assert_eq!(actual.stores[0].products.len(), 1);
        verify_stocklvl_model(&actual, &expect_slset, [0,0], true);
    }
    // ------  subcase 2, start updating
    let expect_slset_ks2 = {
        let mut store = UT_INIT_DATA_STORE[0].clone();
        store.products.extend_from_slice(&all_products[7..9]);
        assert_eq!(store.products.len(), 2);
        StockLevelModelSet {stores:vec![store]}
    };
    let result = stockrepo.save(expect_slset_ks2.clone()).await;
    assert!(result.is_ok());
    let pids = {
        let chosen_store = &expect_slset_ks2.stores[0];
        vec![
            ProductStockIdentity { store_id: chosen_store.store_id,
                product_type: chosen_store.products[0].type_.clone(),
                product_id:   chosen_store.products[0].id_,
                expiry:    chosen_store.products[0].expiry  },
            ProductStockIdentity { store_id: chosen_store.store_id,
                product_type: chosen_store.products[1].type_.clone(),
                product_id:   chosen_store.products[1].id_,
                expiry:    chosen_store.products[1].expiry  },
        ]
    };
    let result = stockrepo.fetch(pids).await;
    assert!(result.is_ok());
    if let Ok(actual) = result {
        assert!( ! ptr::eq(&actual, &expect_slset_ks2) );
        assert_eq!(actual.stores[0].products.len(), 2);
        verify_stocklvl_model(&actual, &expect_slset_ks2, [0,1], true);
        verify_stocklvl_model(&actual, &expect_slset_ks2, [0,0], true);
        verify_stocklvl_model(&actual, &expect_slset, [0,0], false);
    }
} // end of fn in_mem_same_product_diff_expiry


#[tokio::test]
async fn in_mem_save_dstore_error ()
{
    let repo = in_mem_repo_ds_setup::<MockInMemDeadDataStore>(4, None);
    let stockrepo = repo.stock();
    let all_products = ut_init_data_product();
    let expect_slset = {
        let mut store = UT_INIT_DATA_STORE[0].clone();
        store.products.push(all_products[0].clone());
        StockLevelModelSet {stores:vec![store]}
    };
    let result = stockrepo.save(expect_slset).await;
    assert!(result.is_err());
    let error = result.err().unwrap();
    assert_eq!(error.code, AppErrorCode::DataTableNotExist);
    assert_eq!(error.detail, Some("utest".to_string()));
}

#[tokio::test]
async fn in_mem_fetch_dstore_error ()
{
    let repo = in_mem_repo_ds_setup::<MockInMemDeadDataStore>(4, None);
    let stockrepo = repo.stock();
    let all_products = ut_init_data_product();
    let pids = {
        let chosen_store = &UT_INIT_DATA_STORE[0];
        vec![
            ProductStockIdentity { store_id: chosen_store.store_id,
                product_type: all_products[0].type_.clone(),
                product_id:   all_products[0].id_,
                expiry:    all_products[0].expiry  },
        ]
    };
    let result = stockrepo.fetch(pids).await;
    assert_eq!(result.is_err(), true);
    let error = result.err().unwrap();
    assert_eq!(error.code, AppErrorCode::AcquireLockFailure);
    assert_eq!(error.detail, Some("utest".to_string()));
}

async fn ut_retrieve_stocklvl_qty (stockrepo:Arc<Box<dyn AbsOrderStockRepo>>,
                                   pid:ProductStockIdentity ) -> (u32, u32, u32)
{
    let result = stockrepo.fetch(vec![pid]).await;
    assert!(result.is_ok());
    if let Ok(actual) = result {
        assert_eq!(actual.stores.len(), 1);
        assert_eq!(actual.stores[0].products.len(), 1);
        let product_sold = &actual.stores[0].products[0];
        (product_sold.quantity.booked, product_sold.quantity.cancelled,
         product_sold.quantity.total)
    } else { (0,0,0) }
}

fn mock_reserve_usr_cb_1 (ms:&mut StockLevelModelSet, req:&Vec<OrderLineModel>)
    -> AppStockRepoReserveReturn
{
    assert_eq!(ms.stores.len(), 3);
    for om in req.iter() {
        let result = ms.stores.iter_mut().find(|m| {om.seller_id == m.store_id});
        assert!(result.is_some());
        if let Some(s) = result {
            let result = s.products.iter_mut().find(|p| {
                om.product_type == p.type_ && om.product_id == p.id_
            });
            assert!(result.is_some());
            if let Some(p) = result {
                let num_avail = p.quantity.total - p.quantity.cancelled;
                assert!(p.quantity.total > om.qty);
                assert!(num_avail > 0);
                assert!(num_avail > om.qty);
                p.quantity.booked += om.qty;
            }
        }
    }
    Ok(())
} // end of mock_reserve_usr_cb_1


#[tokio::test]
async fn in_mem_try_reserve_ok ()
{
    let mock_curr_time = DateTime::parse_from_rfc3339("2022-01-01T18:49:08.035+08:00").unwrap();
    let mock_warranty  = DateTime::parse_from_rfc3339("2024-11-28T18:46:08.519-08:00").unwrap();
    let repo = in_mem_repo_ds_setup::<AppInMemoryDStore>(30, Some(mock_curr_time));
    let stockrepo = repo.stock();
    let all_products = ut_init_data_product();
    let expect_slset = {
        let mut stores = UT_INIT_DATA_STORE[..3].to_vec();
        stores[0].products.extend_from_slice(&all_products[0..3]);
        stores[1].products.extend_from_slice(&all_products[3..5]);
        stores[2].products.extend_from_slice(&all_products[5..]);
        StockLevelModelSet {stores}
    };
    let result = stockrepo.save(expect_slset.clone()).await;
    assert!(result.is_ok());
    { // before reservation
        let pid = ProductStockIdentity { store_id: expect_slset.stores[0].store_id, product_id: all_products[2].id_,
                product_type: all_products[2].type_.clone(),  expiry: all_products[2].expiry} ;
        assert_eq!(ut_retrieve_stocklvl_qty (stockrepo.clone(), pid).await, (0, 0, 15)) ;
        let pid = ProductStockIdentity { store_id: expect_slset.stores[1].store_id, product_id: all_products[3].id_,
                product_type: all_products[3].type_.clone(),  expiry: all_products[3].expiry} ;
        assert_eq!(ut_retrieve_stocklvl_qty (stockrepo.clone(), pid).await, (0, 0, 8));
        let pid = ProductStockIdentity { store_id: expect_slset.stores[2].store_id, product_id: all_products[7].id_,
                product_type: all_products[7].type_.clone(),  expiry: all_products[7].expiry} ;
        assert_eq!(ut_retrieve_stocklvl_qty (stockrepo.clone(), pid).await, (0, 8, 22));
        let pid = ProductStockIdentity { store_id: expect_slset.stores[2].store_id, product_id: all_products[8].id_,
                product_type: all_products[8].type_.clone(),  expiry: all_products[8].expiry} ;
        assert_eq!(ut_retrieve_stocklvl_qty (stockrepo.clone(), pid).await, (0, 1, 20));
    }
    let order_req = vec![
        OrderLineModel {
            seller_id: expect_slset.stores[0].store_id, product_id: all_products[2].id_, qty:2,
            product_type: all_products[2].type_.clone(), price: OrderLinePriceModel {unit:3, total:6}
            , policy: OrderLineAppliedPolicyModel { reserved_until: mock_warranty.clone(),
                warranty_until: mock_warranty.clone() }
        },
        OrderLineModel {
            seller_id: expect_slset.stores[1].store_id, product_id: all_products[3].id_, qty:1,
            product_type: all_products[3].type_.clone(), price: OrderLinePriceModel {unit:4, total:4}
            , policy: OrderLineAppliedPolicyModel { reserved_until: mock_warranty.clone(),
                warranty_until: mock_warranty.clone() }
        },
        OrderLineModel {
            seller_id: expect_slset.stores[2].store_id, product_id: all_products[7].id_, qty:13,
            product_type: all_products[7].type_.clone(), price: OrderLinePriceModel {unit:20, total:190}
            , policy: OrderLineAppliedPolicyModel { reserved_until: mock_warranty.clone(),
                warranty_until: mock_warranty.clone() }
        },
    ];
    let result = stockrepo.try_reserve(mock_reserve_usr_cb_1, &order_req).await;
    assert!(result.is_ok());
    { // after reservation
        let pid = ProductStockIdentity { store_id: expect_slset.stores[0].store_id, product_id: all_products[2].id_,
                product_type: all_products[2].type_.clone(),  expiry: all_products[2].expiry} ;
        assert_eq!(ut_retrieve_stocklvl_qty (stockrepo.clone(), pid).await, (2, 0, 15));
        let pid = ProductStockIdentity { store_id: expect_slset.stores[1].store_id, product_id: all_products[3].id_,
                product_type: all_products[3].type_.clone(),  expiry: all_products[3].expiry} ;
        assert_eq!(ut_retrieve_stocklvl_qty (stockrepo.clone(), pid).await, (1, 0, 8));
        let pid = ProductStockIdentity { store_id: expect_slset.stores[2].store_id, product_id: all_products[7].id_,
                product_type: all_products[7].type_.clone(),  expiry: all_products[7].expiry} ;
        let opt0 = ut_retrieve_stocklvl_qty (stockrepo.clone(), pid).await;
        let pid = ProductStockIdentity { store_id: expect_slset.stores[2].store_id, product_id: all_products[8].id_,
                product_type: all_products[8].type_.clone(),  expiry: all_products[8].expiry} ;
        let opt1 = ut_retrieve_stocklvl_qty (stockrepo.clone(), pid).await;
        assert!((opt0.0 == 13) || (opt1.0 == 13));
    }
} // end of in_mem_try_reserve_ok



fn mock_reserve_usr_cb_2 (ms:&mut StockLevelModelSet, req:&Vec<OrderLineModel>)
    -> AppStockRepoReserveReturn
{
    assert_eq!(ms.stores.len(), 1);
    assert_eq!(ms.stores[0].products.len(), 2);
    let mut out = vec![];
    let result = ms.stores[0].products.iter_mut().find(|p| {
        req[0].product_type == p.type_ && req[0].product_id == p.id_
    });
    assert!(result.is_some());
    if let Some(p) = result {
        let num_avail = p.quantity.total - p.quantity.booked - p.quantity.cancelled;
        assert!(p.quantity.total > req[0].qty);
        assert!(num_avail > 0);
        assert!(num_avail < req[0].qty);
        let err = OrderLineCreateErrorDto { seller_id: req[0].seller_id, product_id: req[0].product_id,
            product_type: req[0].product_type.clone(), reason: OrderLineErrorReason::NotEnoughToClaim,
            nonexist:None };
        out.push(err);
    }
    let result = ms.stores[0].products.iter_mut().find(|p| {
        req[1].product_type == p.type_ && req[1].product_id == p.id_
    });
    assert!(result.is_some());
    if let Some(p) = result {
        let num_avail = p.quantity.total - p.quantity.booked - p.quantity.cancelled;
        assert!(p.quantity.total > req[1].qty);
        assert!(num_avail == 0);
        assert!(num_avail < req[1].qty);
        let err = OrderLineCreateErrorDto { seller_id: req[1].seller_id, product_id: req[1].product_id,
            product_type: req[1].product_type.clone(), reason: OrderLineErrorReason::OutOfStock,
            nonexist:None };
        out.push(err);
    }
    Err(Ok(out))
} // end of mock_reserve_usr_cb_2

#[tokio::test]
async fn in_mem_try_reserve_shortage ()
{
    let mock_curr_time = DateTime::parse_from_rfc3339("2022-01-01T18:49:08.035+08:00").unwrap();
    let mock_warranty  = DateTime::parse_from_rfc3339("2024-11-28T18:46:08.519-08:00").unwrap();
    let repo = in_mem_repo_ds_setup::<AppInMemoryDStore>(30, Some(mock_curr_time));
    let stockrepo = repo.stock();
    let all_products = ut_init_data_product();
    let expect_slset = {
        let mut stores = UT_INIT_DATA_STORE[..1].to_vec();
        stores[0].products.extend_from_slice(&all_products[0..4]);
        let qty_edit = &mut stores[0].products[0].quantity;
        qty_edit.booked = qty_edit.total - qty_edit.cancelled - 1;
        let qty_edit = &mut stores[0].products[1].quantity;
        qty_edit.booked = qty_edit.total - qty_edit.cancelled;
        StockLevelModelSet {stores}
    }; // assume someone already booked for some items
    let result = stockrepo.save(expect_slset.clone()).await;
    assert!(result.is_ok());
    let order_req = vec![
        OrderLineModel {
            seller_id: expect_slset.stores[0].store_id, product_id: all_products[0].id_, qty:3,
            product_type: all_products[0].type_.clone(), price: OrderLinePriceModel {unit:4, total:11}
            , policy: OrderLineAppliedPolicyModel { reserved_until: mock_warranty.clone(),
                warranty_until: mock_warranty.clone() }
        },
        OrderLineModel {
            seller_id: expect_slset.stores[0].store_id, product_id: all_products[1].id_, qty:9,
            product_type: all_products[1].type_.clone(), price: OrderLinePriceModel {unit:20, total:179}
            , policy: OrderLineAppliedPolicyModel { reserved_until: mock_warranty.clone(),
                warranty_until: mock_warranty.clone() }
        },
    ];
    let result = stockrepo.try_reserve(mock_reserve_usr_cb_2, &order_req).await;
    assert!(result.is_err());
    if let Err(e) = result {
        assert!(e.is_ok());
        if let Ok(usr_e) = e {
            assert_eq!(usr_e.len(), 2);
            assert!(matches!(usr_e[0].reason, OrderLineErrorReason::NotEnoughToClaim));
            assert!(matches!(usr_e[1].reason, OrderLineErrorReason::OutOfStock));
        }
    } { // after reservation, nothing is changed
        let pid = ProductStockIdentity { store_id: expect_slset.stores[0].store_id, product_id: all_products[0].id_,
                product_type: all_products[0].type_.clone(),  expiry: all_products[0].expiry} ;
        assert_eq!(ut_retrieve_stocklvl_qty (stockrepo.clone(), pid).await, (4, 0, 5));
        let pid = ProductStockIdentity { store_id: expect_slset.stores[0].store_id, product_id: all_products[1].id_,
                product_type: all_products[1].type_.clone(),  expiry: all_products[1].expiry} ;
        assert_eq!(ut_retrieve_stocklvl_qty (stockrepo.clone(), pid).await, (11, 0, 11));
    }
} // end of in_mem_try_reserve_shortage


fn mock_reserve_usr_cb_3 (_ms:&mut StockLevelModelSet, _req:&Vec<OrderLineModel>)
    -> AppStockRepoReserveReturn
{
    let detail = Some(format!("unit-test"));
    let e = AppError { code:AppErrorCode::InvalidInput, detail };
    Err(Err(e))
}

#[tokio::test]
async fn in_mem_try_reserve_user_cb_err ()
{
    let mock_curr_time = DateTime::parse_from_rfc3339("2022-01-01T18:49:08.035+08:00").unwrap();
    let mock_warranty  = DateTime::parse_from_rfc3339("2024-11-28T18:46:08.519-08:00").unwrap();
    let repo = in_mem_repo_ds_setup::<AppInMemoryDStore>(30, Some(mock_curr_time));
    let stockrepo = repo.stock();
    let all_products = ut_init_data_product();
    let expect_slset = {
        let mut stores = UT_INIT_DATA_STORE[..1].to_vec();
        stores[0].products.extend_from_slice(&all_products[2..6]);
        StockLevelModelSet {stores}
    }; // assume someone already booked for some items
    let result = stockrepo.save(expect_slset.clone()).await;
    assert!(result.is_ok());
    let order_req = vec![
        OrderLineModel {
            seller_id: expect_slset.stores[0].store_id, product_id: all_products[2].id_, qty:9,
            product_type: all_products[2].type_.clone(), price: OrderLinePriceModel {unit:20, total:179}
            , policy: OrderLineAppliedPolicyModel { reserved_until: mock_warranty.clone(),
                warranty_until: mock_warranty.clone() }
        },
    ];
    let result = stockrepo.try_reserve(mock_reserve_usr_cb_3, &order_req).await;
    assert!(result.is_err());
    if let Err(e) = result {
        assert!(e.is_err());
        if let Err(internal_e) = e {
            assert_eq!(internal_e.code, AppErrorCode::InvalidInput);
            assert_eq!(internal_e.detail.as_ref().unwrap().as_str(), "unit-test");
        }
    }
} // end of in_mem_try_reserve_user_cb_err

