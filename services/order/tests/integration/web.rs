use std::result::Result as DefaultResult ;
use std::sync::Arc;

use chrono::{Local, DateTime, FixedOffset, Duration};
use hyper::Body as HyperBody;
use hyper::body::Bytes as HyperBytes;
use http::{Request, StatusCode};
use http_body::Body as RawHttpBody;
use serde_json::{Value as JsnVal, Map};

use order::{
    AppRpcClientReqProperty, AppConfig, AppSharedState, AppAuthedClaim, AppAuthClaimQuota,
    AppAuthClaimPermission, AppAuthPermissionCode, AppAuthQuotaMatCode
};
use order::constant::{app_meta, limit, ProductType};
use order::error::AppError;
use order::api::web::dto::{
    OrderCreateReqData, OrderCreateRespOkDto, OrderEditReqData, ContactErrorReason,
    OrderCreateRespErrorDto, PhoneNumNationErrorReason
};
use order::api::rpc;
use order::network::WebServiceRoute;

mod common;
use common::{
    test_setup_shr_state, TestWebServer, deserialize_json_template, ITestFinalHttpBody
};
use tokio::sync::Mutex;

fn itest_clone_authed_claim(src:&AppAuthedClaim) -> AppAuthedClaim {
    AppAuthedClaim { profile: src.profile, iat: src.iat, exp: src.exp,
        aud: src.aud.clone(),
        perms: src.perms.iter().map(Clone::clone).collect::<Vec<_>>(),
        quota: src.quota.iter().map(Clone::clone).collect::<Vec<_>>() 
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
    req_fpath:&'static str, mut authed_claim:AppAuthedClaim, expect_status:StatusCode
) -> HyperBytes
{
    let uri = format!("/{}/policy/products", cfg.api_server.listen.api_version);
    let reqbody = {
        let result = deserialize_json_template::<JsnVal>(&cfg.basepath, req_fpath);
        let req_body_template = result.unwrap();
        let rb = serde_json::to_string(&req_body_template).unwrap();
        HyperBody::from(rb)
    };
    {
        let perm = AppAuthClaimPermission {app_code:app_meta::RESOURCE_QUOTA_AP_CODE,
                   codename:AppAuthPermissionCode::can_create_product_policy};
        let res_limit = AppAuthClaimQuota {app_code:app_meta::RESOURCE_QUOTA_AP_CODE,
                   mat_code:AppAuthQuotaMatCode::NumProductPolicies, maxnum:50};
        authed_claim.perms.push(perm);
        authed_claim.quota.push(res_limit);
    }
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
} // end of fn itest_setup_product_policy

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


async fn place_new_order_ok(
    cfg:Arc<AppConfig>, srv:Arc<Mutex<WebServiceRoute<ITestFinalHttpBody>>>,
    req_fpath:&'static str, mut authed_claim:AppAuthedClaim
) -> DefaultResult<String, AppError>
{
    let listener = &cfg.api_server.listen;
    let reqbody = {
        let rb = deserialize_json_template::<OrderCreateReqData>
            (&cfg.basepath, req_fpath) ? ;
        let rb = serde_json::to_string(&rb) .unwrap();
        HyperBody::from(rb)
    };
    authed_claim.quota = [
        (AppAuthQuotaMatCode::NumEmails, 51),
        (AppAuthQuotaMatCode::NumPhones, 52),
        (AppAuthQuotaMatCode::NumOrderLines, 53),
    ].into_iter().map(|(mat_code, maxnum)|
        AppAuthClaimQuota {mat_code, maxnum, app_code:app_meta::RESOURCE_QUOTA_AP_CODE}
    ).collect::<Vec<_>>();

    let uri = format!("/{}/order", listener.api_version);
    let mut req = Request::builder().uri(uri).method("POST")
        .header("content-type", "application/json")
        .header("accept", "application/json")
        .body(reqbody)
        .unwrap();
    let _ = req.extensions_mut().insert(authed_claim);

    let mut response = TestWebServer::consume(&srv, req).await;
    if response.status() != StatusCode::CREATED {
        let respbody = response.body_mut().data().await.unwrap().unwrap();
        let respbody = String::from_utf8(respbody.to_vec()).unwrap();
        println!("[debug] place-new-order , error-resp-body : {:?}", respbody);
    }
    assert_eq!(response.status(), StatusCode::CREATED);
    let actual = TestWebServer::to_custom_type::<OrderCreateRespOkDto>
        (response.body_mut())  .await  ? ;
    assert_eq!(actual.order_id.is_empty() ,  false);
    assert!(actual.reserved_lines.len() > 0);
    Ok(actual.order_id)   //Ok(String::new())
} // end of fn place_new_order_ok

async fn itest_return_olines_request(
    cfg:Arc<AppConfig>, srv:Arc<Mutex<WebServiceRoute<ITestFinalHttpBody>>>,
    req_fpath:&'static str, oid:&str, mut authed_claim:AppAuthedClaim,
    expect_status:StatusCode
) -> Vec<u8>
{
    {
        let perm = AppAuthClaimPermission {app_code:app_meta::RESOURCE_QUOTA_AP_CODE,
                   codename:AppAuthPermissionCode::can_create_return_req};
        authed_claim.perms.push(perm);
    }
    let uri = format!("/{}/order/{}/return", cfg.api_server.listen.api_version, oid);
    let req_body = {
        let result = deserialize_json_template::<JsnVal>(&cfg.basepath, req_fpath);
        let obj = result.unwrap();
        let rb = serde_json::to_string(&obj).unwrap();
        HyperBody::from(rb)
    };
    let mut req = Request::builder().uri(uri).method("PATCH")
        .header("content-type", "application/json").body(req_body).unwrap();
    let _ = req.extensions_mut().insert(authed_claim);
    let mut response = TestWebServer::consume(&srv, req).await;
    let resp_body = response.body_mut().data().await.unwrap().unwrap();
    if response.status() != expect_status {
        println!("reponse serial body : {:?}", resp_body);
    }
    assert_eq!(response.status(), expect_status);
    resp_body.to_vec()
} // end of fn itest_return_olines_request


#[tokio::test]
async fn new_order_then_return() -> DefaultResult<(), AppError>
{
    const FPATH_EDIT_PRODUCTPOLICY_OK:&str = "/tests/integration/examples/policy_product_edit_ok_2.json";
    const FPATH_EDIT_PRODUCTPRICE_OK:&str = "/tests/integration/examples/product_price_celery_ok_4.json";
    const FPATH_EDIT_STOCK_LVL_OK:&str  = "/tests/integration/examples/stock_level_edit_ok_3.json";
    const FPATH_NEW_ORDER_OK:&str  = "/tests/integration/examples/order_new_ok_1.json";
    const FPATH_RETURN_OLINE_REQ_OK:&str  = "/tests/integration/examples/oline_return_request_ok_1.json";
    let shr_state = test_setup_shr_state() ? ;
    let srv = TestWebServer::setup(shr_state.clone());
    let top_lvl_cfg = shr_state.config();
    let mock_authed_usr = 185;
    let authed_claim = setup_mock_authed_claim(mock_authed_usr);
    let _ = itest_setup_product_policy(
        top_lvl_cfg.clone(), srv.clone(), FPATH_EDIT_PRODUCTPOLICY_OK,
        itest_clone_authed_claim(&authed_claim),  StatusCode::OK
    ).await ; 
    {
        let raw_body = itest_setup_product_price(shr_state.clone(),
                       FPATH_EDIT_PRODUCTPRICE_OK).await;
        let respbody = String::from_utf8(raw_body).unwrap();
        assert!(respbody.is_empty()); // task done successfully
    } {
        let expiry = Local::now().fixed_offset() + Duration::minutes(1);
        let resp_body = itest_setup_stock_level(shr_state.clone(), expiry,
                   FPATH_EDIT_STOCK_LVL_OK).await;
        let items = resp_body.as_array().unwrap();
        assert_eq!(items.len(), 3);
    }
    let oid = place_new_order_ok(
        top_lvl_cfg.clone(), srv.clone(), FPATH_NEW_ORDER_OK,
        itest_clone_authed_claim(&authed_claim)
    ).await ?;
    let _ = itest_return_olines_request(
        top_lvl_cfg.clone(), srv.clone(), FPATH_RETURN_OLINE_REQ_OK,
        oid.as_str(), authed_claim,  StatusCode::OK
    ).await ;
    Ok(())
} // end of fn new_order_then_return


async fn itest_setup_product_price<'a>(
    shrstate:AppSharedState, body_fpath:&'a str
) -> Vec<u8>
{
    let mock_rpc_topic = "rpc.order.update_store_products";
    let cfg = shrstate.config().clone();
    let req = {
        let result = deserialize_json_template::<JsnVal>(&cfg.basepath, body_fpath);
        let req_body_template = result.unwrap();
        let msgbody = req_body_template.to_string().into_bytes();
        AppRpcClientReqProperty { start_time: Local::now().fixed_offset(),
            msgbody, route: mock_rpc_topic.to_string() }
    };
    let result = rpc::route_to_handler(req, shrstate).await;
    assert!(result.is_ok());
    result.unwrap()
}

#[tokio::test]
async fn  update_product_price_ok() -> DefaultResult<(), AppError>
{
    let shrstate = test_setup_shr_state()?;
    let subcases = [
        "/tests/integration/examples/product_price_celery_ok_1.json",
        "/tests/integration/examples/product_price_celery_ok_2.json",
        "/tests/integration/examples/product_price_celery_ok_3.json"
    ];
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
    let mock_rpc_topic = "rpc.order.stock_level_edit";
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
        AppRpcClientReqProperty { start_time: Local::now().fixed_offset(),
            msgbody, route: mock_rpc_topic.to_string() }
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
    const FPATH_EDIT_PRODUCTPOLICY_OK:&str = "/tests/integration/examples/policy_product_edit_ok_4.json";
    const FPATH_EDIT_PRODUCTPRICE_OK:[&str;3] = [
        "/tests/integration/examples/product_price_celery_ok_5.json",
        "/tests/integration/examples/product_price_celery_ok_6.json",
        "/tests/integration/examples/product_price_celery_ok_7.json"
    ];
    const FPATH_EDIT_STOCK_LVL_OK:[&str;4] = [
        "/tests/integration/examples/stock_level_edit_ok_1.json",
        "/tests/integration/examples/stock_level_edit_ok_2.json",
        "/tests/integration/examples/stock_level_edit_ok_4.json",
        "/tests/integration/examples/stock_level_edit_ok_5.json"
    ];
    const FPATH_NEW_ORDER_OK:[&str;2]  = ["/tests/integration/examples/order_new_ok_2.json",
                                          "/tests/integration/examples/order_new_ok_3.json"];
    let shrstate = test_setup_shr_state()?;
    let srv = TestWebServer::setup(shrstate.clone());
    let top_lvl_cfg = shrstate.config();
    let expiry = Local::now().fixed_offset() + Duration::days(1);
    {
        let resp_body = itest_setup_stock_level(shrstate.clone(), expiry,
                   FPATH_EDIT_STOCK_LVL_OK[0]).await;
        let items = resp_body.as_array().unwrap();
        assert_eq!(items.len(), 3);
        verify_reply_stock_level(&items, 7001, 2, 18, 0, 0);
        verify_reply_stock_level(&items, 9200125, 1, 12, 0, 0);
        verify_reply_stock_level(&items, 20911, 2, 50, 0, 0);
        let resp_body = itest_setup_stock_level(shrstate.clone(), expiry,
                   FPATH_EDIT_STOCK_LVL_OK[1]).await;
        let items = resp_body.as_array().unwrap();
        assert_eq!(items.len(), 3);
        verify_reply_stock_level(&items, 9200125, 1, 14, 0, 0);
        verify_reply_stock_level(&items, 7001, 2, 18, 2, 0);
        verify_reply_stock_level(&items, 20912, 2, 19, 0, 0);
    }
    let mock_authed_usr = 186;
    let authed_claim = setup_mock_authed_claim(mock_authed_usr);
    let _ = itest_setup_product_policy(
        top_lvl_cfg.clone(), srv.clone(), FPATH_EDIT_PRODUCTPOLICY_OK,
        itest_clone_authed_claim(&authed_claim),  StatusCode::OK
    ).await ; 
    let _raw_body = itest_setup_product_price(shrstate.clone(),
                    FPATH_EDIT_PRODUCTPRICE_OK[0]).await;
    let _raw_body = itest_setup_product_price(shrstate.clone(),
                    FPATH_EDIT_PRODUCTPRICE_OK[1]).await;
    let _raw_body = itest_setup_product_price(shrstate.clone(),
                    FPATH_EDIT_PRODUCTPRICE_OK[2]).await;
    let _oid = place_new_order_ok(
        top_lvl_cfg.clone(), srv.clone(), FPATH_NEW_ORDER_OK[0],
        itest_clone_authed_claim(&authed_claim)
    ).await ?;
    {
        let resp_body = itest_setup_stock_level(shrstate.clone(), expiry,
                   FPATH_EDIT_STOCK_LVL_OK[2]).await;
        let items = resp_body.as_array().unwrap();
        assert_eq!(items.len(), 2);
        verify_reply_stock_level(&items, 9200125, 1, 44, 0, 3);
        verify_reply_stock_level(&items, 7001, 2, 28, 2, 5);
        let resp_body = itest_setup_stock_level(shrstate.clone(), expiry + Duration::minutes(2),
                   FPATH_EDIT_STOCK_LVL_OK[2]).await;
        let items = resp_body.as_array().unwrap();
        assert_eq!(items.len(), 2);
        verify_reply_stock_level(&items, 9200125, 1, 30, 0, 0);
        verify_reply_stock_level(&items, 7001, 2, 10, 0, 0);
    }
    let _oid = place_new_order_ok(
        top_lvl_cfg.clone(), srv.clone(), FPATH_NEW_ORDER_OK[1],
        itest_clone_authed_claim(&authed_claim)
    ).await ?;
    {
        let resp_body = itest_setup_stock_level(shrstate.clone(), expiry,
                   FPATH_EDIT_STOCK_LVL_OK[3]).await;
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
    const FPATH_NEW_ORDER_CONTACT_ERR:&str  = "/tests/integration/examples/order_new_contact_error.json";
    let shr_state = test_setup_shr_state() ? ;
    let srv = TestWebServer::setup(shr_state.clone());
    let top_lvl_cfg = shr_state.config();
    let mock_authed_usr = 231;
    let authed_claim = {
        let mut a = setup_mock_authed_claim(mock_authed_usr);
        a.quota = [
            (AppAuthQuotaMatCode::NumEmails, 17),
            (AppAuthQuotaMatCode::NumPhones, 18),
            (AppAuthQuotaMatCode::NumOrderLines, 19),
        ].into_iter().map(|(mat_code, maxnum)|
            AppAuthClaimQuota {mat_code, maxnum, app_code:app_meta::RESOURCE_QUOTA_AP_CODE}
        ).collect::<Vec<_>>();
        a
    };
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
async fn place_new_order_quota_violation() -> DefaultResult<(), AppError>
{
    const FPATH_NEW_ORDER: &str  = "/tests/integration/examples/order_new_ok_5.json";
    let shr_state = test_setup_shr_state() ? ;
    let srv = TestWebServer::setup(shr_state.clone());
    let top_lvl_cfg = shr_state.config();
    let mock_authed_usr = 144;
    let authed_claim = {
        let mut a = setup_mock_authed_claim(mock_authed_usr);
        a.quota = [
            (AppAuthQuotaMatCode::NumEmails, 1),
            (AppAuthQuotaMatCode::NumPhones, 2),
            (AppAuthQuotaMatCode::NumOrderLines, 1),
        ].into_iter().map(|(mat_code, maxnum)|
            AppAuthClaimQuota {mat_code, maxnum, app_code:app_meta::RESOURCE_QUOTA_AP_CODE}
        ).collect::<Vec<_>>();
        a
    };
    let listener = &top_lvl_cfg.api_server.listen;
    let reqbody = {
        let rb = deserialize_json_template::<OrderCreateReqData>
            (&top_lvl_cfg.basepath, FPATH_NEW_ORDER) ? ;
        let rb = serde_json::to_string(&rb) .unwrap();
        HyperBody::from(rb)
    };
    let uri = format!("/{}/order", listener.api_version);
    let mut req = Request::builder().uri(uri).method("POST")
        .header("content-type", "application/json")
        .header("accept", "application/json") .body(reqbody)  .unwrap();
    let _ = req.extensions_mut().insert(authed_claim);

    let mut response = TestWebServer::consume(&srv, req).await;
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    let actual = TestWebServer::to_custom_type::<OrderCreateRespErrorDto>
        (response.body_mut())  .await  ? ;
    let oline_err = actual.quota_olines.unwrap();
    assert_eq!(oline_err.max_, 1);
    let contact_errs = (
        actual.shipping.unwrap().contact.unwrap(),
        actual.billing.unwrap().contact.unwrap(),
    );
    let (email_err, phone_err) = (contact_errs.1.quota_email.unwrap(),
                                 contact_errs.0.quota_phone.unwrap());
    assert_eq!(email_err.max_, 1);
    assert_eq!(phone_err.max_, 2);
    assert_eq!(email_err.given, 3);
    assert_eq!(phone_err.given, 3);
    Ok(())
} // end of fn place_new_order_quota_violation


#[tokio::test]
async fn edit_order_contact_ok() -> DefaultResult<(), AppError>
{
    const FPATH_EDIT_ORDER_OK:&str = "/tests/integration/examples/order_edit_ok_1.json";
    let shr_state = test_setup_shr_state() ? ;
    let srv = TestWebServer::setup(shr_state.clone());
    let top_lvl_cfg = shr_state.config();
    let mock_authed_usr = 219;
    let authed_claim = setup_mock_authed_claim(mock_authed_usr);
    let reqbody = {
        let mut rb = deserialize_json_template::<OrderEditReqData>
            (&top_lvl_cfg.basepath, FPATH_EDIT_ORDER_OK) ? ;
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
    const FPATH_EDIT_PRODUCTPOLICY_OK:[&str;2] = [
        "/tests/integration/examples/policy_product_edit_ok_1.json",
        "/tests/integration/examples/policy_product_edit_ok_3.json"
    ];
    let shr_state = test_setup_shr_state() ? ;
    let srv = TestWebServer::setup(shr_state.clone());
    let top_lvl_cfg = shr_state.config();
    let mock_authed_usr = 1411;
    for path in FPATH_EDIT_PRODUCTPOLICY_OK {
        let _ = itest_setup_product_policy(
            top_lvl_cfg.clone(), srv.clone(), path,
            setup_mock_authed_claim(mock_authed_usr),  StatusCode::OK
        ).await;
    }
    Ok(())
} // end of fn add_product_policy_ok

#[tokio::test]
async fn add_product_policy_permission_error() -> DefaultResult<(), AppError>
{
    let shr_state = test_setup_shr_state() ? ;
    let srv = TestWebServer::setup(shr_state.clone());
    let top_lvl_cfg = shr_state.config();
    let mock_auth_claim = {
        let mut a = setup_mock_authed_claim(8964);
        let res_limit = AppAuthClaimQuota {app_code:app_meta::RESOURCE_QUOTA_AP_CODE,
                   mat_code:AppAuthQuotaMatCode::NumProductPolicies, maxnum:4};
        a.quota.push(res_limit);
        a
    }; // missing permission
    let uri = format!("/{}/policy/products", top_lvl_cfg.api_server.listen.api_version);
    let reqbody = HyperBody::from("[]".to_string());
    let mut req = Request::builder().uri(uri.clone()).method("POST")
        .header("content-type", "application/json") .body(reqbody) .unwrap();
    let _ = req.extensions_mut().insert(mock_auth_claim);
    let response = TestWebServer::consume(&srv, req).await;
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    Ok(())
} // end of fn add_product_policy_permission_error

#[tokio::test]
async fn add_product_policy_quota_violation() -> DefaultResult<(), AppError>
{
    const FPATH_EDIT_PRODUCTPOLICY_ERR:&str = "/tests/integration/examples/policy_product_edit_exceed_limit.json";
    let shr_state = test_setup_shr_state() ? ;
    let srv = TestWebServer::setup(shr_state.clone());
    let top_lvl_cfg = shr_state.config();
    let mock_auth_claim = {
        let mut a = setup_mock_authed_claim(8964);
        let perm = AppAuthClaimPermission {app_code:app_meta::RESOURCE_QUOTA_AP_CODE,
                   codename:AppAuthPermissionCode::can_create_product_policy};
        let res_limit = AppAuthClaimQuota {app_code:app_meta::RESOURCE_QUOTA_AP_CODE,
                   mat_code:AppAuthQuotaMatCode::NumProductPolicies, maxnum:1};
        a.perms.push(perm);
        a.quota.push(res_limit);
        a
    };
    let _resp_rawbytes = {// ---- subcase 2 ----
        let r = itest_setup_product_policy(
            top_lvl_cfg.clone(), srv.clone(), FPATH_EDIT_PRODUCTPOLICY_ERR,
            mock_auth_claim, StatusCode::PAYLOAD_TOO_LARGE
        ).await;
        r.to_vec()
    };
    Ok(())
} // end of fn add_product_policy_quota_violation

#[tokio::test]
async fn add_product_policy_request_error() -> DefaultResult<(), AppError>
{
    const FPATH_EDIT_PRODUCTPOLICY_ERR:&str = "/tests/integration/examples/policy_product_edit_exceed_limit.json";
    let shr_state = test_setup_shr_state() ? ;
    let srv = TestWebServer::setup(shr_state.clone());
    let top_lvl_cfg = shr_state.config();
    let mock_auth_claim = {
        let mut a = setup_mock_authed_claim(8964);
        let perm = AppAuthClaimPermission {app_code:app_meta::RESOURCE_QUOTA_AP_CODE,
                   codename:AppAuthPermissionCode::can_create_product_policy};
        let res_limit = AppAuthClaimQuota {app_code:app_meta::RESOURCE_QUOTA_AP_CODE,
                   mat_code:AppAuthQuotaMatCode::NumProductPolicies, maxnum:50};
        a.perms.push(perm);
        a.quota.push(res_limit);
        a
    };
    { // ---- subcase 1 ----
        let uri = format!("/{}/policy/products", top_lvl_cfg.api_server.listen.api_version);
        let reqbody = HyperBody::from("[]".to_string());
        let mut req = Request::builder().uri(uri.clone()).method("POST")
            .header("content-type", "application/json") .body(reqbody) .unwrap();
        let _ = req.extensions_mut().insert(itest_clone_authed_claim(&mock_auth_claim));
        let response = TestWebServer::consume(&srv, req).await;
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }
    let resp_rawbytes = {// ---- subcase 2 ----
        let r = itest_setup_product_policy(
            top_lvl_cfg.clone(), srv.clone(), FPATH_EDIT_PRODUCTPOLICY_ERR,
            mock_auth_claim, StatusCode::BAD_REQUEST
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
} // end of fn add_product_policy_request_error


async fn itest_setup_create_order(
    shrstate: AppSharedState, srv:Arc<Mutex<WebServiceRoute<ITestFinalHttpBody>>>,
    time_now: DateTime<FixedOffset>, authed_claim: & AppAuthedClaim,
    fpath_prod_policy:&'static str,  fpath_prod_price:&'static str,
    fpath_stock_edit:&'static str,   fpath_new_order:&'static str,
) -> String
{
    let top_lvl_cfg = shrstate.config();
    let expiry = time_now + Duration::days(1);
    let resp_body = itest_setup_stock_level(shrstate.clone(), expiry,
               fpath_stock_edit ).await;
    let items = resp_body.as_array().unwrap();
    assert!(items.len() > 0);
    let _ = itest_setup_product_policy(
        top_lvl_cfg.clone(), srv.clone(), fpath_prod_policy,
        itest_clone_authed_claim(authed_claim),  StatusCode::OK
    ).await ; 
    let raw_body = itest_setup_product_price(shrstate.clone(), fpath_prod_price).await;
    let resp_body = String::from_utf8(raw_body).unwrap();
    assert!(resp_body.is_empty()); // task done successfully
    let result = place_new_order_ok( top_lvl_cfg.clone(), srv, fpath_new_order,
        itest_clone_authed_claim(authed_claim)
    ).await;
    result.unwrap()
}

async fn itest_setup_get_order_billing(shrstate: AppSharedState, oid:String) -> JsnVal
{
    const FPATH_REP_PAYMENT_TEMPLATE:&str = "/tests/integration/examples/replica_payment_template.json";
    let mock_rpc_topic = "rpc.order.order_reserved_replica_payment";
    let cfg = shrstate.config().clone();
    let req = {
        let result = deserialize_json_template::<JsnVal>(
            &cfg.basepath, FPATH_REP_PAYMENT_TEMPLATE);
        let mut req_body_template = result.unwrap();
        let obj = req_body_template.as_object_mut().unwrap();
        obj.insert("order_id".to_string(), JsnVal::String(oid)) ;
        let msgbody = req_body_template.to_string().into_bytes();
        AppRpcClientReqProperty { start_time: Local::now().fixed_offset(),
            msgbody, route: mock_rpc_topic.to_string() }
    };
    let result = rpc::route_to_handler(req, shrstate).await;
    assert!(result.is_ok());
    let raw = result.unwrap();
    let result = serde_json::from_slice(&raw);
    assert!(result.is_ok());
    result.unwrap()
}

fn itest_verify_order_billing(
    actual:JsnVal, expect_oid:&str, expect_usr_id:u32,
    expect_lines:Vec<(u32, ProductType, u64, u32, u32)>
) {
    let actual = actual.as_object().unwrap();
    let oid = actual.get("oid").unwrap().as_str().unwrap();
    let usr_id = actual.get("usr_id").unwrap().as_u64().unwrap();
    let olines = actual.get("lines").unwrap().as_array().unwrap();
    let bill   = actual.get("billing").unwrap().as_object().unwrap() ;
    assert_eq!(oid, expect_oid);
    assert_eq!(usr_id as u32, expect_usr_id);
    assert_eq!(olines.len(), expect_lines.len());
    assert!(bill.contains_key("contact"));
    expect_lines.into_iter().map(|expect| {
        let actual_line = olines.iter().find_map(|oline| {
            let map = oline.as_object().unwrap();
            let store_id = {
                let s = map.get("seller_id").unwrap().as_u64().unwrap();
                u32::try_from(s).unwrap()
            };
            let product_id   = map.get("product_id").unwrap().as_u64().unwrap();
            let product_type = {
                let p = map.get("product_type").unwrap().as_u64().unwrap();
                ProductType::from(p as u8)
            };
            let cond = (store_id == expect.0) && (product_id == expect.2)
                && (product_type == expect.1);
            if cond {Some(map)} else {None}
        }).unwrap();
        let qty = {
            let q = actual_line.get("quantity").unwrap().as_u64().unwrap();
            u32::try_from(q).unwrap()
        };
        let amount_total = {
            let _amount = actual_line.get("amount").unwrap().as_object().unwrap();
            let _total = _amount.get("total").unwrap().as_u64().unwrap();
            u32::try_from(_total).unwrap()
        };
        assert_eq!(qty, expect.3);
        assert_eq!(amount_total, expect.4);
    }).count();
} // end of fn itest_verify_order_billing

async fn itest_update_payment_status( shrstate:AppSharedState, oid:String , 
    last_paid: Vec<(u32, ProductType, u64, u32, DateTime<FixedOffset>)> 
) {
    const FPATH_UPDATE_PAYMENT_TEMPLATE:&str = "/tests/integration/examples/update_payment_template.json";
    let mock_rpc_topic = "rpc.order.order_reserved_update_payment";
    assert!(!last_paid.is_empty());
    let cfg = shrstate.config().clone();
    let req = {
        let result = deserialize_json_template::<JsnVal>(
            &cfg.basepath, FPATH_UPDATE_PAYMENT_TEMPLATE);
        let mut req_body_template = result.unwrap();
        let obj = req_body_template.as_object_mut().unwrap();
        obj.insert("oid".to_string(), JsnVal::String(oid)) ;
        let lines = obj.get_mut("lines").unwrap().as_array_mut().unwrap();
        lines.clear();
        last_paid.into_iter().map(|item| {
            let mut info = Map::new();
            let prod_typ:u8 = item.1.clone().into();
            info.insert("seller_id".to_string(),    JsnVal::Number(item.0.into())) ;
            info.insert("product_type".to_string(), JsnVal::Number(prod_typ.into())) ;
            info.insert("product_id".to_string(),   JsnVal::Number(item.2.into())) ;
            info.insert("qty".to_string(),  JsnVal::Number(item.3.into())) ;
            info.insert("time".to_string(), JsnVal::String(item.4.to_rfc3339())) ;
            lines.push(JsnVal::Object(info));
        }).count();
        let msgbody = req_body_template.to_string().into_bytes();
        AppRpcClientReqProperty { start_time: Local::now().fixed_offset(),
            msgbody, route: mock_rpc_topic.to_string() }
    };
    let result = rpc::route_to_handler(req, shrstate).await;
    assert!(result.is_ok());
    let raw = result.unwrap();
    let result = serde_json::from_slice::<JsnVal>(&raw);
    assert!(result.is_ok());
    if let Ok(resp_body) = result {
        let map = resp_body.as_object().unwrap();
        let errors = map.get("lines").unwrap().as_array().unwrap();
        assert!(errors.is_empty());
    }
} // end of fn itest_update_payment_status

#[tokio::test]
async fn  replica_update_order_payment() -> DefaultResult<(), AppError>
{
    const FPATH_EDIT_PRODUCTPOLICY: & str = "/tests/integration/examples/policy_product_edit_ok_5.json";
    const FPATH_EDIT_PRODUCTPRICE : & str = "/tests/integration/examples/product_price_celery_ok_8.json";
    const FPATH_EDIT_STOCK_LVL : & str = "/tests/integration/examples/stock_level_edit_ok_6.json";
    const FPATH_NEW_ORDER      : & str = "/tests/integration/examples/order_new_ok_4.json";
    let shrstate = test_setup_shr_state()?;
    let srv = TestWebServer::setup(shrstate.clone());
    let time_now = Local::now().fixed_offset();
    let (mock_authed_usr, mock_seller) = (188, 543);
    let authed_claim = setup_mock_authed_claim(mock_authed_usr);
    let oid = itest_setup_create_order( shrstate.clone(), srv, time_now,
        &authed_claim,  FPATH_EDIT_PRODUCTPOLICY, FPATH_EDIT_PRODUCTPRICE, 
        FPATH_EDIT_STOCK_LVL, FPATH_NEW_ORDER
    ).await;
    // Assume payment service requests the order billing
    let resp_body = itest_setup_get_order_billing(shrstate.clone(), oid.clone()).await;
    itest_verify_order_billing( resp_body, oid.as_str(), mock_authed_usr,
        vec![(mock_seller, ProductType::Package , 20094, 19, 19*59),
             (mock_seller, ProductType::Item, 20092, 13, 13*15)]
    );
    itest_update_payment_status(
        shrstate.clone(), oid.clone(), 
        vec![(mock_seller, ProductType::Package , 20094, 1, time_now + Duration::seconds(5)),
             (mock_seller, ProductType::Item, 20092, 1, time_now + Duration::seconds(9))]
    ).await;
    itest_update_payment_status(
        shrstate.clone(), oid.clone(), 
        vec![(mock_seller, ProductType::Item, 20092, 7, time_now + Duration::seconds(11))]
    ).await;
    itest_update_payment_status(  shrstate, oid, 
        vec![(mock_seller, ProductType::Package , 20094, 5, time_now + Duration::seconds(12))]
    ).await;
    Ok(())
} // end of fn replica_update_order_payment


async fn itest_setup_get_order_refund( shrstate: AppSharedState, oid:String,
    t_start: DateTime<FixedOffset>, t_end: DateTime<FixedOffset>
) -> JsnVal
{
    const FPATH_REP_REFUND_TEMPLATE:&str = "/tests/integration/examples/replica_refund_template.json";
    let mock_rpc_topic = "rpc.order.order_returned_replica_refund";
    let cfg = shrstate.config().clone();
    let req = {
        let result = deserialize_json_template::<JsnVal>(
            &cfg.basepath, FPATH_REP_REFUND_TEMPLATE);
        let mut req_body_template = result.unwrap();
        let obj = req_body_template.as_object_mut().unwrap();
        obj.insert("order_id".to_string(), JsnVal::String(oid)) ;
        obj.insert("start".to_string(), JsnVal::String(t_start.to_rfc3339()));
        obj.insert("end".to_string(),   JsnVal::String(t_end.to_rfc3339()));
        let msgbody = req_body_template.to_string().into_bytes();
        AppRpcClientReqProperty { start_time: Local::now().fixed_offset(),
            msgbody, route: mock_rpc_topic.to_string() }
    };
    let result = rpc::route_to_handler(req, shrstate).await;
    assert!(result.is_ok());
    let raw = result.unwrap();
    let result = serde_json::from_slice(&raw);
    assert!(result.is_ok());
    // println!("[debug] itest-setup-get-order-refund , raw : {:?}",
    //          String::from_utf8(raw).unwrap() );
    result.unwrap()
} // end of fn itest_setup_get_order_refund

fn itest_verify_order_refund(
    actual:JsnVal, expect_lines:Vec<(u32, ProductType, u64, u32, u32)>
) {
    let olines = actual.as_array().unwrap();
    assert_eq!(olines.len(), expect_lines.len());
    expect_lines.into_iter().map(|expect| {
        let actual_line = olines.iter().find_map(|oline| {
            let map = oline.as_object().unwrap();
            let store_id = {
                let s = map.get("seller_id").unwrap().as_u64().unwrap();
                u32::try_from(s).unwrap()
            };
            let product_id   = map.get("product_id").unwrap().as_u64().unwrap();
            let product_type = {
                let p = map.get("product_type").unwrap().as_u64().unwrap();
                ProductType::from(p as u8)
            };
            let cond = (store_id == expect.0) && (product_id == expect.2)
                && (product_type == expect.1);
            if cond {Some(map)} else {None}
        }).unwrap();
        let (amount_unit, amount_total) = {
            let _amount = actual_line.get("amount").unwrap().as_object().unwrap();
            let _unit  = _amount.get("unit").unwrap().as_u64().unwrap();
            let _total = _amount.get("total").unwrap().as_u64().unwrap();
            (u32::try_from(_unit).unwrap(), u32::try_from(_total).unwrap())
        };
        assert_eq!(amount_unit,  expect.3);
        assert_eq!(amount_total, expect.4);
    }).count();
} // end of fn itest_verify_order_refund

#[tokio::test]
async fn  replica_order_refund() -> DefaultResult<(), AppError>
{
    use tokio::time::sleep;
    const FPATH_EDIT_PRODUCTPOLICY:&str = "/tests/integration/examples/policy_product_edit_ok_6.json";
    const FPATH_EDIT_PRODUCTPRICE:&str = "/tests/integration/examples/product_price_celery_ok_9.json";
    const FPATH_EDIT_STOCK_LVL:&str  = "/tests/integration/examples/stock_level_edit_ok_7.json";
    const FPATH_NEW_ORDER:&str  = "/tests/integration/examples/order_new_ok_5.json";
    const FPATH_RETURN_OLINE_REQ:[&str;2]  = [
        "/tests/integration/examples/oline_return_request_ok_2.json",
        "/tests/integration/examples/oline_return_request_ok_3.json",
    ];
    let shrstate = test_setup_shr_state()?;
    let srv = TestWebServer::setup(shrstate.clone());
    let time_now = Local::now().fixed_offset();
    let (mock_authed_usr, mock_seller) = (188, 543);
    let authed_claim = setup_mock_authed_claim(mock_authed_usr);
    let oid = itest_setup_create_order( shrstate.clone(), srv.clone(), time_now,
        &authed_claim,  FPATH_EDIT_PRODUCTPOLICY, FPATH_EDIT_PRODUCTPRICE, 
        FPATH_EDIT_STOCK_LVL, FPATH_NEW_ORDER
    ).await;
    itest_update_payment_status(  shrstate.clone() , oid.clone() , 
        vec![(mock_seller, ProductType::Item, 20097, 11, time_now + Duration::seconds(2)),
             (mock_seller, ProductType::Item, 20095, 16, time_now + Duration::seconds(2)),
             (mock_seller, ProductType::Package, 20096, 14, time_now + Duration::seconds(2)) ]
    ).await;
    sleep(std::time::Duration::from_secs(2)).await ;
    let _resp_body = itest_return_olines_request(
        shrstate.config().clone(), srv.clone(), FPATH_RETURN_OLINE_REQ[0],
        oid.as_str(), itest_clone_authed_claim(&authed_claim),  StatusCode::OK
    ).await ;
    let resp_body = itest_setup_get_order_refund(
        shrstate.clone(), oid.clone(),  time_now - Duration::seconds(2),
        time_now + Duration::seconds(3)
    ).await;
    itest_verify_order_refund(resp_body,
        vec![(mock_seller, ProductType::Item,    20095, 99, 99 * 4),
             (mock_seller, ProductType::Package, 20096, 349, 349 * 2),
             (mock_seller, ProductType::Item,    20097, 299, 299 * 3) ]
    );
    sleep(std::time::Duration::from_secs(1 + limit::MIN_SECS_INTVL_REQ as u64)).await ;
    let _resp_body = itest_return_olines_request(
        shrstate.config().clone(), srv.clone(), FPATH_RETURN_OLINE_REQ[1],
        oid.as_str(), authed_claim,  StatusCode::OK
    ).await ;
    let resp_body = itest_setup_get_order_refund(shrstate, oid,
            time_now + Duration::seconds(3),
            time_now + Duration::seconds(3i64 + 1 + limit::MIN_SECS_INTVL_REQ as i64)
    ).await;
    itest_verify_order_refund(resp_body,
        vec![(mock_seller, ProductType::Item, 20095, 99, 99 * 1),
             (mock_seller, ProductType::Item, 20097, 299, 299 * 1)]
    );
    Ok(())
} // end of fn replica_order_refund


async fn itest_setup_get_order_inventory( shrstate: AppSharedState,
    t_start: DateTime<FixedOffset>, t_end: DateTime<FixedOffset>
) -> JsnVal
{
    const FPATH_REP_INVENTORY_TEMPLATE:&str = "/tests/integration/examples/replica_inventory_template.json";
    let mock_rpc_topic = "rpc.order.order_reserved_replica_inventory";
    let cfg = shrstate.config().clone();
    let req = {
        let result = deserialize_json_template::<JsnVal>(
            &cfg.basepath, FPATH_REP_INVENTORY_TEMPLATE);
        let mut req_body_template = result.unwrap();
        let obj = req_body_template.as_object_mut().unwrap();
        obj.insert("start".to_string(), JsnVal::String(t_start.to_rfc3339()));
        obj.insert("end".to_string(),   JsnVal::String(t_end.to_rfc3339()));
        let msgbody = req_body_template.to_string().into_bytes();
        AppRpcClientReqProperty { start_time: Local::now().fixed_offset(),
            msgbody, route: mock_rpc_topic.to_string() }
    };
    let result = rpc::route_to_handler(req, shrstate).await;
    assert!(result.is_ok());
    let raw = result.unwrap();
    let result = serde_json::from_slice(&raw);
    assert!(result.is_ok());
    println!("[debug] itest-setup-get-order-inventory , raw : {:?}",
             String::from_utf8(raw).unwrap() );
    result.unwrap()
} // end of fn itest_setup_get_order_inventory

fn itest_verify_rsv_inventory(
    actual:&Vec<JsnVal>, expect_oid:&str, expect_rsv: Vec<(u32, ProductType, u64, u32)>
) {
    let olines = actual.iter().find_map(|order| {
        let o_map = order.as_object().unwrap();
        let oid = o_map.get("oid").unwrap().as_str().unwrap();
        if oid == expect_oid {
            let lines = o_map.get("lines").unwrap().as_array().unwrap();
            Some(lines)
        } else {None}
    }).unwrap();
    expect_rsv.into_iter().map(|expect| {
        let actual_line = olines.iter().find_map(|oline| {
            let map = oline.as_object().unwrap();
            let store_id = {
                let s = map.get("seller_id").unwrap().as_u64().unwrap();
                u32::try_from(s).unwrap()
            };
            let product_id   = map.get("product_id").unwrap().as_u64().unwrap();
            let product_type = {
                let p = map.get("product_type").unwrap().as_u64().unwrap();
                ProductType::from(p as u8)
            };
            let cond = (store_id == expect.0) && (product_id == expect.2)
                && (product_type == expect.1);
            if cond {Some(map)} else {None}
        }).unwrap();
        let qty = {
            let q = actual_line.get("qty").unwrap().as_u64().unwrap();
            u32::try_from(q).unwrap()
        };
        assert_eq!(qty, expect.3);
    }).count();
} // end of fn itest_verify_rsv_inventory

#[tokio::test]
async fn  replica_order_rsv_inventory() -> DefaultResult<(), AppError>
{
    use tokio::time::sleep;
    const FPATH_EDIT_PRODUCTPOLICY:&str = "/tests/integration/examples/policy_product_edit_ok_7.json";
    const FPATH_EDIT_PRODUCTPRICE:&str = "/tests/integration/examples/product_price_celery_ok_10.json";
    const FPATH_EDIT_STOCK_LVL:&str  = "/tests/integration/examples/stock_level_edit_ok_8.json";
    const FPATH_NEW_ORDER:&str  = "/tests/integration/examples/order_new_ok_6.json";
    const FPATH_RETURN_OLINE_REQ:&str = "/tests/integration/examples/oline_return_request_ok_4.json";
    let shrstate = test_setup_shr_state()?;
    let srv = TestWebServer::setup(shrstate.clone());
    let time_now = Local::now().fixed_offset();
    let (mock_authed_usr, mock_seller) = (192, 545);
    let authed_claim = setup_mock_authed_claim(mock_authed_usr);
    let oid = itest_setup_create_order( shrstate.clone(), srv.clone(), time_now,
        &authed_claim,  FPATH_EDIT_PRODUCTPOLICY, FPATH_EDIT_PRODUCTPRICE, 
        FPATH_EDIT_STOCK_LVL, FPATH_NEW_ORDER
    ).await;
    {
        let resp_body = itest_setup_get_order_inventory( shrstate.clone(),
            time_now - Duration::seconds(1), time_now + Duration::seconds(2)
        ).await;
        let toplvl_map = resp_body.as_object().unwrap();
        let rsv_all = toplvl_map.get("reservations").unwrap().as_array().unwrap();
        itest_verify_rsv_inventory(rsv_all, oid.as_str(),
            vec![(mock_seller, ProductType::Package, 20101, 17),
                 (mock_seller, ProductType::Item,    20100, 16),
                 (mock_seller, ProductType::Package, 20099, 15) ]
        );
    }
    sleep(std::time::Duration::from_secs(2u64)).await ;
    let _resp_body = itest_return_olines_request(
        shrstate.config().clone(), srv, FPATH_RETURN_OLINE_REQ, oid.as_str(),
        authed_claim,  StatusCode::OK
    ).await ;
    { // Note, since all the test cases run asynchronously, the time range might be large
        let resp_body = itest_setup_get_order_inventory( shrstate.clone(),
            time_now - Duration::seconds(10), time_now + Duration::seconds(10)
        ).await;
        let toplvl_map = resp_body.as_object().unwrap();
        let returns = toplvl_map.get("returns").unwrap().as_array().unwrap();
        itest_verify_rsv_inventory(returns, oid.as_str(),
            vec![(mock_seller, ProductType::Package, 20101, 4),
                 (mock_seller, ProductType::Item,    20100, 6) ]
        );
    }
    Ok(())
} // end of fn replica_order_rsv_inventory
