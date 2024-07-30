mod common;

use std::fs::File;

use actix_http::HttpMessage;
use actix_web::body::MessageBody;
use actix_web::http::header::ContentType;
use actix_web::test::{call_service, TestRequest};
use serde_json::Value as JsnVal;

use payment::AppAuthedClaim;

use common::{itest_setup_app_server, itest_setup_auth_claim};

#[actix_web::test]
async fn charge_stripe_ok() {
    let mock_app = itest_setup_app_server().await;
    let mock_usr_id = 2234u32;
    const CASE_FILE: &str = "./tests/integration/examples/create_charge_stripe_ok.json";
    let req = {
        let rdr = File::open(CASE_FILE).unwrap();
        let req_body = serde_json::from_reader::<File, JsnVal>(rdr).unwrap();
        let r = TestRequest::post()
            .uri("/v0.0.4/charge")
            .append_header(ContentType::json())
            .set_json(req_body)
            .to_request();
        let _empty = r
            .extensions_mut()
            .insert::<AppAuthedClaim>(itest_setup_auth_claim(mock_usr_id));
        r
    };
    let resp = call_service(&mock_app, req).await;
    assert_eq!(resp.status().as_u16(), 202);
    let body_ctx = resp.into_body();
    let result = body_ctx.try_into_bytes();
    assert!(result.is_ok());
    let actual_charge_id = if let Ok(body_raw) = result {
        // println!("[debug] recvied response body : {:?} ", body_raw);
        let actual_body = serde_json::from_slice::<JsnVal>(body_raw.as_ref()).unwrap();
        let actual_method = actual_body
            .as_object()
            .unwrap()
            .get("method")
            .unwrap()
            .get("label")
            .unwrap()
            .as_str()
            .unwrap();
        assert_eq!(actual_method, "Stripe");
        actual_body
            .as_object()
            .unwrap()
            .get("id")
            .unwrap()
            .as_str()
            .unwrap()
            .to_string()
    } else {
        String::new()
    };
    assert!(!actual_charge_id.is_empty());

    let req = {
        let url = "/v0.0.4/charge/".to_string() + actual_charge_id.as_str();
        let r = TestRequest::patch()
            .uri(url.as_str())
            .append_header(ContentType::json())
            .to_request();
        let _empty = r
            .extensions_mut()
            .insert::<AppAuthedClaim>(itest_setup_auth_claim(mock_usr_id));
        r
    };
    let resp = call_service(&mock_app, req).await;
    assert_eq!(resp.status().as_u16(), 200);
} // end of fn charge_stripe_ok
