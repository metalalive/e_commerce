mod common;

use std::fs::File;

use actix_web::http::header::ContentType;
use actix_web::test::{call_service, TestRequest};
use serde_json::Value as JsnVal;

use common::itest_setup_app_server;

#[actix_web::test]
async fn charge_ok() {
    let mock_app = itest_setup_app_server().await;

    const CASE_FILE: &str = "./tests/integration/examples/create_charge_stripe_ok.json";
    let req = {
        let rdr = File::open(CASE_FILE).unwrap();
        let req_body = serde_json::from_reader::<File, JsnVal>(rdr).unwrap();
        TestRequest::post()
            .uri("/v0.0.2/charge")
            .append_header(ContentType::json())
            .set_json(req_body)
            .to_request()
    };
    let resp = call_service(&mock_app, req).await;
    assert_eq!(resp.status().as_u16(), 202);

    let req = {
        TestRequest::patch()
            .uri("/v0.0.2/charge/127-203948892-2903")
            .append_header(ContentType::json())
            .to_request()
    };
    let resp = call_service(&mock_app, req).await;
    assert_eq!(resp.status().as_u16(), 200);
} // end of fn charge_ok
