use std::collections::HashMap;

use chrono::{DateTime, Duration};
use rust_decimal::Decimal;

use ecommerce_common::api::dto::CurrencyDto;
use ecommerce_common::constant::ProductType;
use ecommerce_common::error::AppErrorCode;

use order::api::rpc::dto::{
    InventoryEditStockLevelDto, StockLevelPresentDto, StockLevelReturnDto, StockQuantityPresentDto,
    StockReturnErrorReason,
};
use order::api::web::dto::OrderLineCreateErrorReason;
use order::model::{
    CurrencyModel, OrderCurrencyModel, OrderLineAppliedPolicyModel, OrderLineIdentity,
    OrderLineModel, OrderLineModelSet, OrderLinePriceModel, OrderLineQuantityModel,
    ProductStockModel, StockLevelModelSet, StockQtyRsvModel, StockQuantityModel, StoreStockModel,
};

use crate::model::verify_stocklvl_model;

fn ut_mock_saved_product() -> [ProductStockModel; 11] {
    [
        ProductStockModel {
            type_: ProductType::Item,
            id_: 9002,
            is_create: false,
            expiry: DateTime::parse_from_rfc3339("2023-10-05T08:14:05+09:00")
                .unwrap()
                .into(),
            quantity: StockQuantityModel::new(5, 0, 0, None),
        },
        ProductStockModel {
            type_: ProductType::Package,
            id_: 9003,
            is_create: false,
            expiry: DateTime::parse_from_rfc3339("2023-11-07T08:12:05.008+02:00")
                .unwrap()
                .into(),
            quantity: StockQuantityModel::new(11, 0, 0, None),
        },
        ProductStockModel {
            type_: ProductType::Item,
            id_: 9004,
            is_create: false,
            expiry: DateTime::parse_from_rfc3339("2023-11-09T09:16:01.029-01:00")
                .unwrap()
                .into(),
            quantity: StockQuantityModel::new(15, 0, 0, None),
        },
        ProductStockModel {
            type_: ProductType::Package,
            id_: 9005,
            is_create: false,
            expiry: DateTime::parse_from_rfc3339("2024-11-11T09:22:01.005+08:00")
                .unwrap()
                .into(),
            quantity: StockQuantityModel::new(8, 1, 0, None),
        },
        ProductStockModel {
            type_: ProductType::Item,
            id_: 9006,
            is_create: false,
            expiry: DateTime::parse_from_rfc3339("2024-11-15T09:23:58.098+01:00")
                .unwrap()
                .into(),
            quantity: StockQuantityModel::new(14, 0, 0, None),
        },
        // ------
        // assume some of the product items below are reserved for other orders,
        // the field `booked` is initialized to 1 in quantity model
        ProductStockModel {
            type_: ProductType::Item,
            id_: 9006,
            is_create: false,
            expiry: DateTime::parse_from_rfc3339("2024-11-20T04:50:18.004+01:00")
                .unwrap()
                .into(),
            quantity: StockQuantityModel::new(11, 2, 1, None),
        },
        ProductStockModel {
            type_: ProductType::Item,
            id_: 9006,
            is_create: false,
            expiry: DateTime::parse_from_rfc3339("2024-11-23T05:11:57+01:00")
                .unwrap()
                .into(),
            quantity: StockQuantityModel::new(13, 1, 1, None),
        },
        ProductStockModel {
            type_: ProductType::Item,
            id_: 9002,
            is_create: false,
            expiry: DateTime::parse_from_rfc3339("2023-10-21T18:40:30.040+09:00")
                .unwrap()
                .into(),
            quantity: StockQuantityModel::new(5, 1, 1, None),
        },
        ProductStockModel {
            type_: ProductType::Item,
            id_: 9002,
            is_create: false,
            expiry: DateTime::parse_from_rfc3339("2023-10-07T08:01:00+09:00")
                .unwrap()
                .into(),
            quantity: StockQuantityModel::new(19, 10, 1, None),
        },
        ProductStockModel {
            type_: ProductType::Item,
            id_: 9002,
            is_create: false,
            expiry: DateTime::parse_from_rfc3339("2023-10-18T07:40:33.040+09:00")
                .unwrap()
                .into(),
            quantity: StockQuantityModel::new(6, 1, 1, None),
        },
        ProductStockModel {
            type_: ProductType::Item,
            id_: 9002,
            is_create: false,
            expiry: DateTime::parse_from_rfc3339("2023-10-09T07:58:30.1008+09:00")
                .unwrap()
                .into(),
            quantity: StockQuantityModel::new(10, 1, 1, None),
        },
    ] // end of array
} // end of fn ut_mock_saved_product

#[test]
fn add_update_mix_ok() {
    let saved_products = ut_mock_saved_product();
    let mset = StockLevelModelSet {
        stores: vec![
            StoreStockModel {
                store_id: 1013,
                products: saved_products[0..3].to_vec(),
            },
            StoreStockModel {
                store_id: 1014,
                products: saved_products[3..5].to_vec(),
            },
        ],
    };
    let dt2 = DateTime::from_timestamp(saved_products[3].expiry.timestamp() - 2, 0)
        .unwrap()
        .with_timezone(&saved_products[3].expiry.timezone());
    let newdata = vec![
        InventoryEditStockLevelDto {
            qty_add: 1,
            store_id: 1013,
            product_type: saved_products[0].type_.clone(),
            product_id: saved_products[0].id_,
            expiry: saved_products[0].expiry.fixed_offset(),
        },
        InventoryEditStockLevelDto {
            qty_add: 12,
            store_id: 1013,
            product_type: ProductType::Item,
            expiry: saved_products[0].expiry.fixed_offset(),
            product_id: 5501,
        },
        InventoryEditStockLevelDto {
            qty_add: 19,
            store_id: 1015,
            product_type: ProductType::Package,
            expiry: saved_products[1].expiry.fixed_offset(),
            product_id: 5502,
        },
        // the items below represent the same product with different expiry,
        // in this app, they are considered as separate stock-level model instances
        InventoryEditStockLevelDto {
            qty_add: -2,
            store_id: 1014,
            product_type: saved_products[3].type_.clone(),
            product_id: saved_products[3].id_,
            expiry: saved_products[3].expiry.fixed_offset(),
        },
        InventoryEditStockLevelDto {
            qty_add: 23,
            store_id: 1014,
            product_type: saved_products[3].type_.clone(),
            product_id: saved_products[3].id_,
            expiry: dt2.fixed_offset(),
        },
    ];
    let expect_updated = {
        let mut out = mset.clone();
        out.stores[0].products[0].quantity.total += 1;
        out.stores[1].products[0].quantity.cancelled += 2;
        out.stores[0].products.push(ProductStockModel {
            type_: ProductType::Item,
            id_: 5501,
            expiry: saved_products[0].expiry.clone(),
            is_create: true,
            quantity: StockQuantityModel::new(12, 0, 0, None),
        });
        out.stores[1].products.push(ProductStockModel {
            type_: saved_products[3].type_.clone(),
            id_: saved_products[3].id_,
            expiry: dt2,
            is_create: true,
            quantity: StockQuantityModel::new(23, 0, 0, None),
        });
        out.stores.push(StoreStockModel {
            store_id: 1015,
            products: vec![],
        });
        out.stores[2].products.push(ProductStockModel {
            type_: ProductType::Package,
            id_: 5502,
            expiry: saved_products[1].expiry.clone(),
            is_create: true,
            quantity: StockQuantityModel::new(19, 0, 0, None),
        });
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
                verify_stocklvl_model(&mset, &expect_updated, [idx, jdx], true);
            }
        }
    }
} // end of fn add_update_mix_ok

#[test]
fn update_cancelled_more_than_total() {
    let saved_products = ut_mock_saved_product();
    let mset = StockLevelModelSet {
        stores: vec![StoreStockModel {
            store_id: 1013,
            products: saved_products[4..5].to_vec(),
        }],
    };
    let newdata = vec![InventoryEditStockLevelDto {
        qty_add: -3,
        store_id: 1013,
        product_type: saved_products[4].type_.clone(),
        product_id: saved_products[4].id_,
        expiry: saved_products[4].expiry.fixed_offset(),
    }];
    assert_eq!(mset.stores[0].products[0].quantity.total, 14);
    assert_eq!(mset.stores[0].products[0].quantity.cancelled, 0);
    let result = mset.update(newdata);
    assert!(result.is_ok());
    let mset = result.unwrap();
    let expect = ProductStockModel {
        type_: saved_products[4].type_.clone(),
        id_: saved_products[4].id_,
        is_create: false,
        expiry: saved_products[4].expiry.clone(),
        quantity: StockQuantityModel::new(14, 3, 0, None),
    };
    assert_eq!(mset.stores[0].products[0], expect);
    // ----------------
    let newdata = vec![InventoryEditStockLevelDto {
        qty_add: -13,
        store_id: 1013,
        product_type: saved_products[4].type_.clone(),
        product_id: saved_products[4].id_,
        expiry: saved_products[4].expiry.fixed_offset(),
    }];
    let result = mset.update(newdata);
    assert!(result.is_ok());
    let mset = result.unwrap();
    let expect = ProductStockModel {
        type_: saved_products[4].type_.clone(),
        id_: saved_products[4].id_,
        is_create: false,
        expiry: saved_products[4].expiry.clone(),
        quantity: StockQuantityModel::new(14, 14, 0, None),
    };
    assert_eq!(mset.stores[0].products[0], expect);
} // end of fn update_cancelled_more_than_total

#[test]
fn add_instance_error() {
    let saved_products = ut_mock_saved_product();
    let mset = StockLevelModelSet { stores: vec![] };
    let newdata = vec![InventoryEditStockLevelDto {
        qty_add: -3,
        store_id: 1013,
        product_type: ProductType::Item,
        product_id: 234,
        expiry: saved_products[0].expiry.fixed_offset(),
    }];
    let result = mset.update(newdata);
    assert!(result.is_err());
    if let Err(e) = result {
        assert_eq!(e.code, AppErrorCode::InvalidInput);
        let msg = e.detail.unwrap();
        assert!(msg.contains("negative-initial-quantity"));
    }
}

#[test]
fn present_instance_ok() {
    let saved_products = ut_mock_saved_product();
    let mset = StockLevelModelSet {
        stores: vec![
            StoreStockModel {
                store_id: 1013,
                products: saved_products[0..3].to_vec(),
            },
            StoreStockModel {
                store_id: 1014,
                products: saved_products[3..5].to_vec(),
            },
        ],
    };
    let expect = vec![
        StockLevelPresentDto {
            expiry: DateTime::parse_from_rfc3339("2023-11-07T08:12:05.008+02:00").unwrap(),
            quantity: StockQuantityPresentDto {
                total: 11,
                booked: 0,
                cancelled: 0,
            },
            store_id: 1013,
            product_type: ProductType::Item,
            product_id: 9002,
        },
        StockLevelPresentDto {
            expiry: DateTime::parse_from_rfc3339("2024-11-11T09:22:01.005+08:00").unwrap(),
            quantity: StockQuantityPresentDto {
                total: 8,
                booked: 0,
                cancelled: 1,
            },
            store_id: 1014,
            product_type: ProductType::Package,
            product_id: 9005,
        },
    ];
    let actual: Vec<StockLevelPresentDto> = mset.into();
    assert_eq!(actual.len(), 5);
    for item in expect.iter() {
        let result = actual.iter().find(|d| {
            d.store_id == item.store_id
                && d.product_id == item.product_id
                && d.product_type == item.product_type
        });
        assert!(result.is_some());
    }
} // end of present_instance_ok

fn ut_setup_order_currency(seller_ids: Vec<u32>) -> OrderCurrencyModel {
    let buyer = CurrencyModel {
        name: CurrencyDto::USD,
        rate: Decimal::new(100, 2),
    };
    let seller_c = CurrencyModel {
        name: CurrencyDto::THB,
        rate: Decimal::new(365417, 4),
    };
    let kv_pairs = seller_ids
        .into_iter()
        .map(|seller_id| (seller_id, seller_c.clone()));
    OrderCurrencyModel {
        buyer,
        sellers: HashMap::from_iter(kv_pairs),
    }
}

fn ut_get_curr_qty(store: &StoreStockModel, req: &OrderLineModel) -> Vec<StockQuantityModel> {
    store
        .products
        .iter()
        .filter_map(|p| {
            if req.id_.product_type == p.type_ && req.id_.product_id == p.id_ {
                Some(p.quantity.clone())
            } else {
                None
            }
        })
        .collect()
}

#[test]
fn reserve_ok_1() {
    let create_time = DateTime::parse_from_rfc3339("2022-09-16T14:59:00.091-08:00").unwrap();
    let mock_warranty = DateTime::parse_from_rfc3339("2024-11-28T18:46:08.519-08:00").unwrap();
    let saved_products = ut_mock_saved_product();
    let mut mset = StockLevelModelSet {
        stores: vec![
            StoreStockModel {
                store_id: 1013,
                products: saved_products[0..5].to_vec(),
            },
            StoreStockModel {
                store_id: 1014,
                products: saved_products[5..11].to_vec(),
            },
        ],
    };
    let mut expect_booked_qty = vec![13, 4, 10];
    let reqs = vec![
        OrderLineModel {
            id_: OrderLineIdentity {
                store_id: 1014,
                product_id: saved_products[5].id_,
                product_type: saved_products[5].type_.clone(),
            },
            price: OrderLinePriceModel { unit: 3, total: 35 },
            policy: OrderLineAppliedPolicyModel {
                reserved_until: mock_warranty.clone(),
                warranty_until: mock_warranty.clone(),
            },
            qty: OrderLineQuantityModel {
                reserved: expect_booked_qty[0],
                paid: 0,
                paid_last_update: None,
            },
        },
        OrderLineModel {
            id_: OrderLineIdentity {
                store_id: 1013,
                product_id: saved_products[3].id_,
                product_type: saved_products[3].type_.clone(),
            },
            price: OrderLinePriceModel { unit: 2, total: 8 },
            policy: OrderLineAppliedPolicyModel {
                reserved_until: mock_warranty.clone(),
                warranty_until: mock_warranty.clone(),
            },
            qty: OrderLineQuantityModel {
                reserved: expect_booked_qty[1],
                paid: 0,
                paid_last_update: None,
            },
        },
        OrderLineModel {
            id_: OrderLineIdentity {
                store_id: 1014,
                product_id: saved_products[7].id_,
                product_type: saved_products[7].type_.clone(),
            },
            price: OrderLinePriceModel { unit: 5, total: 48 },
            policy: OrderLineAppliedPolicyModel {
                reserved_until: mock_warranty.clone(),
                warranty_until: mock_warranty.clone(),
            },
            qty: OrderLineQuantityModel {
                reserved: expect_booked_qty[2],
                paid: 0,
                paid_last_update: None,
            },
        },
    ];
    let ol_set = OrderLineModelSet {
        order_id: "AliceOrdered".to_string(),
        lines: reqs,
        create_time: create_time.clone(),
        owner_id: 123,
        currency: ut_setup_order_currency(vec![1013, 1014]),
    };
    let error = mset.try_reserve(&ol_set);
    assert!(error.is_empty());
    [
        ut_get_curr_qty(&mset.stores[1], &ol_set.lines[0]),
        ut_get_curr_qty(&mset.stores[0], &ol_set.lines[1]),
        ut_get_curr_qty(&mset.stores[1], &ol_set.lines[2]),
    ]
    .into_iter()
    .map(|v1| {
        let tot_booked: u32 = v1
            .into_iter()
            .map(|d| {
                if let Some(v) = d.rsv_detail {
                    assert_eq!(v.oid.as_str(), "AliceOrdered");
                    assert!(v.reserved > 0);
                    assert!(d.booked >= v.reserved);
                    v.reserved
                } else {
                    0
                }
            })
            .sum();
        let actual = tot_booked;
        let expect = expect_booked_qty.remove(0);
        assert!(actual > 0);
        assert_eq!(actual, expect);
    })
    .count();
    {
        // verify the order product items were reserved
        let p = mset.stores[1]
            .products
            .iter()
            .collect::<Vec<&ProductStockModel>>();
        assert_eq!(
            (p[0].type_.clone(), p[0].id_),
            (p[1].type_.clone(), p[1].id_)
        );
        assert_eq!(
            (p[0].type_.clone(), p[0].id_),
            (p[2].type_.clone(), p[2].id_)
        );
        assert_eq!(
            (p[0].type_.clone(), p[0].id_),
            (p[3].type_.clone(), p[3].id_)
        );
        assert!(p[0].expiry < p[1].expiry);
        assert!(p[1].expiry < p[2].expiry);
        assert!(p[2].expiry < p[3].expiry);
        assert_eq!(
            p[0].quantity.total,
            (p[0].quantity.booked + p[0].quantity.cancelled)
        );
        assert!(p[1].quantity.total > (p[1].quantity.booked + p[1].quantity.cancelled));
    }
} // end of fn reserve_ok_1

#[test]
fn reserve_ok_2() {
    let create_time = DateTime::parse_from_rfc3339("2022-09-16T14:59:00.091-08:00").unwrap();
    let mock_warranty = DateTime::parse_from_rfc3339("2024-11-28T18:46:08.519-08:00").unwrap();
    let saved_products = ut_mock_saved_product();
    let mut mset = StockLevelModelSet {
        stores: vec![
            StoreStockModel {
                store_id: 1013,
                products: saved_products[0..5].to_vec(),
            },
            StoreStockModel {
                store_id: 1014,
                products: saved_products[5..11].to_vec(),
            },
        ],
    };
    let mut expect_booked_qty = vec![5, 2];
    let reqs = vec![
        OrderLineModel {
            id_: OrderLineIdentity {
                store_id: 1014,
                product_id: saved_products[7].id_,
                product_type: saved_products[7].type_.clone(),
            },
            price: OrderLinePriceModel {
                unit: 10,
                total: 50,
            },
            policy: OrderLineAppliedPolicyModel {
                reserved_until: mock_warranty.clone(),
                warranty_until: mock_warranty.clone(),
            },
            qty: OrderLineQuantityModel {
                reserved: expect_booked_qty[0],
                paid: 0,
                paid_last_update: None,
            },
        },
        OrderLineModel {
            id_: OrderLineIdentity {
                store_id: 1013,
                product_id: saved_products[3].id_,
                product_type: saved_products[3].type_.clone(),
            },
            price: OrderLinePriceModel { unit: 2, total: 8 },
            policy: OrderLineAppliedPolicyModel {
                reserved_until: mock_warranty.clone(),
                warranty_until: mock_warranty.clone(),
            },
            qty: OrderLineQuantityModel {
                reserved: expect_booked_qty[1],
                paid: 0,
                paid_last_update: None,
            },
        },
    ];
    let ol_set = OrderLineModelSet {
        order_id: "BobCart".to_string(),
        lines: reqs,
        create_time,
        owner_id: 321,
        currency: ut_setup_order_currency(vec![1013, 1014]),
    };
    let error = mset.try_reserve(&ol_set);
    assert!(error.is_empty());
    [
        ut_get_curr_qty(&mset.stores[1], &ol_set.lines[0]),
        ut_get_curr_qty(&mset.stores[0], &ol_set.lines[1]),
    ]
    .into_iter()
    .map(|v1| {
        let tot_booked: u32 = v1
            .into_iter()
            .map(|d| {
                if let Some(v) = d.rsv_detail {
                    assert_eq!(v.oid.as_str(), "BobCart");
                    assert!(v.reserved > 0);
                    assert!(d.booked >= v.reserved);
                    v.reserved
                } else {
                    0
                }
            })
            .sum();
        let actual = tot_booked;
        let expect = expect_booked_qty.remove(0);
        assert!(actual > 0);
        assert_eq!(actual, expect);
    })
    .count();
}

#[test]
fn reserve_shortage() {
    let mock_warranty = DateTime::parse_from_rfc3339("2024-11-28T18:46:08.519-08:00").unwrap();
    let saved_products = ut_mock_saved_product();
    let mut mset = StockLevelModelSet {
        stores: vec![
            StoreStockModel {
                store_id: 1013,
                products: saved_products[0..5].to_vec(),
            },
            StoreStockModel {
                store_id: 1014,
                products: saved_products[5..11].to_vec(),
            },
        ],
    };
    {
        // assume this product item has been out of stock
        let qty_ref = &mut mset.stores[0].products[1].quantity;
        qty_ref.reserve("anotherCustomer", qty_ref.total - qty_ref.cancelled);
    }
    let expect_booked_qty = vec![22, 4, 1];
    let reqs = vec![
        OrderLineModel {
            id_: OrderLineIdentity {
                store_id: 1014,
                product_id: saved_products[5].id_,
                product_type: saved_products[5].type_.clone(),
            },
            price: OrderLinePriceModel { unit: 3, total: 66 },
            policy: OrderLineAppliedPolicyModel {
                reserved_until: mock_warranty.clone(),
                warranty_until: mock_warranty.clone(),
            },
            qty: OrderLineQuantityModel {
                reserved: expect_booked_qty[0],
                paid: 0,
                paid_last_update: None,
            },
        },
        OrderLineModel {
            id_: OrderLineIdentity {
                store_id: 1013,
                product_id: saved_products[0].id_,
                product_type: saved_products[0].type_.clone(),
            },
            price: OrderLinePriceModel { unit: 2, total: 8 },
            policy: OrderLineAppliedPolicyModel {
                reserved_until: mock_warranty.clone(),
                warranty_until: mock_warranty.clone(),
            },
            qty: OrderLineQuantityModel {
                reserved: expect_booked_qty[1],
                paid: 0,
                paid_last_update: None,
            },
        },
        OrderLineModel {
            id_: OrderLineIdentity {
                store_id: 1013,
                product_id: saved_products[1].id_,
                product_type: saved_products[1].type_.clone(),
            },
            price: OrderLinePriceModel { unit: 5, total: 5 },
            policy: OrderLineAppliedPolicyModel {
                reserved_until: mock_warranty.clone(),
                warranty_until: mock_warranty.clone(),
            },
            qty: OrderLineQuantityModel {
                reserved: expect_booked_qty[2],
                paid: 0,
                paid_last_update: None,
            },
        },
    ];
    let ol_set = OrderLineModelSet {
        order_id: "xx1".to_string(),
        lines: reqs,
        owner_id: 123,
        create_time: DateTime::parse_from_rfc3339("2022-11-07T04:00:00.519-01:00").unwrap(),
        currency: ut_setup_order_currency(vec![1013, 1014]),
    };
    let error = mset.try_reserve(&ol_set);
    assert_eq!(error.len(), 2);
    {
        let (expect, actual) = (&ol_set.lines[0], &error[0]);
        assert_eq!(expect.id_.store_id, actual.seller_id);
        assert_eq!(expect.id_.product_id, actual.product_id);
        assert_eq!(expect.id_.product_type, actual.product_type);
        assert!(matches!(
            actual.reason,
            OrderLineCreateErrorReason::NotEnoughToClaim
        ));
        let (expect, actual) = (&ol_set.lines[2], &error[1]);
        assert_eq!(expect.id_.store_id, actual.seller_id);
        assert_eq!(expect.id_.product_id, actual.product_id);
        assert_eq!(expect.id_.product_type, actual.product_type);
        assert!(matches!(
            actual.reason,
            OrderLineCreateErrorReason::OutOfStock
        ));
    }
} // end of fn reserve_shortage

#[test]
fn reserve_seller_nonexist() {
    let mock_warranty = DateTime::parse_from_rfc3339("2024-11-28T18:46:08.519-08:00").unwrap();
    let saved_products = ut_mock_saved_product();
    let mut mset = StockLevelModelSet {
        stores: vec![StoreStockModel {
            store_id: 1013,
            products: saved_products[0..5].to_vec(),
        }],
    };
    let expect_booked_qty = vec![2, 2];
    let reqs = vec![
        OrderLineModel {
            id_: OrderLineIdentity {
                store_id: 1013,
                product_id: saved_products[0].id_,
                product_type: saved_products[0].type_.clone(),
            },
            price: OrderLinePriceModel { unit: 2, total: 4 },
            policy: OrderLineAppliedPolicyModel {
                reserved_until: mock_warranty.clone(),
                warranty_until: mock_warranty.clone(),
            },
            qty: OrderLineQuantityModel {
                reserved: expect_booked_qty[1],
                paid: 0,
                paid_last_update: None,
            },
        },
        OrderLineModel {
            id_: OrderLineIdentity {
                store_id: 1099,
                product_id: saved_products[2].id_,
                product_type: saved_products[2].type_.clone(),
            },
            price: OrderLinePriceModel { unit: 3, total: 6 },
            policy: OrderLineAppliedPolicyModel {
                reserved_until: mock_warranty.clone(),
                warranty_until: mock_warranty.clone(),
            },
            qty: OrderLineQuantityModel {
                reserved: expect_booked_qty[0],
                paid: 0,
                paid_last_update: None,
            },
        },
    ];
    let ol_set = OrderLineModelSet {
        order_id: "xx1".to_string(),
        lines: reqs,
        owner_id: 321,
        create_time: DateTime::parse_from_rfc3339("2022-11-07T04:00:00.519-01:00").unwrap(),
        currency: ut_setup_order_currency(vec![1013, 1099]),
    };
    let error = mset.try_reserve(&ol_set);
    assert_eq!(error.len(), 1);
    {
        let (expect, actual) = (&ol_set.lines[1], &error[0]);
        assert_eq!(expect.id_.store_id, actual.seller_id);
        assert_eq!(expect.id_.product_id, actual.product_id);
        assert_eq!(expect.id_.product_type, actual.product_type);
        assert!(matches!(
            actual.reason,
            OrderLineCreateErrorReason::NotExist
        ));
    }
} // end of reserve_seller_nonexist

fn return_across_expiry_rsv_setup(store: &mut StoreStockModel, oid: &str) {
    // assume the products reserved in the giveen store come from the same order
    store
        .products
        .iter_mut()
        .map(|p| {
            assert!(p.quantity.booked > 0);
            let rsv = StockQtyRsvModel {
                oid: oid.to_string(),
                reserved: p.quantity.booked,
            };
            p.quantity.rsv_detail = Some(rsv);
        })
        .count();
}
fn return_across_expiry_estimate_rsv(store: &mut StoreStockModel) -> [u32; 2] {
    let mut out = [0u32, 0];
    store
        .products
        .iter()
        .map(|p| {
            let rsv = p.quantity.rsv_detail.as_ref().unwrap();
            match p.id_ {
                9002 => {
                    out[0] += rsv.reserved;
                }
                9006 => {
                    out[1] += rsv.reserved;
                }
                _others => {}
            }
        })
        .count();
    out
}
#[test]
fn return_across_expiry_ok() {
    let mock_warranty = DateTime::parse_from_rfc3339("2024-11-28T18:46:08.519-08:00").unwrap();
    let saved_products = ut_mock_saved_product();
    let mut mset = StockLevelModelSet {
        stores: vec![
            StoreStockModel {
                store_id: 1013,
                products: saved_products[0..5].to_vec(),
            },
            StoreStockModel {
                store_id: 1014,
                products: saved_products[5..11].to_vec(),
            },
        ],
    };
    return_across_expiry_rsv_setup(&mut mset.stores[1], "ChadBookedThis");
    let rsv_q_before = return_across_expiry_estimate_rsv(&mut mset.stores[1]);
    let data = StockLevelReturnDto {
        order_id: format!("ChadBookedThis"),
        items: vec![
            InventoryEditStockLevelDto {
                store_id: 1014,
                product_type: ProductType::Item,
                product_id: 9002,
                qty_add: 2,
                expiry: mock_warranty,
            },
            InventoryEditStockLevelDto {
                store_id: 1014,
                product_type: ProductType::Item,
                product_id: 9006,
                qty_add: 1,
                expiry: mock_warranty,
            },
        ],
    };
    let error = mset.return_across_expiry(data);
    assert!(error.is_empty());
    let rsv_q_after = return_across_expiry_estimate_rsv(&mut mset.stores[1]);
    assert_eq!((rsv_q_before[0] - rsv_q_after[0]), 2);
    assert_eq!((rsv_q_before[1] - rsv_q_after[1]), 1);
}

#[test]
fn return_across_expiry_nonexist() {
    let mock_warranty = DateTime::parse_from_rfc3339("2024-11-28T18:46:08.519-08:00").unwrap();
    let saved_products = ut_mock_saved_product();
    let mut mset = StockLevelModelSet {
        stores: vec![
            StoreStockModel {
                store_id: 1013,
                products: saved_products[0..5].to_vec(),
            },
            StoreStockModel {
                store_id: 1014,
                products: saved_products[5..11].to_vec(),
            },
        ],
    };
    return_across_expiry_rsv_setup(&mut mset.stores[1], "ChadBookedThis");
    let data = StockLevelReturnDto {
        order_id: format!("ChadBookedThis"),
        items: vec![
            InventoryEditStockLevelDto {
                store_id: 1014,
                product_type: ProductType::Item,
                product_id: 9006,
                qty_add: 1,
                expiry: mock_warranty,
            },
            InventoryEditStockLevelDto {
                store_id: 1014,
                product_type: ProductType::Package,
                product_id: 9999,
                qty_add: 2,
                expiry: mock_warranty,
            },
        ],
    };
    let error = mset.return_across_expiry(data);
    assert_eq!(error.len(), 1);
    assert_eq!(error[0].product_id, 9999);
    assert_eq!(error[0].product_type, ProductType::Package);
    assert!(matches!(error[0].reason, StockReturnErrorReason::NotExist));
}

#[test]
fn return_across_expiry_invalid_qty() {
    let mock_warranty = DateTime::parse_from_rfc3339("2024-11-28T18:46:08.519-08:00").unwrap();
    let saved_products = ut_mock_saved_product();
    let mut mset = StockLevelModelSet {
        stores: vec![StoreStockModel {
            store_id: 1014,
            products: saved_products[5..11].to_vec(),
        }],
    };
    return_across_expiry_rsv_setup(&mut mset.stores[0], "ChadBookedThis");
    let rsv_q_before = return_across_expiry_estimate_rsv(&mut mset.stores[0]);
    let data = StockLevelReturnDto {
        order_id: format!("ChadBookedThis"),
        items: vec![
            InventoryEditStockLevelDto {
                store_id: 1014,
                product_type: ProductType::Item,
                product_id: 9006,
                qty_add: 1,
                expiry: mock_warranty,
            },
            InventoryEditStockLevelDto {
                store_id: 1014,
                product_type: ProductType::Item,
                product_id: 9002,
                qty_add: 3,
                expiry: mock_warranty,
            },
            InventoryEditStockLevelDto {
                store_id: 1014,
                product_type: ProductType::Item,
                product_id: 9006,
                qty_add: 3,
                expiry: mock_warranty,
            },
        ],
    };
    let error = mset.return_across_expiry(data);
    assert_eq!(error.len(), 1);
    assert_eq!(error[0].product_id, 9006);
    assert_eq!(error[0].product_type, ProductType::Item);
    assert!(matches!(
        error[0].reason,
        StockReturnErrorReason::InvalidQuantity
    ));
    let rsv_q_after = return_across_expiry_estimate_rsv(&mut mset.stores[0]);
    assert_eq!((rsv_q_before[0] - rsv_q_after[0]), 3);
    assert_eq!((rsv_q_before[1] - rsv_q_after[1]), 1);
}

fn return_by_expiry_common(mock_oid: &str) -> StockLevelModelSet {
    let saved_products = ut_mock_saved_product();
    let mut mset = StockLevelModelSet {
        stores: vec![
            StoreStockModel {
                store_id: 1013,
                products: saved_products[5..8].to_vec(),
            },
            StoreStockModel {
                store_id: 1014,
                products: saved_products[8..11].to_vec(),
            },
        ],
    };
    {
        // assume more reservations were done within the order
        let num = mset.stores[0].products[1].quantity.reserve(mock_oid, 3);
        assert_eq!(mset.stores[0].products[1].quantity.booked, 4);
        assert_eq!(num, 3);
        let num = mset.stores[1].products[0].quantity.reserve(mock_oid, 2);
        assert_eq!(mset.stores[1].products[0].quantity.booked, 3);
        assert_eq!(num, 2);
        // ----------
        let rsv = mset.stores[0].products[1]
            .quantity
            .rsv_detail
            .as_ref()
            .unwrap();
        assert_eq!(rsv.oid.as_str(), mock_oid);
        assert_eq!(rsv.reserved, 3);
        let rsv = mset.stores[1].products[0]
            .quantity
            .rsv_detail
            .as_ref()
            .unwrap();
        assert_eq!(rsv.oid.as_str(), mock_oid);
        assert_eq!(rsv.reserved, 2);
    }
    mset
}

#[test]
fn return_by_expiry_ok() {
    let mock_oid = "ChadBookedThis";
    let mut mset = return_by_expiry_common(mock_oid);
    let data = StockLevelReturnDto {
        order_id: mock_oid.to_string(),
        items: vec![
            InventoryEditStockLevelDto {
                store_id: 1014,
                product_type: ProductType::Item,
                product_id: 9002,
                qty_add: 2,
                expiry: mset.stores[1].products[0].expiry.fixed_offset()
                    + Duration::milliseconds(13),
            },
            InventoryEditStockLevelDto {
                store_id: 1013,
                product_type: ProductType::Item,
                product_id: 9006,
                qty_add: 2,
                expiry: mset.stores[0].products[1].expiry.fixed_offset()
                    + Duration::milliseconds(46),
            },
        ],
    }; // the expiry time has to be exactly the same
    let error = mset.return_by_expiry(data);
    assert!(error.is_empty());
    {
        assert_eq!(mset.stores[0].products[1].quantity.booked, 2);
        let rsv = mset.stores[0].products[1]
            .quantity
            .rsv_detail
            .as_ref()
            .unwrap();
        assert_eq!(rsv.oid.as_str(), mock_oid);
        assert_eq!(rsv.reserved, 1);
        assert_eq!(mset.stores[1].products[0].quantity.booked, 1);
        let rsv = mset.stores[1].products[0]
            .quantity
            .rsv_detail
            .as_ref()
            .unwrap();
        assert_eq!(rsv.oid.as_str(), mock_oid);
        assert_eq!(rsv.reserved, 0);
    }
} // end of fn return_by_expiry_ok

#[test]
fn return_by_expiry_nonexist() {
    let mock_oid = "ChadBookedThis";
    let saved_products = ut_mock_saved_product();
    let mut mset = StockLevelModelSet {
        stores: vec![
            StoreStockModel {
                store_id: 1013,
                products: saved_products[5..8].to_vec(),
            },
            StoreStockModel {
                store_id: 1014,
                products: saved_products[8..11].to_vec(),
            },
        ],
    };
    let data = StockLevelReturnDto {
        order_id: mock_oid.to_string(),
        items: vec![
            InventoryEditStockLevelDto {
                store_id: 1014,
                product_type: ProductType::Item,
                product_id: 9002,
                qty_add: 1,
                expiry: mset.stores[1].products[0].expiry.fixed_offset() + Duration::seconds(3),
            },
            InventoryEditStockLevelDto {
                store_id: 1013,
                product_type: ProductType::Item,
                product_id: 9006,
                qty_add: 1,
                expiry: mset.stores[0].products[1].expiry.fixed_offset() + Duration::seconds(1),
            },
        ],
    };
    let error = mset.return_by_expiry(data);
    assert_eq!(error.len(), 2);
    assert_eq!(error[0].seller_id, 1014);
    assert_eq!(error[0].product_id, 9002);
    assert_eq!(error[1].seller_id, 1013);
    assert_eq!(error[1].product_id, 9006);
    assert!(matches!(error[0].reason, StockReturnErrorReason::NotExist));
    assert!(matches!(error[1].reason, StockReturnErrorReason::NotExist));
}

#[test]
fn return_by_expiry_invalid_qty() {
    let mock_oid = "ChadBookedThis";
    let mut mset = return_by_expiry_common(mock_oid);
    let data = StockLevelReturnDto {
        order_id: mock_oid.to_string(),
        items: vec![
            InventoryEditStockLevelDto {
                store_id: 1014,
                product_type: ProductType::Item,
                product_id: 9002,
                qty_add: 6,
                expiry: mset.stores[1].products[0].expiry.fixed_offset()
                    + Duration::milliseconds(55),
            },
            InventoryEditStockLevelDto {
                store_id: 1013,
                product_type: ProductType::Item,
                product_id: 9006,
                qty_add: 7,
                expiry: mset.stores[0].products[1].expiry.fixed_offset()
                    + Duration::milliseconds(45),
            },
        ],
    };
    let error = mset.return_by_expiry(data);
    assert_eq!(error.len(), 2);
    assert_eq!(error[0].product_id, 9002);
    assert_eq!(error[1].product_id, 9006);
    assert!(matches!(
        error[0].reason,
        StockReturnErrorReason::InvalidQuantity
    ));
    assert!(matches!(
        error[1].reason,
        StockReturnErrorReason::InvalidQuantity
    ));
}
