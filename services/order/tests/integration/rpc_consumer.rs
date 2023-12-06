use std::result::Result as DefaultResult;
use std::sync::Arc;
use chrono::DateTime;
use serde_json::Value as JsnVal;

use order::constant::ProductType;

use order::api::rpc::route_to_handler;
use order::error::AppError;
use order::{AppRpcClientReqProperty, AppSharedState, AppDataStoreContext};
use order::model::{OrderLineModel, OrderLinePriceModel, OrderLineQuantityModel, OrderLineModelSet, OrderLineIdentity};

mod common;
use common::test_setup_shr_state;

#[tokio::test]
async fn  update_product_price_ok() -> DefaultResult<(), AppError>
{
    let shrstate = test_setup_shr_state()?;
    let msgbody = br#"
             [
                 [],
                 {"s_id": 1178913994, "rm_all": false, "deleting": {"item_type":1, "pkg_type":2},
                  "updating": [
                      {"price": 126, "start_after": "2023-09-04T09:11:13+08:00", "product_type": 1,
                       "end_before": "2023-12-24T07:11:13.730050+08:00", "product_id": 9200125},
                      {"price": 106, "start_after": "2023-09-04T04:30:58.070020+07:00", "product_type": 2,
                       "end_before": "2024-07-13T18:11:56.877000+07:00", "product_id": 802007}
                  ],
                  "creating": [
                      {"price": 135, "start_after": "2023-09-10T09:11:13+09:00", "product_type": 1,
                       "end_before": "2023-12-24T07:11:13.730050+09:00", "product_id": 1101601},
                      {"price": 1038, "start_after": "2022-01-20T04:30:58.070020+10:00", "product_type": 2,
                       "end_before": "2024-02-28T18:11:56.877000+10:00", "product_id": 20076}
                  ]
                 },
                 {"callbacks": null, "errbacks": null, "chain": null, "chord": null}
            ]
            "#;
    let req = AppRpcClientReqProperty { retry: 1,  msgbody:msgbody.to_vec(),
            route: "update_store_products".to_string()  };
    let result = route_to_handler(req, shrstate.clone()).await;
    assert!(result.is_ok());
    let msgbody = br#"
             [
                 [],
                 {"s_id": 1178913994, "rm_all": false, "updating": [], "creating": [],
                  "deleting": {"items":[78,90,123], "pkgs":[12,34,56], "item_type":1, "pkg_type":2}
                 },
                 {"callbacks": null, "errbacks": null, "chain": null, "chord": null}
            ]
            "#;
    let req = AppRpcClientReqProperty { retry: 1,  msgbody:msgbody.to_vec(),
            route: "update_store_products".to_string()  };
    let result = route_to_handler(req, shrstate).await;
    assert!(result.is_ok());
    let respbody = result.unwrap();
    let respbody = String::from_utf8(respbody).unwrap();
    assert!(respbody.is_empty()); // task done successfully
    Ok(())
} // end of fn update_product_price_ok


fn verify_reply_stock_level(objs:&Vec<JsnVal>,  expect_product_id:u64,
                            expect_product_type:u8,  expect_qty_total:u32,
                            expect_qty_cancelled:u32 )
{
    let obj = objs.iter().find(|d| {
        if let JsnVal::Object(item) = d {
            let prod_id_v = item.get("product_id").unwrap();
            let prod_typ_v = item.get("product_type").unwrap();
            let actual_product_id = if let JsnVal::Number(id_) = prod_id_v {
                id_.as_u64().unwrap()
            } else { 0 };
            let actual_product_type = if let JsnVal::Number(typ_) = prod_typ_v {
                typ_.as_u64().unwrap()
            } else { 0 };
            expect_product_id == actual_product_id &&
                expect_product_type as u64 == actual_product_type
        } else { false }
    }).unwrap();
    let qty_v = obj.get("quantity").unwrap();
    if let JsnVal::Object(qty) = qty_v {
        let tot_v = qty.get("total").unwrap();
        if let JsnVal::Number(total) = tot_v {
            assert_eq!(total.as_u64().unwrap(), expect_qty_total as u64);
        }
        let cancel_v = qty.get("cancelled").unwrap();
        if let JsnVal::Number(cancel) = cancel_v {
            assert_eq!(cancel.as_u64().unwrap(), expect_qty_cancelled as u64);
        }
    }
} // end of fn verify_reply_stock_level

async fn itest_common_run_rpc_req(shrstate:AppSharedState, route:&str,
                                  msgbody:Vec<u8> ) -> JsnVal
{
    let req = AppRpcClientReqProperty { retry: 2,  msgbody,
            route: route.to_string()  };
    let result = route_to_handler(req, shrstate).await;
    assert!(result.is_ok());
    let respbody = result.unwrap();
    // println!("raw resp body: {:?} \n", String::from_utf8(respbody.clone()).unwrap() );
    let result = serde_json::from_slice(&respbody);
    assert!(result.is_ok());
    result.unwrap()
}

#[tokio::test]
async fn inventory_stock_level_edit_ok() -> DefaultResult<(), AppError>
{
    let shrstate = test_setup_shr_state()?;
    let msgbody = br#"
            [
                {"qty_add":12, "store_id":1006, "product_type": 1, "product_id": 9200125,
                 "expiry": "2099-12-24T07:11:13.730050+07:00"},
                {"qty_add":18, "store_id":1009, "product_type": 2, "product_id": 7001,
                 "expiry": "2099-12-27T22:19:13.730050+08:00"},
                {"qty_add":50, "store_id":1007, "product_type": 2, "product_id": 20911,
                 "expiry": "2099-12-25T16:27:13.730050+10:00"}
            ]
            "#;
    let value = itest_common_run_rpc_req(
        shrstate.clone(), "stock_level_edit", msgbody.to_vec()).await;
    assert!(value.is_array());
    if let JsnVal::Array(items) = value {
        assert_eq!(items.len(), 3);
        verify_reply_stock_level(&items, 9200125, 1, 12, 0);
        verify_reply_stock_level(&items, 20911, 2, 50, 0);
    }
    let msgbody = br#"
            [
                {"qty_add":2, "store_id":1006, "product_type": 1, "product_id": 9200125,
                 "expiry": "2099-12-24T07:11:13.700450+07:00"},
                {"qty_add":-2, "store_id":1009, "product_type": 2, "product_id": 7001,
                 "expiry": "2099-12-27T22:19:13.730050+08:00"},
                {"qty_add":19, "store_id":1007, "product_type": 2, "product_id": 20911,
                 "expiry": "2099-12-25T16:27:14.0060+10:00"}
            ]
            "#;
    let value = itest_common_run_rpc_req(
        shrstate.clone(), "stock_level_edit", msgbody.to_vec()).await;
    assert!(value.is_array());
    if let JsnVal::Array(items) = value {
        assert_eq!(items.len(), 3);
        verify_reply_stock_level(&items, 9200125, 1, 14, 0);
        verify_reply_stock_level(&items, 7001, 2, 18, 2);
    }
    Ok(())
} // end of fn inventory_stock_level_edit_ok


#[tokio::test]
async fn inventory_stock_level_return_caancelled_ok() -> DefaultResult<(), AppError>
{
    let shrstate = test_setup_shr_state()?;
    let msgbody = br#"
            [
                {"qty_add":21, "store_id":1016, "product_type": 2, "product_id": 9200125,
                 "expiry": "2099-12-24T07:11:13.730050+07:00"},
                {"qty_add":40, "store_id":1019, "product_type": 2, "product_id": 7001,
                 "expiry": "2099-12-27T22:19:13.730050+08:00"},
                {"qty_add":39, "store_id":1017, "product_type": 1, "product_id": 20911,
                 "expiry": "2099-12-25T16:27:13.730050+10:00"}
            ]
            "#;
    let value = itest_common_run_rpc_req(
        shrstate.clone(), "stock_level_edit", msgbody.to_vec()).await;
    assert!(value.is_array());
    let msgbody = br#"
            {"order_id":"on1yfa05", "items":[
                {"qty_add":21, "store_id":1016, "product_type": 2, "product_id": 9200125,
                 "expiry": "2099-12-24T07:11:13.730050+07:00"},
                {"qty_add":39, "store_id":1017, "product_type": 1, "product_id": 20911,
                 "expiry": "2099-12-25T16:27:13.730050+10:00"}
            ]}
            "#;
    let value = itest_common_run_rpc_req(
        shrstate, "stock_return_cancelled", msgbody.to_vec()).await;
    assert!(value.is_array());
    Ok(())
} // end of fn inventory_stock_level_return_caancelled_ok


async fn itest_mock_create_order(ds:Arc<AppDataStoreContext>, oid:&str,
                                 usr_id:u32, create_time:&str )
    -> DefaultResult<(), AppError>
{
    use order::repository::app_repo_order;
    use order::api::dto::{CountryCode, PhoneNumberDto, ShippingMethod};
    use order::model::{
        BillingModel, ShippingModel,ContactModel, PhyAddrModel, ShippingOptionModel,
        OrderLineAppliedPolicyModel
    };
    let repo = app_repo_order(ds).await ?;
    let seller_id = 543;
    let create_time    = DateTime::parse_from_rfc3339(create_time).unwrap();
    let reserved_until = DateTime::parse_from_rfc3339("2023-11-15T09:23:50+02:00").unwrap();
    let warranty_until = DateTime::parse_from_rfc3339("2023-12-24T13:39:41+02:00").unwrap();
    let lines = vec![
        OrderLineModel {id_: OrderLineIdentity {store_id: seller_id, product_id:94,
            product_type:ProductType::Package}, price: OrderLinePriceModel { unit: 50, total: 200 },
            qty: OrderLineQuantityModel {reserved: 4, paid: 0, paid_last_update: None},
            policy: OrderLineAppliedPolicyModel { reserved_until, warranty_until }
        },
        OrderLineModel {id_: OrderLineIdentity {store_id: seller_id, product_id:92,
            product_type:ProductType::Item}, price: OrderLinePriceModel { unit: 34, total: 204 },
            qty: OrderLineQuantityModel {reserved: 6, paid: 0, paid_last_update: None},
            policy: OrderLineAppliedPolicyModel { reserved_until, warranty_until }
        }
    ];
    let bl = BillingModel {
        contact: ContactModel { first_name: "Mick".to_string(), last_name: "Alrndre".to_string(),
            phones: vec![PhoneNumberDto{nation:15,number:"55088381".to_string()}],
            emails: vec!["mick@myhome.io".to_string()],
        },
        address: Some(PhyAddrModel { country: CountryCode::ID, region: "Assam".to_string(),
            city: "parikitru".to_string(), distinct: "Beileyz".to_string(),
            street_name: None, detail: "jrkj8h844".to_string()
        })
    };
    let sh = ShippingModel {
        contact: ContactModel { first_name: "Gojira".to_string(), last_name: "Giant".to_string(),
            phones: vec![PhoneNumberDto{nation:102,number:"0080032013".to_string()}],
            emails: vec!["skydiving@a10kmetre.tw".to_string()],
        },
        address: Some(PhyAddrModel { country: CountryCode::TH, region: "ChiangMai".to_string(),
            city: "903ruriufH".to_string(), distinct: "RiceMiller".to_string(),
            street_name: Some("Hodoop".to_string()), detail: "oh8bur".to_string() },
        ),
        option: vec![
            ShippingOptionModel {seller_id, method:ShippingMethod::FedEx}
        ]
    };
    let ol_set = OrderLineModelSet {order_id:oid.to_string(), lines,
                 owner_id:usr_id, create_time };
    let _ = repo.create(ol_set, bl, sh).await?;
    Ok(())
} // end of itest_mock_create_order

async fn itest_mock_create_oline_return(ds:Arc<AppDataStoreContext>, oid:&str,
                                  create_time:&str )
    -> DefaultResult<(), AppError>
{
    use std::collections::HashMap;
    use order::repository::app_repo_order_return;
    use order::model::OrderReturnModel;
    let repo = app_repo_order_return(ds).await?;
    let seller_id = 543;
    let ms = vec![
        OrderReturnModel {
            id_: OrderLineIdentity {store_id: seller_id, product_id:92, product_type:ProductType::Item},
            qty: HashMap::from([(
                    DateTime::parse_from_rfc3339(create_time).unwrap(),
                    (1, (OrderLinePriceModel {unit: 34, total: 34}))
                )])
        },
        OrderReturnModel {
            id_: OrderLineIdentity {store_id: seller_id, product_id:94, product_type:ProductType::Package},
            qty: HashMap::from([(
                    DateTime::parse_from_rfc3339(create_time).unwrap(),
                    (1, (OrderLinePriceModel {unit: 50, total: 50}))
                )])
        }
    ];
    let _num = repo.save(oid, ms).await ?;
    Ok(())
} // end of fn itest_mock_create_oline_return


#[tokio::test]
async fn  replica_orderinfo_payment_ok() -> DefaultResult<(), AppError>
{
    let shrstate = test_setup_shr_state()?;
    // assume web server has created the order.
    itest_mock_create_order(shrstate.datastore().clone(), "18f00429638a0b",
                            2345, "2023-06-01T09:05:30+03:00").await?;
    let msgbody = br#" {"order_id":"18f00429638a0b"} "#;
    let respbody:JsnVal = itest_common_run_rpc_req(
        shrstate, "order_reserved_replica_payment", msgbody.to_vec()).await;
    assert!(respbody.is_object());
    if let JsnVal::Object(obj) = respbody {
        let oid_v = obj.get("oid").unwrap();
        let usr_id_v = obj.get("usr_id").unwrap();
        let olines_v = obj.get("lines").unwrap();
        let bill_v = obj.get("billing").unwrap();
        assert!(oid_v.is_string());
        assert!(usr_id_v.is_u64());
        assert!(olines_v.is_array());
        assert!(bill_v.is_object());
        if let JsnVal::Array(olines) = olines_v {
            assert_eq!(olines.len(), 2);
        }
    }
    Ok(())
} // end of fn replica_orderinfo_payment_ok

#[tokio::test]
async fn  replica_orderinfo_inventory_ok() -> DefaultResult<(), AppError>
{
    let shrstate = test_setup_shr_state()?;
    // assume web server has created the order.
    itest_mock_create_order(shrstate.datastore().clone(), "18f00429c638a0",
                            2345, "2023-05-30T18:58:04+03:00").await?;
    itest_mock_create_oline_return(shrstate.datastore().clone(), "18f00429c638a0",
                                  "2023-05-30T19:05:45+03:00" ).await?;
    let msgbody = br#" {"start":"2023-05-30T17:50:04.001+03:00",
                        "end": "2023-05-30T19:55:00.008+03:00"}
                     "#;
    let respbody = itest_common_run_rpc_req(
        shrstate, "order_reserved_replica_inventory", msgbody.to_vec()).await;
    assert!(respbody.is_object());
    if let JsnVal::Object(obj) = respbody {
        let rsv_v = obj.get("reservations").unwrap();
        let returns_v = obj.get("returns").unwrap();
        assert!(rsv_v.is_array());
        assert!(returns_v.is_array());
        if let JsnVal::Array(rsv) = rsv_v {
            assert_eq!(rsv.len(), 1);
        }
        if let JsnVal::Array(ret) = returns_v {
            assert_eq!(ret.len(), 2);
        }
    }
    Ok(())
} // end of fn replica_orderinfo_inventory_ok

#[tokio::test]
async fn  update_order_payment_status_ok() -> DefaultResult<(), AppError>
{
    let shrstate = test_setup_shr_state()?;
    // assume web server has created the order.
    itest_mock_create_order(shrstate.datastore().clone(), "18f00429638a0b",
                            3456,   "2023-06-01T16:33:40+08:00").await?;
    let msgbody = br#"
            {"oid":"18f00429638a0b", lines:[
                {"seller_id": 543, "product_id": 92, "product_type": 1,
                 "time": "2023-09-17T06:02:45.008+04:00", "qty": 3}
            ]} 
        "#;
    let _respbody = itest_common_run_rpc_req(
        shrstate, "order_reserved_update_payment", msgbody.to_vec()).await;
    Ok(())
}
