use std::sync::Arc;

use chrono::{DateTime, Duration, Local};

use order::api::rpc::dto::{InventoryEditStockLevelDto, StockLevelReturnDto, StockReturnErrorDto};
use order::api::web::dto::{OrderLineCreateErrorDto, OrderLineCreateErrorReason};
use order::model::{
    OrderLineModelSet, ProductStockIdentity, ProductStockModel, StockLevelModelSet,
    StockQtyRsvModel, StockQuantityModel, StoreStockModel,
};
use order::repository::{app_repo_order, AbsOrderStockRepo, AppStockRepoReserveReturn};

use super::super::super::in_mem::oorder::stock::{
    mock_reserve_usr_cb_1, ut_reserve_init_setup, ut_retrieve_stocklvl_qty,
};
use super::super::dstore_ctx_setup;
use super::create::ut_verify_fetch_all_olines_ok;
use super::ut_oline_init_setup;
use crate::model::verify_stocklvl_model;

#[rustfmt::skip]
fn ut_init_data_product() -> [ProductStockModel; 12] {
    let rawdata = [
        // ------ for insertion, do not verify reservation --------
        (9002, true, "2038-10-05T08:14:05+09:00", 5, 0, 0),
        (9003, true, "2039-11-07T08:12:05.008+02:00", 11, 0, 0),
        (9004, true, "2039-11-09T09:16:01.029-01:00", 15, 0, 0),
        (9005, true, "2040-11-11T09:22:01.005+08:00", 8, 0, 0),
        (9006, true, "2040-11-15T09:23:58.097-09:20", 14, 0, 0),
        // -------- for mix of update / insertion -------------
        (9004, false, "2039-11-09T09:16:01.029-01:00", 15, 7, 0),
        (9006, false, "2040-11-15T09:23:58.097-09:20", 18, 1, 0),
        (9007, true, "2095-09-21T14:36:55.0015+09:00", 145, 0, 0),
        (9006, true, "2040-11-15T09:51:18.0001-09:20", 120, 3, 0),
        // -------- more insertions for reserve / return -------------
        (9008, true, "2095-09-11T19:21:52.4015-08:00", 37, 1, 0),
        (9008, true, "2095-09-12T19:17:36.8492-08:00", 49, 0, 0),
        (9009, true, "2092-09-22T18:07:00.2015+05:00", 46, 1, 0),
    ];
    rawdata.map(|(id_, is_create, expiry, total, booked, cancelled)| ProductStockModel {
        id_,
        is_create,
        expiry: DateTime::parse_from_rfc3339(expiry).unwrap().into(),
        quantity: StockQuantityModel::new(total, booked, cancelled, None),
    })
} // end of ut_init_data_product

const UT_INIT_DATA_STORE: [StoreStockModel; 5] = [
    StoreStockModel {
        store_id: 1011,
        products: vec![],
    },
    StoreStockModel {
        store_id: 1012,
        products: vec![],
    },
    StoreStockModel {
        store_id: 1013,
        products: vec![],
    },
    StoreStockModel {
        store_id: 1014,
        products: vec![],
    },
    StoreStockModel {
        store_id: 1015,
        products: vec![],
    },
];

async fn insert_base_qty_ok(
    stockrepo: Arc<Box<dyn AbsOrderStockRepo>>,
    mut stores: Vec<StoreStockModel>,
    all_products: &[ProductStockModel],
) {
    let expect_slset = {
        stores[0].products.extend_from_slice(&all_products[0..3]);
        stores[1].products.extend_from_slice(&all_products[3..5]);
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
                expiry: m2.expiry,
            })
        })
        .collect();
    let result = stockrepo.fetch(pids).await;
    assert!(result.is_ok());
    if let Ok(actual) = result {
        assert!(!std::ptr::eq(&actual, &expect_slset));
        assert_eq!(actual.stores.len(), expect_slset.stores.len());
        verify_stocklvl_model(&actual, &expect_slset, [1, 1], true);
        verify_stocklvl_model(&actual, &expect_slset, [0, 1], true);
        verify_stocklvl_model(&actual, &expect_slset, [1, 0], true);
        verify_stocklvl_model(&actual, &expect_slset, [0, 2], true);
        verify_stocklvl_model(&actual, &expect_slset, [0, 0], true);
    }
}
async fn update_base_qty_ok(
    stockrepo: Arc<Box<dyn AbsOrderStockRepo>>,
    mut stores: Vec<StoreStockModel>,
    all_products: &[ProductStockModel],
) {
    let expect_slset = {
        stores[0].products.push(all_products[0].clone());
        stores[1].products.push(all_products[1].clone());
        stores[0].products.push(all_products[2].clone());
        stores[1].products.push(all_products[3].clone());
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
                expiry: m2.expiry,
            })
        })
        .collect();
    let result = stockrepo.fetch(pids).await;
    assert!(result.is_ok());
    if let Ok(actual) = result {
        assert_eq!(actual.stores.len(), 2);
        assert_eq!(actual.stores[0].products.len(), 2);
        verify_stocklvl_model(&actual, &expect_slset, [0, 0], true);
        verify_stocklvl_model(&actual, &expect_slset, [1, 0], true);
        verify_stocklvl_model(&actual, &expect_slset, [1, 1], true);
        verify_stocklvl_model(&actual, &expect_slset, [0, 1], true);
    }
}

#[tokio::test]
async fn save_fetch_ok() {
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

fn mock_reserve_usr_cb_0(
    ms: &mut StockLevelModelSet,
    req: &OrderLineModelSet,
) -> AppStockRepoReserveReturn {
    assert_eq!(ms.stores.len(), 1);
    assert_eq!(req.lines.len(), 1);
    let saved_store = &mut ms.stores[0];
    let id_combo = (req.lines[0].id().store_id, req.lines[0].id().product_id);
    let product = match id_combo {
        (1013, 9006) => saved_store
            .products
            .iter_mut()
            .find(|p| p.quantity.total == 120 && p.quantity.cancelled == 3)
            .unwrap(),
        _others => {
            assert_eq!(saved_store.products.len(), 1);
            &mut saved_store.products[0]
        }
    };
    assert!(product.quantity.rsv_detail.is_none());
    product.quantity.booked += req.lines[0].qty.reserved;
    product.quantity.rsv_detail = Some(StockQtyRsvModel {
        oid: req.order_id.clone(),
        reserved: req.lines[0].qty.reserved,
    });
    Ok(())
} // end of mock_reserve_usr_cb_0

#[tokio::test]
async fn try_reserve_ok() {
    let mock_warranty = DateTime::parse_from_rfc3339("3015-11-29T15:02:30.005-03:00").unwrap();
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
    let reserve_data = [
        (1013, 9004, 2, "f1726b0c"),
        (1013, 9006, 17, "f1726b0d"),
        (1013, 9006, 7, "17a6b0c3"),
        (1013, 9006, 4, "b0a578"),
        (1014, 9009, 3, "1101011b00d1"),
        (1014, 9009, 1, "a5c8e2083f"),
    ];

    for (store_id, product_id, num_req, order_id) in reserve_data {
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
        assert_eq!(
            ut_retrieve_stocklvl_qty(stockrepo.clone(), 1013, &all_products[2]).await,
            (2, 0, 15)
        );
        assert_eq!(
            ut_retrieve_stocklvl_qty(stockrepo.clone(), 1013, &all_products[4]).await,
            (0, 0, 14)
        );
        assert_eq!(
            ut_retrieve_stocklvl_qty(stockrepo.clone(), 1013, &all_products[8]).await,
            (17 + 7 + 4, 3, 120)
        );
        assert_eq!(
            ut_retrieve_stocklvl_qty(stockrepo.clone(), 1014, &all_products[9]).await,
            (0, 1, 37)
        );
        assert_eq!(
            ut_retrieve_stocklvl_qty(stockrepo.clone(), 1014, &all_products[11]).await,
            ((3 + 1), 1, 46)
        );
    }
    let ol_set = {
        let create_time = DateTime::parse_from_rfc3339("2022-11-29T07:29:01.027-03:00").unwrap();
        let lines = vec![
            (1013, 9004, 2, 3, mock_warranty + Duration::minutes(1)),
            (1013, 9006, 3, 4, mock_rsved_end + Duration::minutes(2)),
            (1014, 9008, 29, 20, mock_warranty + Duration::minutes(3)),
            (1014, 9009, 6, 15, mock_rsved_end + Duration::minutes(4)),
        ];
        ut_oline_init_setup("800eff40", 123, create_time, lines)
    };
    let result = stockrepo.try_reserve(mock_reserve_usr_cb_1, &ol_set).await;
    assert!(result.is_ok());
    {
        assert_eq!(
            ut_retrieve_stocklvl_qty(stockrepo.clone(), 1013, &all_products[2]).await,
            ((2 + 2), 0, 15)
        );
        assert_eq!(
            ut_retrieve_stocklvl_qty(stockrepo.clone(), 1013, &all_products[4]).await,
            (3, 0, 14)
        );
        assert_eq!(
            ut_retrieve_stocklvl_qty(stockrepo.clone(), 1013, &all_products[8]).await,
            (17 + 7 + 4, 3, 120)
        );
        assert_eq!(
            ut_retrieve_stocklvl_qty(stockrepo.clone(), 1014, &all_products[9]).await,
            (29, 1, 37)
        );
        assert_eq!(
            ut_retrieve_stocklvl_qty(stockrepo.clone(), 1014, &all_products[11]).await,
            ((3 + 1 + 6), 1, 46)
        );
    }
    // verify order lines and top-level metadata
    ut_verify_fetch_all_olines_ok(&o_repo).await;
} // end of fn try_reserve_ok

fn mock_reserve_usr_cb_2(
    ms: &mut StockLevelModelSet,
    req: &OrderLineModelSet,
) -> AppStockRepoReserveReturn {
    assert_eq!(req.lines.len(), 2);
    assert_eq!(ms.stores.len(), 1);
    assert_eq!(ms.stores[0].products.len(), 2);
    ms.stores[0]
        .products
        .iter()
        .map(|p| {
            let qty = &p.quantity;
            let actual_remain = qty.num_avail();
            let expect_remain = match p.id_ {
                9002 => 5,
                9003 => 10,
                _others => 0,
            };
            assert_eq!(expect_remain, actual_remain);
        })
        .count();
    let errors = vec![
        OrderLineCreateErrorDto {
            seller_id: req.lines[0].id().store_id,
            product_id: req.lines[0].id().product_id,
            nonexist: None,
            rsv_limit: None,
            shortage: Some(2),
            reason: OrderLineCreateErrorReason::NotEnoughToClaim,
        },
        OrderLineCreateErrorDto {
            seller_id: req.lines[1].id().store_id,
            product_id: req.lines[1].id().product_id,
            nonexist: None,
            rsv_limit: None,
            shortage: Some(1),
            reason: OrderLineCreateErrorReason::OutOfStock,
        },
    ];
    Err(Ok(errors))
}

#[tokio::test]
async fn try_reserve_shortage() {
    let mock_warranty = Local::now().fixed_offset() + Duration::minutes(3);
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
    ut_reserve_init_setup(
        stockrepo.clone(),
        mock_reserve_usr_cb_0,
        mock_warranty,
        1015,
        9003,
        1,
        "f1726b0e",
    )
    .await;
    let ol_set = {
        let create_time = DateTime::parse_from_rfc3339("2022-11-29T06:35:00.519-02:00").unwrap();
        let lines = vec![
            (1015, 9003, 12, 3, mock_warranty.clone()),
            (1015, 9002, 6, 4, mock_warranty.clone()),
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
        assert!(matches!(
            detail.reason,
            OrderLineCreateErrorReason::NotEnoughToClaim
        ));
        let detail = input_errs.remove(0);
        assert_eq!(detail.product_id, 9002);
        assert_eq!(detail.shortage.unwrap(), 1);
        assert!(matches!(
            detail.reason,
            OrderLineCreateErrorReason::OutOfStock
        ));
    }
} // end of  fn try_reserve_shortage

fn mock_reserve_usr_cb_3(
    ms: &mut StockLevelModelSet,
    req: &OrderLineModelSet,
) -> AppStockRepoReserveReturn {
    macro_rules! inner_try_reserve {
        ($prod_id:literal, $expect_tot_qty_1:literal,
         $expect_tot_qty_2:literal, $product_src:expr, $oid:ident,
         $line_rsv_req:ident ) => {{
            let stk_prod = $product_src
                .iter_mut()
                .find(|p| p.id_ == $prod_id && p.quantity.total == $expect_tot_qty_1)
                .unwrap();
            assert!(stk_prod.quantity.rsv_detail.is_none());
            let num_avail = stk_prod.quantity.num_avail();
            assert!(num_avail < $line_rsv_req);
            let _num = stk_prod.quantity.reserve($oid, num_avail);
            $line_rsv_req -= num_avail;
            assert!(stk_prod.quantity.rsv_detail.is_some());
            let stk_prod = $product_src
                .iter_mut()
                .find(|p| p.id_ == $prod_id && p.quantity.total == $expect_tot_qty_2)
                .unwrap();
            assert!(stk_prod.quantity.rsv_detail.is_none());
            let num_avail = stk_prod.quantity.num_avail();
            assert!(num_avail > $line_rsv_req);
            let _num = stk_prod.quantity.reserve($oid, $line_rsv_req);
            assert!(stk_prod.quantity.rsv_detail.is_some());
        }};
    }
    assert_eq!(ms.stores.len(), 1);
    let store = &mut ms.stores[0];
    let oid = req.order_id.as_str();
    req.lines
        .iter()
        .map(|line| {
            let mut line_rsv_req = line.qty.reserved;
            match line.id().product_id {
                9006 => inner_try_reserve!(9006, 120, 14, store.products, oid, line_rsv_req),
                9008 => inner_try_reserve!(9008, 49, 37, store.products, oid, line_rsv_req),
                _others => {
                    assert!(false);
                }
            };
        })
        .count();
    Ok(())
} // end of mock_reserve_usr_cb_3

fn mock_return_usr_cb_1(
    ms: &mut StockLevelModelSet,
    data: StockLevelReturnDto,
) -> Vec<StockReturnErrorDto> {
    assert_eq!(ms.stores.len(), 1);
    let store = &mut ms.stores[0];
    assert_eq!(store.products.len(), 4);
    assert_eq!(data.items.len(), 3);
    data.items
        .into_iter()
        .map(|item| {
            let stk_prod = store
                .products
                .iter_mut()
                .find(|p| {
                    let exp_diff = p.expiry.fixed_offset() - item.expiry;
                    p.id_ == item.product_id && exp_diff.abs() < Duration::seconds(1)
                })
                .unwrap();
            {
                let detail = stk_prod.quantity.rsv_detail.as_ref().unwrap();
                // println!("[DEBUG] prod-typ:{:?}, prod-id:{}, exp:{:?}, \n qty-stats:{:?}",
                //         stk_prod.type_, stk_prod.id_, stk_prod.expiry, stk_prod.quantity );
                assert!(detail.reserved > 0);
            }
            let line_ret_req = item.qty_add as u32;
            let num = stk_prod.quantity.try_return(line_ret_req);
            assert_eq!(num, line_ret_req);
            {
                let _detail = stk_prod.quantity.rsv_detail.as_ref().unwrap();
            }
        })
        .count();
    vec![]
}

#[tokio::test]
async fn try_return_ok() {
    let ds = dstore_ctx_setup();
    let o_repo = app_repo_order(ds).await.unwrap();
    let stockrepo = o_repo.stock();
    let (mock_oid, mock_usr_id, mock_seller) = ("0d14ff5a", 141, 1034);
    let all_products = ut_init_data_product();
    {
        let products = [4, 8, 9, 10]
            .into_iter()
            .map(|idx| all_products[idx].clone())
            .collect::<Vec<_>>();
        let store = StoreStockModel {
            store_id: mock_seller,
            products,
        };
        let slset = StockLevelModelSet {
            stores: vec![store],
        };
        let result = stockrepo.save(slset.clone()).await;
        assert!(result.is_ok());
    }
    let ol_set = {
        let create_time = Local::now().fixed_offset();
        let mock_warranty = create_time + Duration::days(7);
        let lines = vec![
            (
                mock_seller,
                9006,
                123,
                4,
                mock_warranty + Duration::hours(1),
            ),
            (
                mock_seller,
                9008,
                50,
                20,
                mock_warranty + Duration::hours(10),
            ),
        ];
        ut_oline_init_setup(mock_oid, mock_usr_id, create_time, lines)
    };
    let result = stockrepo.try_reserve(mock_reserve_usr_cb_3, &ol_set).await;
    assert!(result.is_ok());
    let data = {
        let items = [
            (mock_seller, 9006u64, 1i32, all_products[8].expiry.clone()),
            (mock_seller, 9008, 1, all_products[9].expiry.clone()),
            (mock_seller, 9008, 5, all_products[10].expiry.clone()),
        ]
        .into_iter()
        .map(|d| InventoryEditStockLevelDto {
            qty_add: d.2,
            store_id: d.0,
            product_id: d.1,
            expiry: d.3.into(),
        })
        .collect();
        StockLevelReturnDto {
            order_id: mock_oid.to_string(),
            items,
        }
    };
    let result = stockrepo.try_return(mock_return_usr_cb_1, data).await;
    assert!(result.is_ok());

    let pids = [4, 8, 9, 10]
        .into_iter()
        .map(|idx| {
            let m = &all_products[idx];
            ProductStockIdentity {
                store_id: mock_seller,
                product_id: m.id_,
                expiry: m.expiry_without_millis(),
            }
        })
        .collect();
    let result = stockrepo.fetch(pids).await;
    assert!(result.is_ok());
    if let Ok(mut mset) = result {
        let mut products = mset.stores.remove(0).products;
        assert_eq!(products.len(), 4);
        products.sort_by(|a, b| {
            if a.id_ == b.id_ {
                a.expiry.cmp(&b.expiry)
            } else {
                a.id_.cmp(&b.id_)
            }
        });
        let expect_seq = vec![(9006u64, 6u32), (9006, 116), (9008, 0), (9008, 44)];
        products
            .into_iter()
            .zip(expect_seq.into_iter())
            .map(|(p, expect)| {
                let actual = (p.id_, p.quantity.booked);
                assert_eq!(actual, expect);
            })
            .count();
    }
} // end of fn  try_return_ok
