use std::result::Result as DefaultResult ;

use hyper::Body as HyperBody;
use http::{Request, StatusCode};

use order::error::AppError;
use order::api::web::model::{OrderCreateReqData, OrderCreateRespAsyncData, OrderEditReqData, ProductPolicyData};

mod common;
use common::{test_setup_shr_state, TestWebServer, deserialize_json_template};

const FPATH_NEW_ORDER_OK_1:&'static str  = "/tests/integration/examples/order_new_ok_1.json";
const FPATH_EDIT_ORDER_OK_1:&'static str = "/tests/integration/examples/order_edit_ok_1.json";
const FPATH_EDIT_PRODUCTPOLICY_OK_1:&'static str = "/tests/integration/examples/policy_product_edit_ok_1.json";


#[tokio::test]
async fn place_new_order_ok() -> DefaultResult<(), AppError>
{
    let shr_state = test_setup_shr_state() ? ;
    let srv = TestWebServer::setup(shr_state.clone());
    let top_lvl_cfg = shr_state.config();
    let listener = &top_lvl_cfg.api_server.listen;
    let reqbody = {
        let rb = deserialize_json_template::<OrderCreateReqData>
            (&top_lvl_cfg.basepath, FPATH_NEW_ORDER_OK_1) ? ;
        let rb = serde_json::to_string(&rb) .unwrap();
        HyperBody::from(rb)
    };
    let uri = format!("/{}/order", listener.api_version);
    let req = Request::builder().uri(uri).method("POST")
        .header("content-type", "application/json")
        .header("accept", "application/json")
        .body(reqbody)
        .unwrap();

    let mut response = TestWebServer::consume(&srv, req).await;
    assert_eq!(response.status(), StatusCode::ACCEPTED);
    let actual = TestWebServer::to_custom_type::<OrderCreateRespAsyncData>
        (response.body_mut())  .await  ? ;
    assert_eq!(actual.order_id.is_empty() ,  false);
    assert_eq!(actual.async_stock_chk.is_empty() ,  false);
    assert_eq!(actual.reserved_lines.is_empty() ,  false);
    Ok(())
} // end of place_new_order_ok


#[tokio::test]
async fn edit_order_contact_ok() -> DefaultResult<(), AppError>
{
    let shr_state = test_setup_shr_state() ? ;
    let srv = TestWebServer::setup(shr_state.clone());
    let top_lvl_cfg = shr_state.config();
    let reqbody = {
        let mut rb = deserialize_json_template::<OrderEditReqData>
            (&top_lvl_cfg.basepath, FPATH_EDIT_ORDER_OK_1) ? ;
        use std::fmt::Write;
        rb.billing.contact.first_name.clear();
        rb.billing.contact.first_name.write_str("Satunam") .unwrap();
        let rb = serde_json::to_string(&rb).unwrap();
        HyperBody::from(rb)
    };
    let uri = format!("/{ver}/order/{oid}", oid = "8dj30Hr",
                      ver = top_lvl_cfg.api_server.listen.api_version);
    let req = Request::builder().uri(uri).method("PATCH")
        .header("content-type", "application/json")
        .body(reqbody)
        .unwrap();

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
    let reqbody = {
        let mut rb = deserialize_json_template::<Vec<ProductPolicyData>>
            (&top_lvl_cfg.basepath, FPATH_EDIT_PRODUCTPOLICY_OK_1) ? ;
        assert!(rb.len() > 0);
        let item = rb.get_mut(0).unwrap();
        item.warranty_hours = 2345;
        let rb = serde_json::to_string(&rb).unwrap();
        HyperBody::from(rb)
    };
    let uri = format!("/{}/policy/products", top_lvl_cfg.api_server.listen.api_version);
    let req = Request::builder().uri(uri).method("POST")
        .header("content-type", "application/json")
        .body(reqbody)
        .unwrap();
    let response = TestWebServer::consume(&srv, req).await;
    assert_eq!(response.status(), StatusCode::OK);
    Ok(())
}


