use std::sync::Arc;

use chrono::{DateTime, Duration, Local};

use order::api::web::dto::{OrderLineCreateErrorDto, OrderLineCreateErrorReason};
use order::constant::ProductType;
use order::repository::{
    app_repo_order, AbsOrderStockRepo, AppStockRepoReserveReturn
};
use order::model::{
    StoreStockModel, StockLevelModelSet, ProductStockIdentity, ProductStockModel,
    StockQuantityModel, StockQtyRsvModel, OrderLineModelSet, OrderLineModel, OrderLineIdentity,
    OrderLineQuantityModel, OrderLineAppliedPolicyModel, OrderLinePriceModel, 
};

use crate::model::verify_stocklvl_model;
use super::super::dstore_ctx_setup;
use super::super::super::in_mem::oorder::stock::{
    ut_reserve_init_setup, ut_retrieve_stocklvl_qty, mock_reserve_usr_cb_1
};
use super::ut_oline_init_setup;
use super::create::ut_verify_fetch_all_olines_ok;

fn ut_init_data_product() -> [ProductStockModel; 12]
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
           expiry:DateTime::parse_from_rfc3339("2023-11-15T09:23:58.097-09:20").unwrap().into(),
           quantity: StockQuantityModel::new(14, 0, 0, None)
        },
        // -------- for mix of update / insertion -------------
        ProductStockModel { type_:ProductType::Package, id_:9004, is_create:false,
           expiry:DateTime::parse_from_rfc3339("2023-11-09T09:16:01.029-01:00").unwrap().into(),
           quantity: StockQuantityModel::new(15, 7, 0, None)
        },
        ProductStockModel { type_:ProductType::Item, id_:9006, is_create:false,
           expiry:DateTime::parse_from_rfc3339("2023-11-15T09:23:58.097-09:20").unwrap().into(),
           quantity: StockQuantityModel::new(18, 1, 0, None)
        },
        ProductStockModel { type_:ProductType::Item, id_:9007, is_create:true,
           expiry:DateTime::parse_from_rfc3339("2023-09-21T14:36:55.0015+09:00").unwrap().into(),
           quantity: StockQuantityModel::new(145, 0, 0, None)
        },
        ProductStockModel { type_:ProductType::Item, id_:9006, is_create:true,
           expiry:DateTime::parse_from_rfc3339("2023-11-15T09:51:18.0001-09:20").unwrap().into(),
           quantity: StockQuantityModel::new(120, 3, 0, None)
        }, // the same product ID , different expiries
        // -------- more insertions for reserve / return -------------
        ProductStockModel { type_:ProductType::Package, id_:9008, is_create:true,
           expiry:DateTime::parse_from_rfc3339("2023-09-11T19:21:52.4015-08:00").unwrap().into(),
           quantity: StockQuantityModel::new(37, 1, 0, None)
        },
        ProductStockModel { type_:ProductType::Package, id_:9008, is_create:true,
           expiry:DateTime::parse_from_rfc3339("2023-09-12T19:17:36.8492-08:00").unwrap().into(),
           quantity: StockQuantityModel::new(49, 0, 0, None)
        },
        ProductStockModel { type_:ProductType::Item, id_:9009, is_create:true,
           expiry:DateTime::parse_from_rfc3339("2023-09-22T18:07:00.2015+05:00").unwrap().into(),
           quantity: StockQuantityModel::new(46, 1, 0, None)
        },
    ]
} // end of ut_init_data_product


const UT_INIT_DATA_STORE: [StoreStockModel; 5] = 
[
    StoreStockModel {store_id:1011, products:vec![]},
    StoreStockModel {store_id:1012, products:vec![]},
    StoreStockModel {store_id:1013, products:vec![]},
    StoreStockModel {store_id:1014, products:vec![]},
    StoreStockModel {store_id:1015, products:vec![]},
];


async fn insert_base_qty_ok(
    stockrepo: Arc<Box<dyn AbsOrderStockRepo>>,
    mut stores: Vec<StoreStockModel>,
    all_products: &[ProductStockModel]
)
{
    let expect_slset = {
        stores[0].products.extend_from_slice(&all_products[0..3]);
        stores[1].products.extend_from_slice(&all_products[3..5]);
        StockLevelModelSet { stores }
    };
    let result = stockrepo.save(expect_slset.clone()).await;
    assert!(result.is_ok());
    let pids = expect_slset.stores.iter().flat_map(|m1| {
        m1.products.iter().map(
            |m2| ProductStockIdentity { store_id:m1.store_id, product_id:m2.id_,
                    product_type:m2.type_.clone(), expiry:m2.expiry }
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
    mut stores: Vec<StoreStockModel>,
    all_products: &[ProductStockModel]
)
{
    let expect_slset = {
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
                    product_type:m2.type_.clone(), expiry:m2.expiry }
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

#[cfg(feature="mariadb")]
#[tokio::test]
async fn save_fetch_ok()
{
    let ds = dstore_ctx_setup();
    let o_repo = app_repo_order(ds).await.unwrap();
    let stockrepo = o_repo.stock();
    let stores = &UT_INIT_DATA_STORE[0..2];
    let all_products = ut_init_data_product();
    let (data1, data2) = all_products.split_at(5);
    let (data2, _) = data2.split_at(4);
    insert_base_qty_ok(stockrepo.clone(), stores.to_vec(), data1).await;
    update_base_qty_ok(stockrepo, stores.to_vec(), data2).await;
} // end of fn save_fetch_ok


fn mock_reserve_usr_cb_0(ms:&mut StockLevelModelSet, req:&OrderLineModelSet)
    -> AppStockRepoReserveReturn
{
    assert_eq!(ms.stores.len(), 1);
    assert_eq!(req.lines.len(), 1);
    let saved_store = &mut ms.stores[0];
    let id_combo = (
        req.lines[0].id_.store_id,
        req.lines[0].id_.product_type.clone(),
        req.lines[0].id_.product_id,
    );
    let product = match id_combo {
        (1013, ProductType::Item, 9006) => 
            saved_store.products.iter_mut().find(
                |p| p.quantity.total == 120 && p.quantity.cancelled == 3
            ).unwrap() ,
        _others => {
            assert_eq!(saved_store.products.len(), 1);
            & mut saved_store.products[0]
        },
    };
    assert!(product.quantity.rsv_detail.is_none());
    product.quantity.booked += req.lines[0].qty.reserved;
    product.quantity.rsv_detail = Some(StockQtyRsvModel { oid: req.order_id.clone(),
             reserved: req.lines[0].qty.reserved } );
    Ok(())
} // end of mock_reserve_usr_cb_0

#[cfg(feature="mariadb")]
#[tokio::test]
async fn try_reserve_ok()
{
    let mock_warranty  = DateTime::parse_from_rfc3339("3015-11-29T15:02:30.005-03:00").unwrap();
    let mock_rsved_end = DateTime::parse_from_rfc3339("3014-11-29T15:40:43.005-03:00").unwrap();
    let ds = dstore_ctx_setup();
    let o_repo = app_repo_order(ds).await.unwrap();
    let stockrepo = o_repo.stock();
    let all_products = ut_init_data_product();
    {
        let mut stores = UT_INIT_DATA_STORE[2..4].to_vec();
        stores[0].products.extend_from_slice(&all_products[2..5]);
        stores[0].products.extend_from_slice(&all_products[7..9]);
        stores[1].products.extend_from_slice(&all_products[9..12]);
        let expect_slset = StockLevelModelSet { stores };
        let result = stockrepo.save(expect_slset.clone()).await;
        assert!(result.is_ok());
    }
    ut_reserve_init_setup(stockrepo.clone(), mock_reserve_usr_cb_0, mock_warranty, 1013,
        ProductType::Package, 9004, 2, "f1726b0c").await;
    ut_reserve_init_setup(stockrepo.clone(), mock_reserve_usr_cb_0, mock_warranty, 1013,
        ProductType::Item, 9006, 17, "f1726b0d").await;
    ut_reserve_init_setup(stockrepo.clone(), mock_reserve_usr_cb_0, mock_warranty, 1013,
        ProductType::Item, 9006, 7, "17a6b0c3").await;
    ut_reserve_init_setup(stockrepo.clone(), mock_reserve_usr_cb_0, mock_warranty, 1013,
        ProductType::Item, 9006, 4, "b0a578").await;
    ut_reserve_init_setup(stockrepo.clone(), mock_reserve_usr_cb_0, mock_warranty, 1014,
        ProductType::Item, 9009, 3, "1101011b00d1").await;
    ut_reserve_init_setup(stockrepo.clone(), mock_reserve_usr_cb_0, mock_warranty, 1014,
        ProductType::Item, 9009, 1, "a5c8e2083f").await;
    {
        assert_eq!(ut_retrieve_stocklvl_qty(stockrepo.clone(), 1013, &all_products[2]).await, (2, 0, 15)) ;
        assert_eq!(ut_retrieve_stocklvl_qty(stockrepo.clone(), 1013, &all_products[4]).await, (0, 0, 14));
        assert_eq!(ut_retrieve_stocklvl_qty(stockrepo.clone(), 1013, &all_products[8]).await, (17+7+4, 3, 120));
        assert_eq!(ut_retrieve_stocklvl_qty(stockrepo.clone(), 1014, &all_products[9]).await, (0, 1, 37));
        assert_eq!(ut_retrieve_stocklvl_qty(stockrepo.clone(), 1014, &all_products[11]).await, ((3+1), 1, 46));
    }
    let ol_set = {
        let create_time = DateTime::parse_from_rfc3339("2022-11-29T07:29:01.027-03:00").unwrap();
        let lines = vec![
            (1013, ProductType::Package, 9004, 2,  3, mock_warranty + Duration::minutes(1)),
            (1013, ProductType::Item,    9006, 3,  4, mock_rsved_end + Duration::minutes(2)),
            (1014, ProductType::Package, 9008, 29, 20, mock_warranty + Duration::minutes(3)),
            (1014, ProductType::Item,    9009, 6,  15, mock_rsved_end + Duration::minutes(4)),
        ];
        ut_oline_init_setup("800eff40", 123, create_time, lines)
    };
    let result = stockrepo.try_reserve(mock_reserve_usr_cb_1, &ol_set).await;
    assert!(result.is_ok());
    {
        assert_eq!(ut_retrieve_stocklvl_qty(stockrepo.clone(), 1013, &all_products[2]).await, ((2+2), 0, 15)) ;
        assert_eq!(ut_retrieve_stocklvl_qty(stockrepo.clone(), 1013, &all_products[4]).await, (3, 0, 14));
        assert_eq!(ut_retrieve_stocklvl_qty(stockrepo.clone(), 1013, &all_products[8]).await, (17+7+4, 3, 120));
        assert_eq!(ut_retrieve_stocklvl_qty(stockrepo.clone(), 1014, &all_products[9]).await, (29, 1, 37));
        assert_eq!(ut_retrieve_stocklvl_qty(stockrepo.clone(), 1014, &all_products[11]).await, ((3+1+6), 1, 46));
    }
     // verify order lines and top-level metadata
    ut_verify_fetch_all_olines_ok(&o_repo).await;
} // end of fn try_reserve_ok


fn mock_reserve_usr_cb_2(ms:&mut StockLevelModelSet, req:&OrderLineModelSet)
    -> AppStockRepoReserveReturn
{
    assert_eq!(req.lines.len(), 2);
    assert_eq!(ms.stores.len(), 1);
    assert_eq!(ms.stores[0].products.len(), 2);
    ms.stores[0].products.iter().map(|p| {
        let qty = &p.quantity;
        let actual_remain = qty.num_avail();
        let expect_remain = match p.id_ {9002 => 5, 9003 => 10, _others => 0};
        assert_eq!(expect_remain, actual_remain);
    }).count();
    let errors = vec![
        OrderLineCreateErrorDto {
            seller_id: req.lines[0].id_.store_id, product_id: req.lines[0].id_.product_id,
            product_type: req.lines[0].id_.product_type.clone(), nonexist: None,
            shortage: Some(2), reason: OrderLineCreateErrorReason::NotEnoughToClaim
        },
        OrderLineCreateErrorDto {
            seller_id: req.lines[1].id_.store_id, product_id: req.lines[1].id_.product_id,
            product_type: req.lines[1].id_.product_type.clone(), nonexist: None,
            shortage: Some(1), reason: OrderLineCreateErrorReason::OutOfStock
        },
    ];
    Err(Ok(errors))
}

#[cfg(feature="mariadb")]
#[tokio::test]
async fn try_reserve_shortage()
{
    let mock_warranty  = Local::now().fixed_offset() + Duration::minutes(3);
    let ds = dstore_ctx_setup();
    let o_repo = app_repo_order(ds).await.unwrap();
    let stockrepo = o_repo.stock();
    let all_products = ut_init_data_product();
    {
        let mut stores = UT_INIT_DATA_STORE[4..5].to_vec();
        stores[0].products.extend_from_slice(&all_products[0..2]);
        let expect_slset = StockLevelModelSet { stores };
        let result = stockrepo.save(expect_slset).await;
        assert!(result.is_ok());
    }
    ut_reserve_init_setup(stockrepo.clone(), mock_reserve_usr_cb_0, mock_warranty, 1015,
        ProductType::Package, 9003, 1, "f1726b0e").await;
    let ol_set = {
        let create_time = DateTime::parse_from_rfc3339("2022-11-29T06:35:00.519-02:00").unwrap();
        let lines = vec![
            (1015, ProductType::Package, 9003, 12,  3, mock_warranty.clone()),
            (1015, ProductType::Item,    9002, 6,  4, mock_warranty.clone()),
        ];
        ut_oline_init_setup("8100ffe0", 123, create_time, lines)
    };
    let result = stockrepo.try_reserve(mock_reserve_usr_cb_2, &ol_set).await;
    assert!(result.is_err());
    let error = result.unwrap_err();
    let mut input_errs = error.unwrap();
    assert_eq!(input_errs.len(), 2);
    {
        let detail = input_errs.remove(0);
        assert_eq!(detail.product_id, 9003);
        assert_eq!(detail.shortage.unwrap(), 2);
        assert!(matches!(detail.reason, OrderLineCreateErrorReason::NotEnoughToClaim));
        let detail = input_errs.remove(0);
        assert_eq!(detail.product_id, 9002);
        assert_eq!(detail.shortage.unwrap(), 1);
        assert!(matches!(detail.reason, OrderLineCreateErrorReason::OutOfStock));
    }
} // end of  fn try_reserve_shortage
