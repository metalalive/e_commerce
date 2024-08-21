mod common;

use std::fs::File;
use std::thread::sleep;
use std::time::Duration;

use actix_http::HttpMessage;
use actix_http::Request;
use actix_web::body::MessageBody;
use actix_web::dev::{Service, ServiceResponse};
use actix_web::error::Error as WebError;
use actix_web::http::header::ContentType;
use actix_web::http::Method;
use actix_web::test::{call_service, TestRequest};
use actix_web::web::Bytes as ActixBytes;
use serde_json::Value as JsnVal;

use payment::hard_limit::CREATE_CHARGE_SECONDS_INTERVAL;
use payment::AppAuthedClaim;

use common::{itest_setup_app_server, itest_setup_auth_claim};

const CASES_PATH: &str = "./tests/integration/examples/";
const API_VERSION: &str = "v0.0.5";

async fn itest_onboard_merchant(
    app: &ItestService!(),
    case_file: &str,
    store_id: u32,
    usr_id: u32,
    expect_resp_status: u16,
) -> Result<ActixBytes, impl MessageBody> {
    let uri = format!("/{API_VERSION}/store/{store_id}/onboard");
    let req_body = {
        let path = CASES_PATH.to_string() + case_file;
        let rdr = File::open(path).unwrap();
        serde_json::from_reader::<File, JsnVal>(rdr).unwrap()
    };
    let req = TestRequest::with_uri(uri.as_str())
        .method(Method::POST)
        .append_header(ContentType::json())
        .set_json(req_body)
        .to_request();
    let _empty = req
        .extensions_mut()
        .insert::<AppAuthedClaim>(itest_setup_auth_claim(usr_id));
    let resp = call_service(app, req).await;
    assert_eq!(resp.status().as_u16(), expect_resp_status);
    let body_ctx = resp.into_body();
    body_ctx.try_into_bytes()
}

async fn itest_track_onboarding_status(
    app: &ItestService!(),
    case_file: &str,
    store_id: u32,
    usr_id: u32,
    expect_resp_status: u16,
) -> Result<ActixBytes, impl MessageBody> {
    let uri = format!("/{API_VERSION}/store/{store_id}/onboard/status");
    let req_body = {
        let path = CASES_PATH.to_string() + case_file;
        let rdr = File::open(path).unwrap();
        serde_json::from_reader::<File, JsnVal>(rdr).unwrap()
    };
    let req = TestRequest::with_uri(uri.as_str())
        .method(Method::PATCH)
        .append_header(ContentType::json())
        .set_json(req_body)
        .to_request();
    let _empty = req
        .extensions_mut()
        .insert::<AppAuthedClaim>(itest_setup_auth_claim(usr_id));
    let resp = call_service(app, req).await;
    assert_eq!(resp.status().as_u16(), expect_resp_status);
    let body_ctx = resp.into_body();
    body_ctx.try_into_bytes()
}

async fn itest_create_charge(
    app: &ItestService!(),
    case_file: &str,
    usr_id: u32,
    expect_resp_status: u16,
) -> Result<ActixBytes, impl MessageBody> {
    let path = CASES_PATH.to_string() + case_file;
    let rdr = File::open(path).unwrap();
    let req_body = serde_json::from_reader::<File, JsnVal>(rdr).unwrap();
    let uri = format!("/{API_VERSION}/charge");
    let req = TestRequest::post()
        .uri(uri.as_str())
        .append_header(ContentType::json())
        .set_json(req_body)
        .to_request();
    let _empty = req
        .extensions_mut()
        .insert::<AppAuthedClaim>(itest_setup_auth_claim(usr_id));
    let resp = call_service(app, req).await;
    assert_eq!(resp.status().as_u16(), expect_resp_status);
    let body_ctx = resp.into_body();
    body_ctx.try_into_bytes()
}

async fn itest_refresh_charge_status(
    app: &ItestService!(),
    charge_id: &str,
    usr_id: u32,
    expect_resp_status: u16,
) -> Result<ActixBytes, impl MessageBody> {
    assert!(!charge_id.is_empty());
    let uri = format!("/{API_VERSION}/charge/{charge_id}");
    let req = TestRequest::patch()
        .uri(uri.as_str())
        .append_header(ContentType::json())
        .to_request();
    let _empty = req
        .extensions_mut()
        .insert::<AppAuthedClaim>(itest_setup_auth_claim(usr_id));
    let resp = call_service(app, req).await;
    assert_eq!(resp.status().as_u16(), expect_resp_status);
    let body_ctx = resp.into_body();
    body_ctx.try_into_bytes()
}

async fn itest_capture_charge_payout(
    app: &ItestService!(),
    case_file: &str,
    charge_id: &str,
    usr_id: u32,
    expect_resp_status: u16,
) -> Result<ActixBytes, impl MessageBody> {
    let uri = format!("/{API_VERSION}/charge/{charge_id}/capture");
    let req_body = {
        let path = CASES_PATH.to_string() + case_file;
        let rdr = File::open(path).unwrap();
        serde_json::from_reader::<File, JsnVal>(rdr).unwrap()
    };
    let req = TestRequest::with_uri(uri.as_str())
        .method(Method::POST)
        .append_header(ContentType::json())
        .set_json(req_body)
        .to_request();
    let _empty = req
        .extensions_mut()
        .insert::<AppAuthedClaim>(itest_setup_auth_claim(usr_id));
    let resp = call_service(app, req).await;
    assert_eq!(resp.status().as_u16(), expect_resp_status);
    let body_ctx = resp.into_body();
    body_ctx.try_into_bytes()
}

#[rustfmt::skip]
fn verify_newcharge_stripe (resp_body: &JsnVal) {
    let mthd_obj = resp_body.as_object().unwrap()
        .get("method").unwrap();
    let actual_method = mthd_obj.get("label").unwrap()
        .as_str().unwrap();
    assert_eq!(actual_method, "Stripe");
    let cond = mthd_obj.get("id").unwrap().is_string();
    assert!(cond);
    let cond = mthd_obj.get("redirect_url").unwrap().is_string();
    assert!(cond);
    let cond = mthd_obj.get("client_session").unwrap().is_null();
    assert!(cond);
}
#[rustfmt::skip]
fn extract_charge_id (resp_body: &JsnVal) -> String {
    resp_body.as_object().unwrap()
        .get("id").unwrap()
        .as_str().unwrap()
        .to_string()
}

#[rustfmt::skip]
fn verify_charge_status(resp_body: &JsnVal, expect_oid: &str, expect_progress: &str) {
    let actual_oid = resp_body
        .as_object().unwrap()
        .get("order_id").unwrap()
        .as_str().unwrap();
    assert_eq!(actual_oid, expect_oid);
    let actual_progress = resp_body
        .as_object().unwrap()
        .get("status").unwrap()
        .as_str().unwrap();
    assert_eq!(actual_progress, expect_progress);
}

#[actix_web::test]
async fn charge_stripe_ok() {
    let mock_app = itest_setup_app_server().await;
    let (buyer_usr_id, seller_usr_id) = (2234u32, 6741u32);
    let mock_store_id = 983u32;
    let expect_oid = "7931649be98f24";
    let result = itest_onboard_merchant(
        &mock_app,
        "onboard_merchant_stripe_ok_1.json",
        mock_store_id,
        seller_usr_id,
        202,
    )
    .await;
    assert!(result.is_ok());
    let result = itest_track_onboarding_status(
        &mock_app,
        "track_onboarding_status_stripe_ok_1.json",
        mock_store_id,
        seller_usr_id,
        200,
    )
    .await;
    assert!(result.is_ok());

    let result = itest_create_charge(
        &mock_app,
        "create_charge_stripe_ok_1.json",
        buyer_usr_id,
        202,
    )
    .await;
    assert!(result.is_ok());
    let charge_id = result
        .map(|body_raw| {
            // println!("[debug] recvied response body : {:?} ", body_raw);
            let actual_body = serde_json::from_slice::<JsnVal>(body_raw.as_ref()).unwrap();
            verify_newcharge_stripe(&actual_body);
            extract_charge_id(&actual_body)
        })
        .map_err(|_e| ())
        .unwrap();

    let result =
        itest_refresh_charge_status(&mock_app, charge_id.as_str(), buyer_usr_id, 200).await;
    assert!(result.is_ok());
    if let Ok(body_raw) = result {
        let actual_body = serde_json::from_slice::<JsnVal>(body_raw.as_ref()).unwrap();
        verify_charge_status(&actual_body, expect_oid, "Completed");
    }

    // ---- start another charge event of the same order
    sleep(Duration::from_secs(CREATE_CHARGE_SECONDS_INTERVAL as u64));
    let result = itest_create_charge(
        &mock_app,
        "create_charge_stripe_ok_2.json",
        buyer_usr_id,
        202,
    )
    .await;
    assert!(result.is_ok());
    let charge_id = result
        .map(|body_raw| {
            let actual_body = serde_json::from_slice::<JsnVal>(body_raw.as_ref()).unwrap();
            extract_charge_id(&actual_body)
        })
        .map_err(|_e| ())
        .unwrap();
    let result =
        itest_refresh_charge_status(&mock_app, charge_id.as_str(), buyer_usr_id, 200).await;
    assert!(result.is_ok());
    if let Ok(body_raw) = result {
        let actual_body = serde_json::from_slice::<JsnVal>(body_raw.as_ref()).unwrap();
        verify_charge_status(&actual_body, expect_oid, "Completed");
    }

    // --- caoture charge confirmed and authorised from buyer
    let result = itest_capture_charge_payout(
        &mock_app,
        "capture_authed_charge_ok_1.json",
        charge_id.as_str(),
        seller_usr_id,
        200,
    )
    .await;
    assert!(result.is_ok());
} // end of fn charge_stripe_ok
