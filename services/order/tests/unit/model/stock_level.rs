use chrono::{DateTime, Duration};

use order::api::web::dto::OrderLineCreateErrorReason;
use order::constant::ProductType;
use order::error::AppErrorCode;
use order::model::{
    StockLevelModelSet, ProductStockModel, StoreStockModel, StockQuantityModel,
    OrderLineModel, OrderLinePriceModel, OrderLineAppliedPolicyModel, OrderLineQuantityModel, OrderLineModelSet, OrderLineIdentity
};
use order::api::rpc::dto::{
    InventoryEditStockLevelDto, StockLevelPresentDto, StockQuantityPresentDto, StockLevelReturnDto,
    StockReturnErrorReason
};

use crate::model::verify_stocklvl_model;

fn ut_mock_saved_product() -> [ProductStockModel;11]
{
    let mock_rsv_detail = vec![("ChadBookedThis", 1u32)];
    [
        ProductStockModel { type_:ProductType::Item, id_:9002, is_create:false,
           expiry:DateTime::parse_from_rfc3339("2023-10-05T08:14:05+09:00").unwrap().into() ,
           quantity: StockQuantityModel::new(5, 0, None)
        },
        ProductStockModel { type_:ProductType::Package, id_:9003, is_create:false,
           expiry:DateTime::parse_from_rfc3339("2023-11-07T08:12:05.008+02:00").unwrap().into(),
           quantity: StockQuantityModel::new(11, 0, None)
        },
        ProductStockModel { type_:ProductType::Item, id_:9004, is_create:false,
           expiry:DateTime::parse_from_rfc3339("2023-11-09T09:16:01.029-01:00").unwrap().into(),
           quantity: StockQuantityModel::new(15, 0, None)
        },
        ProductStockModel { type_:ProductType::Package, id_:9005, is_create:false,
           expiry:DateTime::parse_from_rfc3339("2024-11-11T09:22:01.005+08:00").unwrap().into(),
           quantity: StockQuantityModel::new(8, 1, None)
        },
        ProductStockModel { type_:ProductType::Item, id_:9006, is_create:false,
           expiry:DateTime::parse_from_rfc3339("2024-11-15T09:23:58.098+01:00").unwrap().into(),
           quantity: StockQuantityModel::new(14, 0, None)
        },
        //--------
        ProductStockModel { type_:ProductType::Item, id_:9006, is_create:false,
           expiry:DateTime::parse_from_rfc3339("2024-11-20T04:50:18.004+01:00").unwrap().into(),
           quantity: StockQuantityModel::new(11, 2, Some(mock_rsv_detail.clone()))
        },
        ProductStockModel { type_:ProductType::Item, id_:9006, is_create:false,
           expiry:DateTime::parse_from_rfc3339("2024-11-23T05:11:57+01:00").unwrap().into(),
           quantity: StockQuantityModel::new(13, 1, Some(mock_rsv_detail.clone()))
        },
        ProductStockModel { type_:ProductType::Item, id_:9002, is_create:false,
           expiry:DateTime::parse_from_rfc3339("2023-10-21T18:40:30.040+09:00").unwrap().into(),
           quantity: StockQuantityModel::new(5, 1, Some(mock_rsv_detail.clone()))
        },
        ProductStockModel { type_:ProductType::Item, id_:9002, is_create:false,
           expiry:DateTime::parse_from_rfc3339("2023-10-07T08:01:00+09:00").unwrap().into(),
           quantity: StockQuantityModel::new(19, 10, Some(mock_rsv_detail.clone()))
        },
        ProductStockModel { type_:ProductType::Item, id_:9002, is_create:false,
           expiry:DateTime::parse_from_rfc3339("2023-10-18T07:40:33.040+09:00").unwrap().into(),
           quantity: StockQuantityModel::new(6, 1, Some(mock_rsv_detail.clone()))
        },
        ProductStockModel { type_:ProductType::Item, id_:9002, is_create:false,
           expiry:DateTime::parse_from_rfc3339("2023-10-09T07:58:30.1008+09:00").unwrap().into(),
           quantity: StockQuantityModel::new(10, 1, Some(mock_rsv_detail.clone()))
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
            expiry: saved_products[0].expiry.fixed_offset()  },
        InventoryEditStockLevelDto {qty_add: 12, store_id: 1013, product_type: ProductType::Item,
            expiry: saved_products[0].expiry.fixed_offset(), product_id: 5501  },
        InventoryEditStockLevelDto {qty_add: 19, store_id: 1015, product_type: ProductType::Package,
            expiry: saved_products[1].expiry.fixed_offset(), product_id: 5502  },
        // the items below represent the same product with different expiry,
        // in this app, they are considered as separate stock-level model instances
        InventoryEditStockLevelDto {qty_add: -2, store_id: 1014,
            product_type: saved_products[3].type_.clone(), product_id: saved_products[3].id_,
            expiry: saved_products[3].expiry.fixed_offset()  },
        InventoryEditStockLevelDto {qty_add: 23, store_id: 1014,
            product_type: saved_products[3].type_.clone(), product_id: saved_products[3].id_,
            expiry: dt2.fixed_offset() },
    ];
    let expect_updated = {
        let mut out = mset.clone();
        out.stores[0].products[0].quantity.total += 1;
        out.stores[1].products[0].quantity.cancelled += 2;
        out.stores[0].products.push(ProductStockModel { type_:ProductType::Item, id_:5501,
            expiry: saved_products[0].expiry.clone(), is_create: true,
            quantity: StockQuantityModel::new(12, 0, None)  });
        out.stores[1].products.push(ProductStockModel { type_:saved_products[3].type_.clone(),
            id_:saved_products[3].id_, expiry: dt2, is_create: true,
            quantity: StockQuantityModel::new(23, 0, None)  });
        out.stores.push(StoreStockModel {store_id: 1015, products:vec![]});
        out.stores[2].products.push(ProductStockModel { type_:ProductType::Package, id_:5502,
            expiry: saved_products[1].expiry.clone(), is_create: true,
            quantity: StockQuantityModel::new(19, 0, None)  });
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
            expiry: saved_products[4].expiry.fixed_offset()  },
    ];
    assert_eq!(mset.stores[0].products[0].quantity.total, 14);
    assert_eq!(mset.stores[0].products[0].quantity.cancelled, 0);
    let result = mset.update(newdata);
    assert!(result.is_ok());
    let mset = result.unwrap();
    let expect = ProductStockModel { type_:saved_products[4].type_.clone(), id_:saved_products[4].id_,
        is_create: false, expiry: saved_products[4].expiry.clone(),
        quantity: StockQuantityModel::new(14, 3, None)
    };
    assert_eq!(mset.stores[0].products[0], expect);
    // ----------------
    let newdata = vec![
        InventoryEditStockLevelDto {qty_add: -13, store_id: 1013,
            product_type: saved_products[4].type_.clone(), product_id: saved_products[4].id_,
            expiry: saved_products[4].expiry.fixed_offset()  },
    ];
    let result = mset.update(newdata);
    assert!(result.is_ok());
    let mset = result.unwrap();
    let expect = ProductStockModel { type_:saved_products[4].type_.clone(), id_:saved_products[4].id_,
        is_create: false, expiry: saved_products[4].expiry.clone(),
        quantity: StockQuantityModel::new(14, 14, None)
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
            product_id: 234, expiry: saved_products[0].expiry.fixed_offset() },
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
        if req.id_.product_type == p.type_ && req.id_.product_id == p.id_ {
            Some(p.quantity.clone())
        } else { None }
    }).collect()
}

#[test]
fn reserve_ok()
{
    let create_time   = DateTime::parse_from_rfc3339("2022-09-16T14:59:00.091-08:00").unwrap();
    let mock_warranty  = DateTime::parse_from_rfc3339("2024-11-28T18:46:08.519-08:00").unwrap();
    let saved_products = ut_mock_saved_product();
    let mut mset = StockLevelModelSet{ stores: vec![
        StoreStockModel {store_id:1013, products: saved_products[0..5].to_vec()},
        StoreStockModel {store_id:1014, products: saved_products[5..11].to_vec()},
    ]};
    // ------ subcase 1 --------
    let mut expect_booked_qty = vec![13,4,10];
    let reqs = vec![
        OrderLineModel { id_:OrderLineIdentity{ store_id:1014, product_id:saved_products[5].id_,
            product_type:saved_products[5].type_.clone()},  price:OrderLinePriceModel {unit:3, total:35},
            policy:OrderLineAppliedPolicyModel { reserved_until: mock_warranty.clone(),
                warranty_until: mock_warranty.clone() },
            qty: OrderLineQuantityModel {reserved: expect_booked_qty[0], paid:0, paid_last_update: None}
        },
        OrderLineModel { id_:OrderLineIdentity{ store_id:1013, product_id:saved_products[3].id_,
            product_type:saved_products[3].type_.clone()}, price:OrderLinePriceModel {unit:2, total:8},
            policy:OrderLineAppliedPolicyModel { reserved_until: mock_warranty.clone(),
                warranty_until: mock_warranty.clone() },
            qty: OrderLineQuantityModel {reserved: expect_booked_qty[1], paid: 0, paid_last_update: None}
        },
        OrderLineModel { id_:OrderLineIdentity{ store_id:1014, product_id:saved_products[7].id_,
            product_type:saved_products[7].type_.clone()},  price:OrderLinePriceModel {unit:5, total:48},
            policy:OrderLineAppliedPolicyModel { reserved_until: mock_warranty.clone(),
                warranty_until: mock_warranty.clone() },
            qty: OrderLineQuantityModel {reserved: expect_booked_qty[2], paid: 0, paid_last_update: None}
        },
    ];
    let ol_set = OrderLineModelSet {order_id:"AliceOrdered".to_string(), lines:reqs,
                 create_time:create_time.clone(), owner_id:123 };
    let error = mset.try_reserve(&ol_set);
    assert!(error.is_empty());
    [
        ut_get_curr_qty(&mset.stores[1], &ol_set.lines[0]),
        ut_get_curr_qty(&mset.stores[0], &ol_set.lines[1]),
        ut_get_curr_qty(&mset.stores[1], &ol_set.lines[2]),
    ].into_iter().map(|v1| {
        let tot_booked:u32 = v1.into_iter().map(|d| {
            if let Some(v) = d.reservation().get("AliceOrdered") {
                assert!(v > &0);
                v.clone()
            } else { 0 }
        }).sum();
        let actual = tot_booked;
        let expect = expect_booked_qty.remove(0);
        assert!(actual > 0);
        assert_eq!(actual, expect);
    }).count();
    { // verify the order product items were reserved
        let p = mset.stores[1].products.iter().collect::<Vec<&ProductStockModel>>();
        assert_eq!((p[0].type_.clone(), p[0].id_), (p[1].type_.clone(), p[1].id_));
        assert_eq!((p[0].type_.clone(), p[0].id_), (p[2].type_.clone(), p[2].id_));
        assert_eq!((p[0].type_.clone(), p[0].id_), (p[3].type_.clone(), p[3].id_));
        assert!(p[0].expiry < p[1].expiry);
        assert!(p[1].expiry < p[2].expiry);
        assert!(p[2].expiry < p[3].expiry);
        assert_eq!(p[0].quantity.total, (p[0].quantity.num_booked() + p[0].quantity.cancelled));
        assert!(p[1].quantity.total > (p[1].quantity.num_booked() + p[1].quantity.cancelled));
    }
    // ------ subcase 2 -------
    expect_booked_qty = vec![5,2];
    let reqs = vec![
        OrderLineModel {id_: OrderLineIdentity{ store_id:1014, product_id:saved_products[7].id_,
            product_type:saved_products[7].type_.clone()}, price:OrderLinePriceModel {unit:10, total:50},
            policy:OrderLineAppliedPolicyModel { reserved_until: mock_warranty.clone(),
                warranty_until: mock_warranty.clone() },
            qty: OrderLineQuantityModel {reserved: expect_booked_qty[0], paid: 0, paid_last_update: None}
        },
        OrderLineModel {id_: OrderLineIdentity{ store_id:1013, product_id:saved_products[3].id_,
            product_type:saved_products[3].type_.clone()}, price:OrderLinePriceModel {unit:2, total:8},
            policy:OrderLineAppliedPolicyModel { reserved_until: mock_warranty.clone(),
                warranty_until: mock_warranty.clone() },
            qty: OrderLineQuantityModel {reserved: expect_booked_qty[1], paid: 0, paid_last_update: None}
        },
    ];
    let ol_set = OrderLineModelSet {order_id:"BobCart".to_string(), lines:reqs,
                    create_time, owner_id:321 } ;
    let error = mset.try_reserve(&ol_set);
    assert!(error.is_empty());
    [
        ut_get_curr_qty(&mset.stores[1], &ol_set.lines[0]),
        ut_get_curr_qty(&mset.stores[0], &ol_set.lines[1]),
    ].into_iter().map(|v1| {
        let tot_booked:u32 = v1.into_iter().map(|d| {
            if let Some(v) = d.reservation().get("BobCart") {
                v.clone()
            } else { 0 }
        }).sum();
        let actual = tot_booked;
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
        qty_ref.reserve("anotherCustomer", qty_ref.total - qty_ref.cancelled);
    }
    let expect_booked_qty = vec![22,4,1];
    let reqs = vec![
        OrderLineModel {id_:OrderLineIdentity{ store_id:1014, product_id:saved_products[5].id_,
            product_type:saved_products[5].type_.clone()}, price:OrderLinePriceModel {unit:3, total:66},
            policy:OrderLineAppliedPolicyModel { reserved_until: mock_warranty.clone(),
                warranty_until: mock_warranty.clone() },
            qty:OrderLineQuantityModel {reserved: expect_booked_qty[0], paid: 0, paid_last_update: None}
        },
        OrderLineModel {id_:OrderLineIdentity{ store_id:1013, product_id:saved_products[0].id_,
            product_type:saved_products[0].type_.clone()}, price:OrderLinePriceModel {unit:2, total:8},
            policy:OrderLineAppliedPolicyModel { reserved_until: mock_warranty.clone(),
                warranty_until: mock_warranty.clone() },
            qty:OrderLineQuantityModel {reserved: expect_booked_qty[1], paid: 0, paid_last_update: None}
        },
        OrderLineModel {id_:OrderLineIdentity{ store_id:1013, product_id:saved_products[1].id_,
            product_type:saved_products[1].type_.clone()}, price:OrderLinePriceModel {unit:5, total:5},
            policy:OrderLineAppliedPolicyModel { reserved_until: mock_warranty.clone(),
                warranty_until: mock_warranty.clone() },
            qty:OrderLineQuantityModel {reserved: expect_booked_qty[2], paid: 0, paid_last_update: None}
        },
    ];
    let ol_set = OrderLineModelSet {order_id:"xx1".to_string(), lines:reqs, owner_id:123,
            create_time: DateTime::parse_from_rfc3339("2022-11-07T04:00:00.519-01:00").unwrap()
    };
    let error = mset.try_reserve(&ol_set);
    assert_eq!(error.len(), 2);
    {
        let (expect, actual) = (&ol_set.lines[0], &error[0]);
        assert_eq!(expect.id_.store_id, actual.seller_id);
        assert_eq!(expect.id_.product_id, actual.product_id);
        assert_eq!(expect.id_.product_type, actual.product_type);
        assert!(matches!(actual.reason, OrderLineCreateErrorReason::NotEnoughToClaim));
        let (expect, actual) = (&ol_set.lines[2], &error[1]);
        assert_eq!(expect.id_.store_id, actual.seller_id);
        assert_eq!(expect.id_.product_id, actual.product_id);
        assert_eq!(expect.id_.product_type, actual.product_type);
        assert!(matches!(actual.reason, OrderLineCreateErrorReason::OutOfStock));
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
        OrderLineModel {id_: OrderLineIdentity {store_id:1013, product_id:saved_products[0].id_,
            product_type:saved_products[0].type_.clone()}, price:OrderLinePriceModel {unit:2, total:4},
            policy:OrderLineAppliedPolicyModel { reserved_until: mock_warranty.clone(),
                warranty_until: mock_warranty.clone() },
            qty:OrderLineQuantityModel {reserved: expect_booked_qty[1], paid: 0, paid_last_update: None}
        },
        OrderLineModel {id_: OrderLineIdentity {store_id:1099, product_id:saved_products[2].id_,
            product_type:saved_products[2].type_.clone()}, price:OrderLinePriceModel {unit:3, total:6},
            policy:OrderLineAppliedPolicyModel { reserved_until: mock_warranty.clone(),
                warranty_until: mock_warranty.clone() },
            qty:OrderLineQuantityModel {reserved: expect_booked_qty[0], paid: 0, paid_last_update: None}
        },
    ];
    let ol_set = OrderLineModelSet {order_id:"xx1".to_string(), lines:reqs, owner_id:321,
            create_time: DateTime::parse_from_rfc3339("2022-11-07T04:00:00.519-01:00").unwrap()
    } ;
    let error = mset.try_reserve(&ol_set);
    assert_eq!(error.len(), 1);
    {
        let (expect, actual) = (&ol_set.lines[1], &error[0]);
        assert_eq!(expect.id_.store_id, actual.seller_id);
        assert_eq!(expect.id_.product_id, actual.product_id);
        assert_eq!(expect.id_.product_type, actual.product_type);
        assert!(matches!(actual.reason, OrderLineCreateErrorReason::NotExist));
    }
} // end of reserve_seller_nonexist


#[test]
fn return_across_expiry_ok()
{
    let mock_warranty  = DateTime::parse_from_rfc3339("2024-11-28T18:46:08.519-08:00").unwrap();
    let saved_products = ut_mock_saved_product();
    let mut mset = StockLevelModelSet{ stores: vec![
        StoreStockModel {store_id:1013, products: saved_products[0..5].to_vec()},
        StoreStockModel {store_id:1014, products: saved_products[5..11].to_vec()},
    ]};
    let data = StockLevelReturnDto { order_id: format!("ChadBookedThis"), items: vec![
        InventoryEditStockLevelDto {store_id:1014, product_type:ProductType::Item,
            product_id:9002, qty_add:2 , expiry:mock_warranty},
        InventoryEditStockLevelDto {store_id:1014, product_type:ProductType::Item,
            product_id:9006, qty_add:1 , expiry:mock_warranty},
    ]};
    let error = mset.return_across_expiry(data);
    assert!(error.is_empty());
}

#[test]
fn return_across_expiry_nonexist()
{
    let mock_warranty  = DateTime::parse_from_rfc3339("2024-11-28T18:46:08.519-08:00").unwrap();
    let saved_products = ut_mock_saved_product();
    let mut mset = StockLevelModelSet{ stores: vec![
        StoreStockModel {store_id:1013, products: saved_products[0..5].to_vec()},
        StoreStockModel {store_id:1014, products: saved_products[5..11].to_vec()},
    ]};
    let data = StockLevelReturnDto { order_id: format!("ChadBookedThis"), items: vec![
        InventoryEditStockLevelDto {store_id:1014, product_type:ProductType::Item,
            product_id:9006, qty_add:1 , expiry:mock_warranty},
        InventoryEditStockLevelDto {store_id:1014, product_type:ProductType::Package,
            product_id:9999, qty_add:2 , expiry:mock_warranty},
    ]};
    let error = mset.return_across_expiry(data);
    assert_eq!(error.len(), 1);
    assert_eq!(error[0].product_id, 9999);
    assert_eq!(error[0].product_type, ProductType::Package);
    assert!(matches!(error[0].reason, StockReturnErrorReason::NotExist));
}

#[test]
fn return_across_expiry_invalid_qty()
{
    let mock_warranty  = DateTime::parse_from_rfc3339("2024-11-28T18:46:08.519-08:00").unwrap();
    let saved_products = ut_mock_saved_product();
    let mut mset = StockLevelModelSet{ stores: vec![
        StoreStockModel {store_id:1014, products: saved_products[5..11].to_vec()},
    ]};
    let data = StockLevelReturnDto { order_id: format!("ChadBookedThis"), items: vec![
        InventoryEditStockLevelDto {store_id:1014, product_type:ProductType::Item,
            product_id:9006, qty_add:1 , expiry:mock_warranty},
        InventoryEditStockLevelDto {store_id:1014, product_type:ProductType::Item,
            product_id:9002, qty_add:3 , expiry:mock_warranty},
        InventoryEditStockLevelDto {store_id:1014, product_type:ProductType::Item,
            product_id:9006, qty_add:3 , expiry:mock_warranty},
    ]};
    let error = mset.return_across_expiry(data);
    assert_eq!(error.len(), 1);
    assert_eq!(error[0].product_id, 9006);
    assert_eq!(error[0].product_type, ProductType::Item);
    assert!(matches!(error[0].reason, StockReturnErrorReason::InvalidQuantity));
}

fn return_by_id_common(mock_oid: &str) -> StockLevelModelSet
{
    let saved_products = ut_mock_saved_product();
    let mut mset = StockLevelModelSet {stores: vec![
        StoreStockModel {store_id:1013, products:saved_products[5..8].to_vec()},
        StoreStockModel {store_id:1014, products:saved_products[8..11].to_vec()},
    ]};
    { // assume more reservations were done within the order
        let num = mset.stores[0].products[1].quantity.reserve(mock_oid, 3);
        assert_eq!(num, 3);
        let num = mset.stores[1].products[0].quantity.reserve(mock_oid, 2);
        assert_eq!(num, 2);
        let qty_map = mset.stores[0].products[1].quantity.reservation();
        assert_eq!(qty_map.get(mock_oid).unwrap().clone(), 4);
        let qty_map = mset.stores[1].products[0].quantity.reservation();
        assert_eq!(qty_map.get(mock_oid).unwrap().clone(), 3);
    }
    mset
}

#[test]
fn return_by_id_ok()
{
    let mock_oid = "ChadBookedThis";
    let mut mset = return_by_id_common(mock_oid);
    let data = StockLevelReturnDto {
        order_id:mock_oid.to_string(), items: vec![
        InventoryEditStockLevelDto {store_id:1014, product_type:ProductType::Item,
            product_id:9002, qty_add:2, expiry:mset.stores[1].products[0].expiry.fixed_offset() },
        InventoryEditStockLevelDto {store_id:1013, product_type:ProductType::Item,
            product_id:9006, qty_add:2, expiry:mset.stores[0].products[1].expiry.fixed_offset() },
    ]}; // the expiry time has to be exactly the same
    let error = mset.return_by_id(data);
    assert!(error.is_empty());
    {
        let qty_map = mset.stores[0].products[1].quantity.reservation();
        assert_eq!(qty_map.get(mock_oid).unwrap().clone(), 2);
        let qty_map = mset.stores[1].products[0].quantity.reservation();
        assert_eq!(qty_map.get(mock_oid).unwrap().clone(), 1);
    }
} // end of fn return_by_id_ok


#[test]
fn return_by_id_nonexist()
{
    let mock_oid = "ChadBookedThis";
    let saved_products = ut_mock_saved_product();
    let mut mset = StockLevelModelSet {stores: vec![
        StoreStockModel {store_id:1013, products:saved_products[5..8].to_vec()},
        StoreStockModel {store_id:1014, products:saved_products[8..11].to_vec()},
    ]};
    let data = StockLevelReturnDto {
        order_id:mock_oid.to_string(), items: vec![
        InventoryEditStockLevelDto {
            store_id:1014, product_type:ProductType::Item, product_id:9002, qty_add:1,
            expiry:mset.stores[1].products[0].expiry.fixed_offset() + Duration::milliseconds(43) },
        InventoryEditStockLevelDto {
            store_id:1013, product_type:ProductType::Item, product_id:9006, qty_add:1,
            expiry:mset.stores[0].products[1].expiry.fixed_offset() + Duration::milliseconds(16) },
    ]};
    let error = mset.return_by_id(data);
    assert_eq!(error.len(), 2);
    assert_eq!(error[0].seller_id, 1014);
    assert_eq!(error[0].product_id, 9002);
    assert_eq!(error[1].seller_id, 1013);
    assert_eq!(error[1].product_id, 9006);
    assert!(matches!(error[0].reason, StockReturnErrorReason::NotExist));
    assert!(matches!(error[1].reason, StockReturnErrorReason::NotExist));
}

#[test]
fn return_by_id_invalid_qty()
{
    let mock_oid = "ChadBookedThis";
    let mut mset = return_by_id_common(mock_oid);
    let data = StockLevelReturnDto {
        order_id:mock_oid.to_string(), items: vec![
        InventoryEditStockLevelDto {store_id:1014, product_type:ProductType::Item,
            product_id:9002, qty_add:6, expiry:mset.stores[1].products[0].expiry.fixed_offset() },
        InventoryEditStockLevelDto {store_id:1013, product_type:ProductType::Item,
            product_id:9006, qty_add:7, expiry:mset.stores[0].products[1].expiry.fixed_offset() },
    ]};
    let error = mset.return_by_id(data);
    assert_eq!(error.len(), 2);
    assert_eq!(error[0].product_id, 9002);
    assert_eq!(error[1].product_id, 9006);
    assert!(matches!(error[0].reason, StockReturnErrorReason::InvalidQuantity));
    assert!(matches!(error[1].reason, StockReturnErrorReason::InvalidQuantity));
}

