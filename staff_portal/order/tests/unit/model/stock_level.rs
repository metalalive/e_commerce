use chrono::DateTime;

use order::api::web::dto::OrderLineErrorReason;
use order::constant::ProductType;
use order::error::AppErrorCode;
use order::model::{
    StockLevelModelSet, ProductStockModel, StoreStockModel, StockQuantityModel,
    OrderLineModel, OrderLinePriceModel, OrderLineAppliedPolicyModel
};
use order::api::rpc::dto::{InventoryEditStockLevelDto, StockLevelPresentDto, StockQuantityPresentDto};

use crate::model::verify_stocklvl_model;

fn ut_mock_saved_product() -> [ProductStockModel;11]
{
    [
        ProductStockModel { type_:ProductType::Item, id_:9002, is_create:false,
           expiry:DateTime::parse_from_rfc3339("2023-10-05T08:14:05+09:00").unwrap(),
           quantity: StockQuantityModel {total:5, booked:0, cancelled:0}
        },
        ProductStockModel { type_:ProductType::Package, id_:9003, is_create:false,
           expiry:DateTime::parse_from_rfc3339("2023-11-07T08:12:05.008+02:00").unwrap(),
           quantity: StockQuantityModel {total:11, booked:0, cancelled:0}
        },
        ProductStockModel { type_:ProductType::Item, id_:9004, is_create:false,
           expiry:DateTime::parse_from_rfc3339("2023-11-09T09:16:01.029-01:00").unwrap(),
           quantity: StockQuantityModel {total:15, booked:0, cancelled:0}
        },
        ProductStockModel { type_:ProductType::Package, id_:9005, is_create:false,
           expiry:DateTime::parse_from_rfc3339("2024-11-11T09:22:01.005+08:00").unwrap(),
           quantity: StockQuantityModel {total:8, booked:0, cancelled:1}
        },
        ProductStockModel { type_:ProductType::Item, id_:9006, is_create:false,
           expiry:DateTime::parse_from_rfc3339("2024-11-15T09:23:58.098+01:00").unwrap(),
           quantity: StockQuantityModel {total:14, booked:0, cancelled:0}
        },
        //--------
        ProductStockModel { type_:ProductType::Item, id_:9006, is_create:false,
           expiry:DateTime::parse_from_rfc3339("2024-11-20T04:50:18.004+01:00").unwrap(),
           quantity: StockQuantityModel {total:11, booked:1, cancelled:2}
        },
        ProductStockModel { type_:ProductType::Item, id_:9006, is_create:false,
           expiry:DateTime::parse_from_rfc3339("2024-11-23T05:11:57+01:00").unwrap(),
           quantity: StockQuantityModel {total:13, booked:1, cancelled:1}
        },
        ProductStockModel { type_:ProductType::Item, id_:9002, is_create:false,
           expiry:DateTime::parse_from_rfc3339("2023-10-01T18:40:30.040+09:00").unwrap(),
           quantity: StockQuantityModel {total:5, booked:1, cancelled:1}
        },
        ProductStockModel { type_:ProductType::Item, id_:9002, is_create:false,
           expiry:DateTime::parse_from_rfc3339("2023-10-07T08:01:00+09:00").unwrap(),
           quantity: StockQuantityModel {total:19, booked:1, cancelled:10}
        },
        ProductStockModel { type_:ProductType::Item, id_:9002, is_create:false,
           expiry:DateTime::parse_from_rfc3339("2023-10-08T07:40:33.040+09:00").unwrap(),
           quantity: StockQuantityModel {total:6, booked:1, cancelled:1}
        },
        ProductStockModel { type_:ProductType::Item, id_:9002, is_create:false,
           expiry:DateTime::parse_from_rfc3339("2023-10-09T07:58:30.1008+09:00").unwrap(),
           quantity: StockQuantityModel {total:10, booked:1, cancelled:1}
        },
    ] // end of array
} // end of fn ut_mock_saved_product

#[test]
fn add_update_mix_ok()
{
    let saved_products = ut_mock_saved_product();
    let mset = StockLevelModelSet{ stores: vec![
        StoreStockModel {store_id:1013, products: saved_products[0..3].to_vec()},
        StoreStockModel {store_id:1014, products: saved_products[3..5].to_vec()},
    ]};
    let dt2 = DateTime::from_timestamp(saved_products[3].expiry.timestamp() - 2, 0)
            .unwrap() .with_timezone(&saved_products[3].expiry.timezone());
    let newdata = vec![
        InventoryEditStockLevelDto {qty_add: 1, store_id: 1013,
            product_type: saved_products[0].type_.clone(), product_id: saved_products[0].id_,
            expiry: saved_products[0].expiry.clone()  },
        InventoryEditStockLevelDto {qty_add: 12, store_id: 1013, product_type: ProductType::Item,
            expiry: saved_products[0].expiry.clone(), product_id: 5501  },
        InventoryEditStockLevelDto {qty_add: 19, store_id: 1015, product_type: ProductType::Package,
            expiry: saved_products[1].expiry.clone(), product_id: 5502  },
        // the items below represent the same product with different expiry,
        // in this app, they are considered as separate stock-level model instances
        InventoryEditStockLevelDto {qty_add: -2, store_id: 1014,
            product_type: saved_products[3].type_.clone(), product_id: saved_products[3].id_,
            expiry: saved_products[3].expiry.clone()  },
        InventoryEditStockLevelDto {qty_add: 23, store_id: 1014,
            product_type: saved_products[3].type_.clone(), product_id: saved_products[3].id_,
            expiry: dt2.clone() },
    ];
    let expect_updated = {
        let mut out = mset.clone();
        out.stores[0].products[0].quantity.total += 1;
        out.stores[1].products[0].quantity.cancelled += 2;
        out.stores[0].products.push(ProductStockModel { type_:ProductType::Item, id_:5501,
            expiry: saved_products[0].expiry.clone(), is_create: true,
            quantity: StockQuantityModel{total:12, booked:0, cancelled:0}  });
        out.stores[1].products.push(ProductStockModel { type_:saved_products[3].type_.clone(),
            id_:saved_products[3].id_, expiry: dt2, is_create: true,
            quantity: StockQuantityModel{total:23, booked:0, cancelled:0}  });
        out.stores.push(StoreStockModel {store_id: 1015, products:vec![]});
        out.stores[2].products.push(ProductStockModel { type_:ProductType::Package, id_:5502,
            expiry: saved_products[1].expiry.clone(), is_create: true,
            quantity: StockQuantityModel{total:19, booked:0, cancelled:0}  });
        out
    };
    let result = mset.update(newdata);
    assert!(result.is_ok());
    if let Ok(mset) = result {
        assert_eq!(mset.stores.len(), 3);
        assert_eq!(mset.stores[0].products.len(), 4);
        assert_eq!(mset.stores[1].products.len(), 3);
        assert_eq!(mset.stores[2].products.len(), 1);
        for idx in 0..mset.stores.len() {
            for jdx in 0..mset.stores[idx].products.len() {
                verify_stocklvl_model(&mset, &expect_updated, [idx,jdx], true);
            }
        }
    }
} // end of fn add_update_mix_ok


#[test]
fn update_cancelled_more_than_total()
{
    let saved_products = ut_mock_saved_product();
    let mset = StockLevelModelSet{ stores: vec![
        StoreStockModel {store_id:1013, products: saved_products[4..5].to_vec() },
    ]};
    let newdata = vec![
        InventoryEditStockLevelDto {qty_add: -3, store_id: 1013,
            product_type: saved_products[4].type_.clone(), product_id: saved_products[4].id_,
            expiry: saved_products[4].expiry.clone()  },
    ];
    assert_eq!(mset.stores[0].products[0].quantity.total, 14);
    assert_eq!(mset.stores[0].products[0].quantity.cancelled, 0);
    let result = mset.update(newdata);
    assert!(result.is_ok());
    let mset = result.unwrap();
    let expect = ProductStockModel { type_:saved_products[4].type_.clone(), id_:saved_products[4].id_,
        is_create: false, expiry: saved_products[4].expiry.clone(),
        quantity: StockQuantityModel{total:14, booked:0, cancelled:3}
    };
    assert_eq!(mset.stores[0].products[0], expect);
    // ----------------
    let newdata = vec![
        InventoryEditStockLevelDto {qty_add: -13, store_id: 1013,
            product_type: saved_products[4].type_.clone(), product_id: saved_products[4].id_,
            expiry: saved_products[4].expiry.clone()  },
    ];
    let result = mset.update(newdata);
    assert!(result.is_ok());
    let mset = result.unwrap();
    let expect = ProductStockModel { type_:saved_products[4].type_.clone(), id_:saved_products[4].id_,
        is_create: false, expiry: saved_products[4].expiry.clone(),
        quantity: StockQuantityModel{total:14, booked:0, cancelled:14}
    };
    assert_eq!(mset.stores[0].products[0], expect);
} // end of fn update_cancelled_more_than_total


#[test]
fn add_instance_error()
{
    let saved_products = ut_mock_saved_product();
    let mset = StockLevelModelSet{ stores: vec![]};
    let newdata = vec![
        InventoryEditStockLevelDto {qty_add: -3, store_id: 1013, product_type: ProductType::Item,
            product_id: 234, expiry: saved_products[0].expiry.clone() },
    ];
    let result = mset.update(newdata);
    assert!(result.is_err());
    if let Err(e) = result {
        assert_eq!(e.code, AppErrorCode::InvalidInput);
        let msg = e.detail.unwrap();
        assert!(msg.contains("negative-initial-quantity"));
    }
}


#[test]
fn present_instance_ok()
{
    let saved_products = ut_mock_saved_product();
    let mset = StockLevelModelSet{ stores: vec![
        StoreStockModel {store_id:1013, products: saved_products[0..3].to_vec()},
        StoreStockModel {store_id:1014, products: saved_products[3..5].to_vec()},
    ]};
    let expect = vec![
        StockLevelPresentDto {
            expiry:DateTime::parse_from_rfc3339("2023-11-07T08:12:05.008+02:00").unwrap(),
            quantity: StockQuantityPresentDto  {total:11, booked:0, cancelled:0},
            store_id:1013, product_type: ProductType::Item, product_id: 9002
        },
        StockLevelPresentDto {
            expiry:DateTime::parse_from_rfc3339("2024-11-11T09:22:01.005+08:00").unwrap(),
            quantity: StockQuantityPresentDto {total:8, booked:0, cancelled:1},
            store_id:1014, product_type:ProductType::Package, product_id:9005, 
        },
    ];
    let actual:Vec<StockLevelPresentDto> = mset.into();
    assert_eq!(actual.len(), 5);
    for item in expect.iter() {
        let result = actual.iter().find(|d| {
            d.store_id == item.store_id && d.product_id == item.product_id
                && d.product_type == item.product_type
        });
        assert!(result.is_some());
    }
} // end of present_instance_ok


fn  ut_get_curr_qty (store:&StoreStockModel, req:&OrderLineModel)
    -> Vec<StockQuantityModel>
{
    store.products.iter().filter_map(|p| {
        if req.product_type == p.type_ && req.product_id == p.id_ {
            Some(p.quantity.clone())
        } else { None }
    }).collect()
}

#[test]
fn reserve_ok()
{
    let mock_warranty  = DateTime::parse_from_rfc3339("2024-11-28T18:46:08.519-08:00").unwrap();
    let saved_products = ut_mock_saved_product();
    let mut mset = StockLevelModelSet{ stores: vec![
        StoreStockModel {store_id:1013, products: saved_products[0..5].to_vec()},
        StoreStockModel {store_id:1014, products: saved_products[5..11].to_vec()},
    ]};
    // ------ subcase 1 --------
    let mut expect_booked_qty = vec![13,4,10];
    let reqs = vec![
        OrderLineModel {seller_id:1014, product_type:saved_products[5].type_.clone(),
            product_id:saved_products[5].id_, price:OrderLinePriceModel {unit:3, total:35},
            policy:OrderLineAppliedPolicyModel { reserved_until: mock_warranty.clone(),
                warranty_until: mock_warranty.clone() }, qty:expect_booked_qty[0]
        },
        OrderLineModel {seller_id:1013, product_type:saved_products[3].type_.clone(),
            product_id:saved_products[3].id_, price:OrderLinePriceModel {unit:2, total:8},
            policy:OrderLineAppliedPolicyModel { reserved_until: mock_warranty.clone(),
                warranty_until: mock_warranty.clone() }, qty:expect_booked_qty[1]
        },
        OrderLineModel {seller_id:1014, product_type:saved_products[7].type_.clone(),
            product_id:saved_products[7].id_, price:OrderLinePriceModel {unit:5, total:48},
            policy:OrderLineAppliedPolicyModel { reserved_until: mock_warranty.clone(),
                warranty_until: mock_warranty.clone() }, qty:expect_booked_qty[2]
        },
    ];
    let mut qty_stats_before = vec![
        ut_get_curr_qty(&mset.stores[1], &reqs[0]),
        ut_get_curr_qty(&mset.stores[0], &reqs[1]),
        ut_get_curr_qty(&mset.stores[1], &reqs[2]),
    ];
    let error = mset.try_reserve(&reqs);
    assert!(error.is_empty());
    [
        ut_get_curr_qty(&mset.stores[1], &reqs[0]),
        ut_get_curr_qty(&mset.stores[0], &reqs[1]),
        ut_get_curr_qty(&mset.stores[1], &reqs[2]),
    ].into_iter().map(|v1| {
        let v0 = qty_stats_before.remove(0);
        let tot_booked_v0:u32 = v0.into_iter().map(|d| d.booked).sum();
        let tot_booked_v1:u32 = v1.into_iter().map(|d| d.booked).sum();
        let actual = tot_booked_v1 - tot_booked_v0;
        let expect = expect_booked_qty.remove(0);
        assert!(actual > 0);
        assert_eq!(actual, expect);
    }).count();
    // ------ subcase 2 -------
    expect_booked_qty = vec![5,2];
    let reqs = vec![
        OrderLineModel {seller_id:1014, product_type:saved_products[7].type_.clone(),
            product_id:saved_products[7].id_, price:OrderLinePriceModel {unit:10, total:50},
            policy:OrderLineAppliedPolicyModel { reserved_until: mock_warranty.clone(),
                warranty_until: mock_warranty.clone() }, qty:expect_booked_qty[0]
        },
        OrderLineModel {seller_id:1013, product_type:saved_products[3].type_.clone(),
            product_id:saved_products[3].id_, price:OrderLinePriceModel {unit:2, total:8},
            policy:OrderLineAppliedPolicyModel { reserved_until: mock_warranty.clone(),
                warranty_until: mock_warranty.clone() }, qty:expect_booked_qty[1]
        },
    ];
    qty_stats_before = vec![
        ut_get_curr_qty(&mset.stores[1], &reqs[0]),
        ut_get_curr_qty(&mset.stores[0], &reqs[1]),
    ];
    let error = mset.try_reserve(&reqs);
    assert!(error.is_empty());
    [
        ut_get_curr_qty(&mset.stores[1], &reqs[0]),
        ut_get_curr_qty(&mset.stores[0], &reqs[1]),
    ].into_iter().map(|v1| {
        let v0 = qty_stats_before.remove(0);
        let tot_booked_v0:u32 = v0.into_iter().map(|d| d.booked).sum();
        let tot_booked_v1:u32 = v1.into_iter().map(|d| d.booked).sum();
        let actual = tot_booked_v1 - tot_booked_v0;
        let expect = expect_booked_qty.remove(0);
        assert!(actual > 0);
        assert_eq!(actual, expect);
    }).count();
} // end of reserve_ok


#[test]
fn reserve_shortage()
{
    let mock_warranty  = DateTime::parse_from_rfc3339("2024-11-28T18:46:08.519-08:00").unwrap();
    let saved_products = ut_mock_saved_product();
    let mut mset = StockLevelModelSet{ stores: vec![
        StoreStockModel {store_id:1013, products: saved_products[0..5].to_vec()},
        StoreStockModel {store_id:1014, products: saved_products[5..11].to_vec()},
    ]};
    { // assume this product item has been out of stock
        let qty_ref = &mut mset.stores[0].products[1].quantity;
        qty_ref.booked = qty_ref.total - qty_ref.cancelled;
    }
    let expect_booked_qty = vec![22,4,1];
    let reqs = vec![
        OrderLineModel {seller_id:1014, product_type:saved_products[5].type_.clone(),
            product_id:saved_products[5].id_, price:OrderLinePriceModel {unit:3, total:66},
            policy:OrderLineAppliedPolicyModel { reserved_until: mock_warranty.clone(),
                warranty_until: mock_warranty.clone() }, qty:expect_booked_qty[0]
        },
        OrderLineModel {seller_id:1013, product_type:saved_products[0].type_.clone(),
            product_id:saved_products[0].id_, price:OrderLinePriceModel {unit:2, total:8},
            policy:OrderLineAppliedPolicyModel { reserved_until: mock_warranty.clone(),
                warranty_until: mock_warranty.clone() }, qty:expect_booked_qty[1]
        },
        OrderLineModel {seller_id:1013, product_type:saved_products[1].type_.clone(),
            product_id:saved_products[1].id_, price:OrderLinePriceModel {unit:5, total:5},
            policy:OrderLineAppliedPolicyModel { reserved_until: mock_warranty.clone(),
                warranty_until: mock_warranty.clone() }, qty:expect_booked_qty[2]
        },
    ];
    let error = mset.try_reserve(&reqs);
    assert_eq!(error.len(), 2);
    {
        let (expect, actual) = (&reqs[0], &error[0]);
        assert_eq!(expect.seller_id, actual.seller_id);
        assert_eq!(expect.product_id, actual.product_id);
        assert_eq!(expect.product_type, actual.product_type);
        assert!(matches!(actual.reason, OrderLineErrorReason::NotEnoughToClaim));
        let (expect, actual) = (&reqs[2], &error[1]);
        assert_eq!(expect.seller_id, actual.seller_id);
        assert_eq!(expect.product_id, actual.product_id);
        assert_eq!(expect.product_type, actual.product_type);
        assert!(matches!(actual.reason, OrderLineErrorReason::OutOfStock));
    }
} // end of fn reserve_shortage


#[test]
fn reserve_seller_nonexist()
{
    let mock_warranty  = DateTime::parse_from_rfc3339("2024-11-28T18:46:08.519-08:00").unwrap();
    let saved_products = ut_mock_saved_product();
    let mut mset = StockLevelModelSet{ stores: vec![
        StoreStockModel {store_id:1013, products: saved_products[0..5].to_vec()},
    ]};
    let expect_booked_qty = vec![2,2];
    let reqs = vec![
        OrderLineModel {seller_id:1013, product_type:saved_products[0].type_.clone(),
            product_id:saved_products[0].id_, price:OrderLinePriceModel {unit:2, total:4},
            policy:OrderLineAppliedPolicyModel { reserved_until: mock_warranty.clone(),
                warranty_until: mock_warranty.clone() }, qty:expect_booked_qty[1]
        },
        OrderLineModel {seller_id:1099, product_type:saved_products[2].type_.clone(),
            product_id:saved_products[2].id_, price:OrderLinePriceModel {unit:3, total:6},
            policy:OrderLineAppliedPolicyModel { reserved_until: mock_warranty.clone(),
                warranty_until: mock_warranty.clone() }, qty:expect_booked_qty[0]
        },
    ];
    let error = mset.try_reserve(&reqs);
    assert_eq!(error.len(), 1);
    {
        let (expect, actual) = (&reqs[1], &error[0]);
        assert_eq!(expect.seller_id, actual.seller_id);
        assert_eq!(expect.product_id, actual.product_id);
        assert_eq!(expect.product_type, actual.product_type);
        assert!(matches!(actual.reason, OrderLineErrorReason::NotExist));
    }
} // end of reserve_seller_nonexist

