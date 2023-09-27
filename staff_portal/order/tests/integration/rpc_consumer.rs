use std::result::Result as DefaultResult;
use serde_json::Value as JsnVal;

use order::api::rpc::route_to_handler;
use order::error::AppError;
use order::AppRpcClientReqProperty;

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


#[tokio::test]
async fn inventory_edit_stock_level_ok() -> DefaultResult<(), AppError>
{
    let shrstate = test_setup_shr_state()?;
    let msgbody = br#"
            [
                {"qty_add":12, "store_id":1006, "product_type": 1, "product_id": 9200125,
                 "expiry": "2023-12-24T07:11:13.730050+07:00"},
                {"qty_add":-18, "store_id":1009, "product_type": 2, "product_id": 7001,
                 "expiry": "2023-12-27T22:19:13.730050+08:00"},
                {"qty_add":50, "store_id":1007, "product_type": 2, "product_id": 20911,
                 "expiry": "2023-12-25T16:27:13.730050+10:00"}
            ]
            "#;
    let req = AppRpcClientReqProperty { retry: 2,  msgbody:msgbody.to_vec(),
            route: "edit_stock_level".to_string()  };
    let result = route_to_handler(req, shrstate).await;
    assert!(result.is_ok());
    let respbody = result.unwrap();
    let result = serde_json::from_slice(&respbody);
    assert!(result.is_ok());
    let value:JsnVal = result.unwrap();
    assert!(value.is_array()); // TODO, should return current stock level
    Ok(())
} // end of fn test_update_product_price_ok

