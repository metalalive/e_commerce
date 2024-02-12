use std::result::Result as DefaultResult ;
use std::sync::Arc;

use chrono::{Local, DateTime, FixedOffset, Duration};
use hyper::Body as HyperBody;
use hyper::body::Bytes as HyperBytes;
use http::{Request, StatusCode};
use http_body::Body as RawHttpBody;
use serde_json::Value as JsnVal;

use order::{AppRpcClientReqProperty, AppConfig, AppSharedState, AppAuthedClaim};
use order::constant::app_meta;
use order::error::AppError;
use order::api::web::dto::{
    OrderCreateReqData, OrderCreateRespOkDto, OrderEditReqData, OrderLineReqDto,
    OrderCreateRespErrorDto, ContactErrorReason, PhoneNumNationErrorReason
};
use order::api::rpc;
use order::network::WebServiceRoute;

mod common;
use common::{
    test_setup_shr_state, TestWebServer, deserialize_json_template, ITestFinalHttpBody
};
use tokio::sync::Mutex;

const FPATH_NEW_ORDER_OK_1:&'static str  = "/tests/integration/examples/order_new_ok_1.json";
const FPATH_NEW_ORDER_OK_2:&'static str  = "/tests/integration/examples/order_new_ok_2.json";
const FPATH_NEW_ORDER_OK_3:&'static str  = "/tests/integration/examples/order_new_ok_3.json";
const FPATH_NEW_ORDER_CONTACT_ERR:&'static str  = "/tests/integration/examples/order_new_contact_error.json";
const FPATH_EDIT_ORDER_OK_1:&'static str = "/tests/integration/examples/order_edit_ok_1.json";
const FPATH_EDIT_PRODUCTPOLICY_OK_1:&'static str = "/tests/integration/examples/policy_product_edit_ok_1.json";
const FPATH_EDIT_PRODUCTPOLICY_OK_2:&'static str = "/tests/integration/examples/policy_product_edit_ok_2.json";
const FPATH_EDIT_PRODUCTPOLICY_OK_3:&'static str = "/tests/integration/examples/policy_product_edit_ok_3.json";
const FPATH_EDIT_PRODUCTPOLICY_OK_4:&'static str = "/tests/integration/examples/policy_product_edit_ok_4.json";
const FPATH_EDIT_PRODUCTPOLICY_ERR:&'static str = "/tests/integration/examples/policy_product_edit_exceed_limit.json";
const FPATH_EDIT_PRODUCTPRICE_OK_1:&'static str = "/tests/integration/examples/product_price_celery_ok_1.json";
const FPATH_EDIT_PRODUCTPRICE_OK_2:&'static str = "/tests/integration/examples/product_price_celery_ok_2.json";
const FPATH_EDIT_PRODUCTPRICE_OK_3:&'static str = "/tests/integration/examples/product_price_celery_ok_3.json";
const FPATH_EDIT_PRODUCTPRICE_OK_4:&'static str = "/tests/integration/examples/product_price_celery_ok_4.json";
const FPATH_EDIT_PRODUCTPRICE_OK_5:&'static str = "/tests/integration/examples/product_price_celery_ok_5.json";
const FPATH_EDIT_PRODUCTPRICE_OK_6:&'static str = "/tests/integration/examples/product_price_celery_ok_6.json";
const FPATH_EDIT_PRODUCTPRICE_OK_7:&'static str = "/tests/integration/examples/product_price_celery_ok_7.json";
const FPATH_EDIT_STOCK_LVL_OK_1:&'static str  = "/tests/integration/examples/stock_level_edit_ok_1.json";
const FPATH_EDIT_STOCK_LVL_OK_2:&'static str  = "/tests/integration/examples/stock_level_edit_ok_2.json";
const FPATH_EDIT_STOCK_LVL_OK_3:&'static str  = "/tests/integration/examples/stock_level_edit_ok_3.json";
const FPATH_EDIT_STOCK_LVL_OK_4:&'static str  = "/tests/integration/examples/stock_level_edit_ok_4.json";
const FPATH_EDIT_STOCK_LVL_OK_5:&'static str  = "/tests/integration/examples/stock_level_edit_ok_5.json";
const FPATH_RETURN_OLINE_REQ_OK_1:&'static str  = "/tests/integration/examples/oline_return_request_ok_1.json";


fn itest_clone_authed_claim(src:&AppAuthedClaim) -> AppAuthedClaim {
    AppAuthedClaim { profile: src.profile, iat: src.iat, exp: src.exp,
        aud: src.aud.clone(), perms: vec![], quota: vec![]
    }
}

fn setup_mock_authed_claim(usr_id:u32) -> AppAuthedClaim
{
    let now = Local::now().fixed_offset();
    let ts = now.timestamp();
    AppAuthedClaim {
        profile:usr_id, iat: ts - 54, exp: ts + 150, perms:vec![], quota:vec![],
        aud: vec![app_meta::LABAL.to_string()]
    }
}

async fn itest_setup_product_policy(
    cfg:Arc<AppConfig>, srv:Arc<Mutex<WebServiceRoute<ITestFinalHttpBody>>>,
    req_fpath:&'static str, authed_claim:AppAuthedClaim, expect_status:StatusCode
) -> HyperBytes
{
    let uri = format!("/{}/policy/products", cfg.api_server.listen.api_version);
    let reqbody = {
        let result = deserialize_json_template::<JsnVal>(&cfg.basepath, req_fpath);
        let req_body_template = result.unwrap();
        let rb = serde_json::to_string(&req_body_template).unwrap();
        HyperBody::from(rb)
    };
    let mut req = Request::builder().uri(uri.clone()).method("POST")
        .header("content-type", "application/json") .body(reqbody) .unwrap();
    let _ = req.extensions_mut().insert(authed_claim);
    let mut response = TestWebServer::consume(&srv, req).await;
    assert_eq!(response.status(), expect_status);
    // required by UnsyncBoxBody, to access raw data of body
    let bd = response.body_mut();
    let result = bd.data().await;
    if let Some(rb) = result {
        rb.unwrap()
    } else { HyperBytes::new() }
}

fn verify_reply_stock_level(objs:&Vec<JsnVal>,  expect_product_id:u64,
                            expect_product_type:u8,  expect_qty_total:u32,
                            expect_qty_cancelled:u32, expect_qty_booked:u32 )
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
        let book_v = qty.get("booked").unwrap();
        if let JsnVal::Number(book) = book_v {
            assert_eq!(book.as_u64().unwrap(), expect_qty_booked as u64);
        }
    }
} // end of fn verify_reply_stock_level


async fn place_new_order_ok(cfg:Arc<AppConfig>, srv:Arc<Mutex<WebServiceRoute<ITestFinalHttpBody>>>,
                            req_fpath:&'static str, authed_claim:AppAuthedClaim )
    -> DefaultResult<String, AppError>
{
    let listener = &cfg.api_server.listen;
    let reqbody = {
        let rb = deserialize_json_template::<OrderCreateReqData>
            (&cfg.basepath, req_fpath) ? ;
        let rb = serde_json::to_string(&rb) .unwrap();
        HyperBody::from(rb)
    };
    let uri = format!("/{}/order", listener.api_version);
    let mut req = Request::builder().uri(uri).method("POST")
        .header("content-type", "application/json")
        .header("accept", "application/json")
        .body(reqbody)
        .unwrap();
    let _ = req.extensions_mut().insert(authed_claim);

    let mut response = TestWebServer::consume(&srv, req).await;
    //let respbody = response.body_mut().data().await.unwrap().unwrap();
    //let respbody = String::from_utf8(respbody.to_vec()).unwrap();
    //println!("[debug] place-new-order , resp-body : {:?}", respbody);
    assert_eq!(response.status(), StatusCode::CREATED);
    let actual = TestWebServer::to_custom_type::<OrderCreateRespOkDto>
        (response.body_mut())  .await  ? ;
    assert_eq!(actual.order_id.is_empty() ,  false);
    assert!(actual.reserved_lines.len() > 0);
    Ok(actual.order_id)   //Ok(String::new())
}

async fn return_olines_request_ok(cfg:Arc<AppConfig>, srv:Arc<Mutex<WebServiceRoute<ITestFinalHttpBody>>>,
                            req_fpath:&'static str, oid:&str, authed_claim:AppAuthedClaim )
    -> DefaultResult<(), AppError>
{
    let uri = format!("/{}/order/{}/return", cfg.api_server.listen.api_version, oid);
    let req_body = {
        let obj = deserialize_json_template::<Vec<OrderLineReqDto>>
                  (&cfg.basepath, req_fpath) ?;
        let rb = serde_json::to_string(&obj).unwrap();
        HyperBody::from(rb)
    };
    let mut req = Request::builder().uri(uri).method("PATCH")
        .header("content-type", "application/json").body(req_body).unwrap();
    let _ = req.extensions_mut().insert(authed_claim);
    let response = TestWebServer::consume(&srv, req).await;
    // let bodydata = response.body_mut().data().await.unwrap().unwrap();
    // println!("reponse serial body : {:?}", bodydata);
    assert_eq!(response.status(), StatusCode::OK);
    Ok(())
} // end of fn return_olines_request_ok


#[tokio::test]
async fn itest_order_entry() -> DefaultResult<(), AppError>
{
    let shr_state = test_setup_shr_state() ? ;
    let srv = TestWebServer::setup(shr_state.clone());
    let top_lvl_cfg = shr_state.config();
    let mock_authed_usr = 185;
    let authed_claim = setup_mock_authed_claim(mock_authed_usr);
    let _ = itest_setup_product_policy(
        top_lvl_cfg.clone(), srv.clone(), FPATH_EDIT_PRODUCTPOLICY_OK_2,
        itest_clone_authed_claim(&authed_claim),  StatusCode::OK
    ).await ; 
    {
        let raw_body = itest_setup_product_price(shr_state.clone(),
                       FPATH_EDIT_PRODUCTPRICE_OK_4).await;
        let respbody = String::from_utf8(raw_body).unwrap();
        assert!(respbody.is_empty()); // task done successfully
    } {
        let expiry = Local::now().fixed_offset();
        let resp_body = itest_setup_stock_level(shr_state.clone(), expiry,
                   FPATH_EDIT_STOCK_LVL_OK_3).await;
        let items = resp_body.as_array().unwrap();
        assert_eq!(items.len(), 3);
    }
    let oid = place_new_order_ok(
        top_lvl_cfg.clone(), srv.clone(), FPATH_NEW_ORDER_OK_1,
        itest_clone_authed_claim(&authed_claim)
    ).await ?;
    return_olines_request_ok(
        top_lvl_cfg.clone(), srv.clone(), FPATH_RETURN_OLINE_REQ_OK_1,
        oid.as_str(), authed_claim
    ).await ?;
    Ok(())
} // end of fn itest_order_entry


async fn itest_setup_product_price<'a>(
    shrstate:AppSharedState, body_fpath:&'a str
) -> Vec<u8>
{
    let mock_rpc_topic = "update_store_products";
    let cfg = shrstate.config().clone();
    let req = {
        let result = deserialize_json_template::<JsnVal>(&cfg.basepath, body_fpath);
        let req_body_template = result.unwrap();
        let msgbody = req_body_template.to_string().into_bytes();
        AppRpcClientReqProperty { retry: 1, msgbody, route: mock_rpc_topic.to_string() }
    };
    let result = rpc::route_to_handler(req, shrstate).await;
    assert!(result.is_ok());
    result.unwrap()
}

#[tokio::test]
async fn  update_product_price_ok() -> DefaultResult<(), AppError>
{
    let shrstate = test_setup_shr_state()?;
    let subcases = [FPATH_EDIT_PRODUCTPRICE_OK_1, FPATH_EDIT_PRODUCTPRICE_OK_2,
                    FPATH_EDIT_PRODUCTPRICE_OK_3];
    for path in subcases {
        let raw_body = itest_setup_product_price(shrstate.clone(), path).await;
        let respbody = String::from_utf8(raw_body).unwrap();
        assert!(respbody.is_empty()); // task done successfully
    }
    Ok(())
} // end of fn update_product_price_ok


async fn itest_setup_stock_level<'a>(
    shrstate:AppSharedState, expiry:DateTime<FixedOffset> , body_fpath:&'a str
) -> JsnVal
{
    let mock_rpc_topic = "stock_level_edit";
    let cfg = shrstate.config().clone();
    let req = {
        let result = deserialize_json_template::<JsnVal>(&cfg.basepath, body_fpath);
        let mut req_body_template = result.unwrap();
        let items = req_body_template.as_array_mut().unwrap();
        for item in items {
            let t_fmt = expiry.to_rfc3339();
            let map = item.as_object_mut().unwrap();
            let _old_val = map.insert("expiry".to_string(), JsnVal::String(t_fmt));
        }
        let msgbody = req_body_template.to_string().into_bytes();
        AppRpcClientReqProperty { retry: 1, msgbody, route: mock_rpc_topic.to_string() }
    };
    let result = rpc::route_to_handler(req, shrstate).await;
    assert!(result.is_ok());
    let respbody = result.unwrap();
    let result = serde_json::from_slice(&respbody);
    assert!(result.is_ok());
    result.unwrap()
} // end of async fn itest_setup_stock_level

#[tokio::test]
async fn  update_stock_level_ok() -> DefaultResult<(), AppError>
{
    let shrstate = test_setup_shr_state()?;
    let srv = TestWebServer::setup(shrstate.clone());
    let top_lvl_cfg = shrstate.config();
    let expiry = Local::now().fixed_offset() + Duration::days(1);
    {
        let resp_body = itest_setup_stock_level(shrstate.clone(), expiry,
                   FPATH_EDIT_STOCK_LVL_OK_1).await;
        let items = resp_body.as_array().unwrap();
        assert_eq!(items.len(), 3);
        verify_reply_stock_level(&items, 7001, 2, 18, 0, 0);
        verify_reply_stock_level(&items, 9200125, 1, 12, 0, 0);
        verify_reply_stock_level(&items, 20911, 2, 50, 0, 0);
        let resp_body = itest_setup_stock_level(shrstate.clone(), expiry,
                   FPATH_EDIT_STOCK_LVL_OK_2).await;
        let items = resp_body.as_array().unwrap();
        assert_eq!(items.len(), 3);
        verify_reply_stock_level(&items, 9200125, 1, 14, 0, 0);
        verify_reply_stock_level(&items, 7001, 2, 18, 2, 0);
        verify_reply_stock_level(&items, 20912, 2, 19, 0, 0);
    }
    let mock_authed_usr = 186;
    let authed_claim = setup_mock_authed_claim(mock_authed_usr);
    let _ = itest_setup_product_policy(
        top_lvl_cfg.clone(), srv.clone(), FPATH_EDIT_PRODUCTPOLICY_OK_4,
        itest_clone_authed_claim(&authed_claim),  StatusCode::OK
    ).await ; 
    let _raw_body = itest_setup_product_price(shrstate.clone(),
                    FPATH_EDIT_PRODUCTPRICE_OK_5).await;
    let _raw_body = itest_setup_product_price(shrstate.clone(),
                    FPATH_EDIT_PRODUCTPRICE_OK_6).await;
    let _raw_body = itest_setup_product_price(shrstate.clone(),
                    FPATH_EDIT_PRODUCTPRICE_OK_7).await;
    let _oid = place_new_order_ok(
        top_lvl_cfg.clone(), srv.clone(), FPATH_NEW_ORDER_OK_2,
        itest_clone_authed_claim(&authed_claim)
    ).await ?;
    {
        let resp_body = itest_setup_stock_level(shrstate.clone(), expiry,
                   FPATH_EDIT_STOCK_LVL_OK_4).await;
        let items = resp_body.as_array().unwrap();
        assert_eq!(items.len(), 2);
        verify_reply_stock_level(&items, 9200125, 1, 44, 0, 3);
        verify_reply_stock_level(&items, 7001, 2, 28, 2, 5);
        let resp_body = itest_setup_stock_level(shrstate.clone(), expiry + Duration::minutes(2),
                   FPATH_EDIT_STOCK_LVL_OK_4).await;
        let items = resp_body.as_array().unwrap();
        assert_eq!(items.len(), 2);
        verify_reply_stock_level(&items, 9200125, 1, 30, 0, 0);
        verify_reply_stock_level(&items, 7001, 2, 10, 0, 0);
    }
    let _oid = place_new_order_ok(
        top_lvl_cfg.clone(), srv.clone(), FPATH_NEW_ORDER_OK_3,
        itest_clone_authed_claim(&authed_claim)
    ).await ?;
    {
        let resp_body = itest_setup_stock_level(shrstate.clone(), expiry,
                   FPATH_EDIT_STOCK_LVL_OK_5).await;
        let items = resp_body.as_array().unwrap();
        assert_eq!(items.len(), 4);
        verify_reply_stock_level(&items, 9200125, 1, 45, 0, 14);
        verify_reply_stock_level(&items, 7001, 2, 29, 2, 8);
        verify_reply_stock_level(&items, 20911, 2, 51, 0, 4);
        verify_reply_stock_level(&items, 20912, 2, 20, 0, 6);
    }
    Ok(())
} // end of fn update_stock_level_ok


#[tokio::test]
async fn place_new_order_contact_error() -> DefaultResult<(), AppError>
{
    let shr_state = test_setup_shr_state() ? ;
    let srv = TestWebServer::setup(shr_state.clone());
    let top_lvl_cfg = shr_state.config();
    let mock_authed_usr = 231;
    let authed_claim = setup_mock_authed_claim(mock_authed_usr);
    let listener = &top_lvl_cfg.api_server.listen;
    let reqbody = {
        let rb = deserialize_json_template::<OrderCreateReqData>
            (&top_lvl_cfg.basepath, FPATH_NEW_ORDER_CONTACT_ERR) ? ;
        let rb = serde_json::to_string(&rb) .unwrap();
        HyperBody::from(rb)
    };
    let uri = format!("/{}/order", listener.api_version);
    let mut req = Request::builder().uri(uri).method("POST")
        .header("content-type", "application/json")
        .header("accept", "application/json") .body(reqbody)  .unwrap();
    let _ = req.extensions_mut().insert(authed_claim);

    let mut response = TestWebServer::consume(&srv, req).await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let actual = TestWebServer::to_custom_type::<OrderCreateRespErrorDto>
        (response.body_mut())  .await  ? ;
    let contact_err = actual.shipping.unwrap().contact.unwrap();
    let (name_err, phone_err) = (contact_err.first_name.unwrap(),
                                 contact_err.phones.unwrap());
    assert!(matches!(name_err, ContactErrorReason::Empty));
    assert_eq!(phone_err.len(), 2);
    assert!(phone_err[0].is_none());
    let ph_err_1 = phone_err[1].as_ref().unwrap();
    assert!(matches!(ph_err_1.nation.as_ref().unwrap(), PhoneNumNationErrorReason::InvalidCode));
    Ok(())
} // end of place_new_order_contact_error


#[tokio::test]
async fn edit_order_contact_ok() -> DefaultResult<(), AppError>
{
    let shr_state = test_setup_shr_state() ? ;
    let srv = TestWebServer::setup(shr_state.clone());
    let top_lvl_cfg = shr_state.config();
    let mock_authed_usr = 219;
    let authed_claim = setup_mock_authed_claim(mock_authed_usr);
    let reqbody = {
        let mut rb = deserialize_json_template::<OrderEditReqData>
            (&top_lvl_cfg.basepath, FPATH_EDIT_ORDER_OK_1) ? ;
        use std::fmt::Write;
        rb.billing.contact.first_name.clear();
        rb.billing.contact.first_name.write_str("Satunam") .unwrap();
        let rb = serde_json::to_string(&rb).unwrap();
        HyperBody::from(rb)
    };
    let uri = format!("/{ver}/order/{oid}", oid = "r8dj30H",
                      ver = top_lvl_cfg.api_server.listen.api_version);
    let mut req = Request::builder().uri(uri).method("PATCH")
        .header("content-type", "application/json") .body(reqbody) .unwrap();
    let _ = req.extensions_mut().insert(authed_claim);

    let response = TestWebServer::consume(&srv, req).await;
    assert_eq!(response.status(), StatusCode::OK);
    Ok(())
}

#[tokio::test]
async fn add_product_policy_ok() -> DefaultResult<(), AppError>
{
    let shr_state = test_setup_shr_state() ? ;
    let srv = TestWebServer::setup(shr_state.clone());
    let top_lvl_cfg = shr_state.config();
    let mock_authed_usr = 1411;
    let _ = itest_setup_product_policy(
        top_lvl_cfg.clone(), srv.clone(), FPATH_EDIT_PRODUCTPOLICY_OK_1,
        setup_mock_authed_claim(mock_authed_usr),  StatusCode::OK
    ).await;
    let _ = itest_setup_product_policy(
        top_lvl_cfg.clone(), srv.clone(), FPATH_EDIT_PRODUCTPOLICY_OK_3,
        setup_mock_authed_claim(mock_authed_usr),  StatusCode::OK
    ).await;
    Ok(())
} // end of fn add_product_policy_ok

#[tokio::test]
async fn add_product_policy_error() -> DefaultResult<(), AppError>
{
    let shr_state = test_setup_shr_state() ? ;
    let srv = TestWebServer::setup(shr_state.clone());
    let top_lvl_cfg = shr_state.config();
    let mock_authed_usr = 983;
    { // ---- subcase 1 ----
        let uri = format!("/{}/policy/products", top_lvl_cfg.api_server.listen.api_version);
        let reqbody = HyperBody::from("[]".to_string());
        let mut req = Request::builder().uri(uri.clone()).method("POST")
            .header("content-type", "application/json") .body(reqbody) .unwrap();
        let _ = req.extensions_mut().insert(setup_mock_authed_claim(mock_authed_usr));
        let response = TestWebServer::consume(&srv, req).await;
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }
    let resp_rawbytes = {// ---- subcase 2 ----
        let r = itest_setup_product_policy(
            top_lvl_cfg.clone(), srv.clone(), FPATH_EDIT_PRODUCTPOLICY_ERR,
            setup_mock_authed_claim(mock_authed_usr), StatusCode::BAD_REQUEST
        ).await;
        //println!("response body content, first 50 bytes : {:?}", resp_rawbytes.slice(..50) );
        r.to_vec()
    };
    let result = serde_json::from_slice::<JsnVal>(resp_rawbytes.as_slice());
    assert!(result.is_ok());
    if let Ok(mut resp) = result {
        let errors = resp.as_array_mut().unwrap();
        assert_eq!(errors.len(), 1);
        let map = errors.remove(0);
        let prod_typ = map.get("product_type").unwrap().as_u64().unwrap();
        let prod_id  = map.get("product_id").unwrap().as_u64().unwrap() ;
        let _err_typ  = map.get("err_type").unwrap().as_str().unwrap() ;
        let _warranty = map.get("warranty_hours").unwrap().as_object().unwrap();
        assert_eq!(prod_typ, 1u64);
        assert_eq!(prod_id , 10093183u64);
    }
    Ok(())
} // end of fn add_product_policy_error
