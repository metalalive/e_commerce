use std::ptr;
use chrono::DateTime;

use order::error::AppErrorCode;
use order::model::{StockLevelModelSet, StoreStockModel, ProductStockModel, StockQuantityModel, ProductStockIdentity};
use order::repository::{OrderInMemRepo, AbsOrderRepo};
use order::datastore::{AppInMemoryDStore, AbstInMemoryDStore};

use super::{in_mem_ds_ctx_setup, MockInMemDeadDataStore};

fn in_mem_repo_ds_setup<T:AbstInMemoryDStore + 'static>(nitems:u32) -> OrderInMemRepo
{
    let ds = in_mem_ds_ctx_setup::<T>(nitems);
    let result = OrderInMemRepo::build(ds);
    assert_eq!(result.is_ok(), true);
    result.unwrap()
}

fn ut_init_data_product() -> [ProductStockModel;9]
{
    [   // ------ for insertion --------
        ProductStockModel { type_:1, id_:9002, is_create:true,
           expiry:DateTime::parse_from_rfc3339("2023-10-05T08:14:05+09:00").unwrap(),
           quantity: StockQuantityModel {total:5, booked:0, cancelled:0}
        },
        ProductStockModel { type_:2, id_:9003, is_create:true,
           expiry:DateTime::parse_from_rfc3339("2023-11-07T08:12:05.008+02:00").unwrap(),
           quantity: StockQuantityModel {total:11, booked:0, cancelled:0}
        },
        ProductStockModel { type_:2, id_:9004, is_create:true,
           expiry:DateTime::parse_from_rfc3339("2023-11-09T09:16:01.029-01:00").unwrap(),
           quantity: StockQuantityModel {total:15, booked:0, cancelled:0}
        },
        ProductStockModel { type_:1, id_:9005, is_create:true,
           expiry:DateTime::parse_from_rfc3339("2024-11-11T09:22:01.005+08:00").unwrap(),
           quantity: StockQuantityModel {total:8, booked:0, cancelled:0}
        },
        ProductStockModel { type_:1, id_:9006, is_create:true,
           expiry:DateTime::parse_from_rfc3339("2024-11-15T09:23:58.098+01:00").unwrap(),
           quantity: StockQuantityModel {total:14, booked:0, cancelled:0}
        },
        // ---------------------
        ProductStockModel { type_:2, id_:9004, is_create:false,
           expiry:DateTime::parse_from_rfc3339("2023-11-09T09:16:01.029-01:00").unwrap(),
           quantity: StockQuantityModel {total:15, booked:0, cancelled:7}
        },
        ProductStockModel { type_:1, id_:9006, is_create:false,
           expiry:DateTime::parse_from_rfc3339("2024-11-15T09:23:58.098+01:00").unwrap(),
           quantity: StockQuantityModel {total:18, booked:0, cancelled:1}
        },
        // ---------------------
        ProductStockModel { type_:2, id_:9004, is_create:false,
           expiry:DateTime::parse_from_rfc3339("2023-11-09T09:16:01.035-01:00").unwrap(),
           quantity: StockQuantityModel {total:22, booked:0, cancelled:8}
        },
        ProductStockModel { type_:2, id_:9004, is_create:true,
           expiry:DateTime::parse_from_rfc3339("2023-11-09T12:30:10.035-01:00").unwrap(),
           quantity: StockQuantityModel {total:20, booked:0, cancelled:0}
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
    let repo = in_mem_repo_ds_setup::<AppInMemoryDStore>(12);
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
            |m2| ProductStockIdentity {store_id:m1.store_id, product_type:m2.type_,
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

fn verify_stocklvl_model(actual:&StockLevelModelSet,
                         expect:&StockLevelModelSet,
                         idx:[usize;2] ,
                         use_eq_op:bool )
{
    let rand_chosen_store = &expect.stores[idx[0]];
    let result = actual.stores.iter().find(|m| {m.store_id == rand_chosen_store.store_id});
    assert!(result.is_some());
    if let Some(actual_st) = result {
        let rand_chosen_product = &rand_chosen_store.products[idx[1]];
        let result = actual_st.products.iter().find(|m| {
            m.type_ == rand_chosen_product.type_ &&  m.id_ == rand_chosen_product.id_
                && m.expiry_without_millis() == rand_chosen_product.expiry_without_millis()
        });
        assert!(result.is_some());
        if let Some(actual_prod) = result {
            if use_eq_op {
                assert_eq!(actual_prod, rand_chosen_product);
            } else {
                assert_ne!(actual_prod, rand_chosen_product);
            }
        }
    }
} // end of verify_stocklvl_model


#[tokio::test]
async fn in_mem_update_existing_ok ()
{
    let repo = in_mem_repo_ds_setup::<AppInMemoryDStore>(8);
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
                product_type: chosen_store.products[2].type_,
                product_id:   chosen_store.products[2].id_,
                expiry:    chosen_store.products[2].expiry  },
            ProductStockIdentity { store_id:chosen_store.store_id,
                product_type: chosen_store.products[4].type_,
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
    let repo = in_mem_repo_ds_setup::<AppInMemoryDStore>(8);
    let stockrepo = repo.stock();
    let all_products = {
        let out = ut_init_data_product();
        assert_eq!((out[2].type_, out[2].id_), (out[7].type_, out[7].id_));
        assert_eq!((out[2].type_, out[2].id_), (out[8].type_, out[8].id_));
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
                product_type: chosen_store.products[0].type_,
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
                product_type: chosen_store.products[0].type_,
                product_id:   chosen_store.products[0].id_,
                expiry:    chosen_store.products[0].expiry  },
            ProductStockIdentity { store_id: chosen_store.store_id,
                product_type: chosen_store.products[1].type_,
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
    let repo = in_mem_repo_ds_setup::<MockInMemDeadDataStore>(4);
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
    let repo = in_mem_repo_ds_setup::<MockInMemDeadDataStore>(4);
    let stockrepo = repo.stock();
    let all_products = ut_init_data_product();
    let pids = {
        let chosen_store = &UT_INIT_DATA_STORE[0];
        vec![
            ProductStockIdentity { store_id: chosen_store.store_id,
                product_type: all_products[0].type_,
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

