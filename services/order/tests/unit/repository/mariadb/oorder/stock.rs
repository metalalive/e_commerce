use std::sync::Arc;

use chrono::DateTime;

use order::constant::ProductType;
use order::repository::{app_repo_order, AbsOrderStockRepo};
use order::model::{
    StoreStockModel, StockLevelModelSet, ProductStockIdentity, ProductStockModel,
    StockQuantityModel
};

use crate::model::verify_stocklvl_model;
use super::super::dstore_ctx_setup;

fn ut_init_data_product() -> [ProductStockModel; 9]
{
    [   // ------ for insertion, do not verify reservation --------
        ProductStockModel { type_:ProductType::Item, id_:9002, is_create:true,
           expiry: DateTime::parse_from_rfc3339("2023-10-05T08:14:05+09:00").unwrap().into(),
           quantity: StockQuantityModel::new(5, 0, 0, None)
        },
        ProductStockModel { type_:ProductType::Package, id_:9003, is_create:true,
           expiry:DateTime::parse_from_rfc3339("2023-11-07T08:12:05.008+02:00").unwrap().into(),
           quantity: StockQuantityModel::new(11, 0, 0, None)
        },
        ProductStockModel { type_:ProductType::Package, id_:9004, is_create:true,
           expiry:DateTime::parse_from_rfc3339("2023-11-09T09:16:01.029-01:00").unwrap().into(),
           quantity: StockQuantityModel::new(15, 0, 0, None)
        },
        ProductStockModel { type_:ProductType::Item, id_:9005, is_create:true,
           expiry:DateTime::parse_from_rfc3339("2024-11-11T09:22:01.005+08:00").unwrap().into(),
           quantity: StockQuantityModel::new(8, 0, 0, None)
        },
        ProductStockModel { type_:ProductType::Item, id_:9006, is_create:true,
           expiry:DateTime::parse_from_rfc3339("2023-11-15T09:23:58.097+01:00").unwrap().into(),
           quantity: StockQuantityModel::new(14, 0, 0, None)
        },
        // -------- for mix of update / insertion -------------
        ProductStockModel { type_:ProductType::Package, id_:9004, is_create:false,
           expiry:DateTime::parse_from_rfc3339("2023-11-09T09:16:01.029-01:00").unwrap().into(),
           quantity: StockQuantityModel::new(15, 7, 0, None)
        },
        ProductStockModel { type_:ProductType::Item, id_:9006, is_create:false,
           expiry:DateTime::parse_from_rfc3339("2023-11-15T09:23:58.097+01:00").unwrap().into(),
           quantity: StockQuantityModel::new(18, 1, 0, None)
        },
        ProductStockModel { type_:ProductType::Item, id_:9007, is_create:true,
           expiry:DateTime::parse_from_rfc3339("2023-09-21T14:36:55.0015+09:00").unwrap().into(),
           quantity: StockQuantityModel::new(145, 0, 0, None)
        },
        ProductStockModel { type_:ProductType::Item, id_:9006, is_create:true,
           expiry:DateTime::parse_from_rfc3339("2023-11-15T04:51:18.0001-09:20").unwrap().into(),
           quantity: StockQuantityModel::new(120, 0, 0, None)
        }, // the same product ID , different expiries
    ]
} // end of ut_init_data_product


const UT_INIT_DATA_STORE: [StoreStockModel; 2] = 
[
    StoreStockModel {store_id:1001, products:vec![]},
    StoreStockModel {store_id:1002, products:vec![]},
];


async fn insert_base_qty_ok(
    stockrepo: Arc<Box<dyn AbsOrderStockRepo>> ,
    all_products: &[ProductStockModel]
)
{
    let expect_slset = {
        let mut stores = UT_INIT_DATA_STORE.to_vec();
        stores[0].products.extend_from_slice(&all_products[0..3]);
        stores[1].products.extend_from_slice(&all_products[3..5]);
        StockLevelModelSet { stores }
    };
    let result = stockrepo.save(expect_slset.clone()).await;
    assert!(result.is_ok());
    let pids = expect_slset.stores.iter().flat_map(|m1| {
        m1.products.iter().map(
            |m2| ProductStockIdentity { store_id:m1.store_id,
                product_id:m2.id_,  product_type:m2.type_.clone(),
                expiry:m2.expiry_without_millis() }
        )
    }).collect();
    let result = stockrepo.fetch(pids).await;
    assert!(result.is_ok());
    if let Ok(actual) = result {
        assert!( ! std::ptr::eq(&actual, &expect_slset) );
        assert_eq!(actual.stores.len(), expect_slset.stores.len());
        verify_stocklvl_model(&actual, &expect_slset, [1,1], true);
        verify_stocklvl_model(&actual, &expect_slset, [0,1], true);
        verify_stocklvl_model(&actual, &expect_slset, [1,0], true);
        verify_stocklvl_model(&actual, &expect_slset, [0,2], true);
        verify_stocklvl_model(&actual, &expect_slset, [0,0], true);
    }
}
async fn update_base_qty_ok(
    stockrepo: Arc<Box<dyn AbsOrderStockRepo>> ,
    all_products: &[ProductStockModel]
)
{
    let expect_slset = {
        let mut stores = UT_INIT_DATA_STORE.to_vec();
        stores[0].products.push(all_products[0].clone());
        stores[1].products.push(all_products[1].clone());
        stores[0].products.push(all_products[2].clone());
        stores[1].products.push(all_products[3].clone());
        StockLevelModelSet { stores }
    };
    let result = stockrepo.save(expect_slset.clone()).await;
    assert!(result.is_ok());
    let pids = expect_slset.stores.iter().flat_map(|m1| {
        m1.products.iter().map(
            |m2| ProductStockIdentity { store_id:m1.store_id, product_id:m2.id_,
                    product_type:m2.type_.clone(), expiry:m2.expiry_without_millis() }
        )
    }).collect();
    let result = stockrepo.fetch(pids).await;
    assert!(result.is_ok());
    if let Ok(actual) = result {
        assert_eq!(actual.stores.len(), 2);
        assert_eq!(actual.stores[0].products.len(), 2);
        verify_stocklvl_model(&actual, &expect_slset, [0,0], true);
        verify_stocklvl_model(&actual, &expect_slset, [1,0], true);
        verify_stocklvl_model(&actual, &expect_slset, [1,1], true);
        verify_stocklvl_model(&actual, &expect_slset, [0,1], true);
    }
} 

#[tokio::test]
async fn save_fetch_ok()
{
    let ds = dstore_ctx_setup();
    let o_repo = app_repo_order(ds).await.unwrap();
    let stockrepo = o_repo.stock();
    let all_products = ut_init_data_product();
    let (data1, data2) = all_products.split_at(5);
    insert_base_qty_ok(stockrepo.clone(), data1).await;
    update_base_qty_ok(stockrepo, data2).await;
} // end of fn save_fetch_ok

