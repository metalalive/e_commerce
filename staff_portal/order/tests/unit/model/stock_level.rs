use chrono::DateTime;

use order::constant::ProductType;
use order::error::AppErrorCode;
use order::model::{StockLevelModelSet, ProductStockModel, StoreStockModel, StockQuantityModel};
use order::api::rpc::dto::{InventoryEditStockLevelDto, StockLevelPresentDto, StockQuantityPresentDto};

use crate::model::verify_stocklvl_model;

fn ut_mock_saved_product() -> [ProductStockModel;5]
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
    ]
}

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

