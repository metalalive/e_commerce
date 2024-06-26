mod common;

use std::fs::File;

use actix_http::HttpMessage;
use actix_web::http::header::ContentType;
use actix_web::test::{call_service, TestRequest};
use payment::AppAuthedClaim;
use serde_json::Value as JsnVal;

use common::{itest_setup_app_server, itest_setup_auth_claim};

#[actix_web::test]
async fn charge_ok() {
    let mock_app = itest_setup_app_server().await;
    let mock_usr_id = 2234u32;
    const CASE_FILE: &str = "./tests/integration/examples/create_charge_stripe_ok.json";
    let req = {
        let rdr = File::open(CASE_FILE).unwrap();
        let req_body = serde_json::from_reader::<File, JsnVal>(rdr).unwrap();
        let r = TestRequest::post()
            .uri("/v0.0.2/charge")
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

    let req = {
        let r = TestRequest::patch()
            .uri("/v0.0.2/charge/127-203948892-2903")
            .append_header(ContentType::json())
            .to_request();
        let _empty = r
            .extensions_mut()
            .insert::<AppAuthedClaim>(itest_setup_auth_claim(mock_usr_id));
        r
    };
    let resp = call_service(&mock_app, req).await;
    assert_eq!(resp.status().as_u16(), 200);
} // end of fn charge_ok
