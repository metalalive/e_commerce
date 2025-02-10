mod common;

use std::fs::File;
use std::thread::sleep;
use std::time::Duration;

use actix_http::HttpMessage;
use actix_http::Request;
use actix_web::body::MessageBody;
use actix_web::dev::{Service, ServiceResponse};
use actix_web::error::Error as WebError;
use actix_web::http::header::{Accept, ContentType};
use actix_web::http::Method;
use actix_web::test::{call_service, TestRequest};
use actix_web::web::Bytes as ActixBytes;
use chrono::{Duration as ChronoDuration, Local};
use serde_json::Value as JsnVal;

use payment::adapter::repository::app_repo_refund;
use payment::hard_limit::CREATE_CHARGE_SECONDS_INTERVAL;
use payment::usecase::SyncRefundReqUseCase;
use payment::{
    app_meta, AppAuthClaimPermission, AppAuthClaimQuota, AppAuthPermissionCode,
    AppAuthQuotaMatCode, AppAuthedClaim,
};

use common::{itest_setup_app_server, itest_setup_auth_claim};

const CASES_PATH: &str = "./tests/integration/examples/";
const API_VERSION: &str = "v0.1.0";

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
    let mut authed_claim = itest_setup_auth_claim(usr_id);
    authed_claim.perms.push(AppAuthClaimPermission {
        app_code: app_meta::RESOURCE_QUOTA_AP_CODE,
        codename: AppAuthPermissionCode::can_onboard_merchant,
    });
    let _empty = req.extensions_mut().insert::<AppAuthedClaim>(authed_claim);
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
    let mut authed_claim = itest_setup_auth_claim(usr_id);
    authed_claim.perms.push(AppAuthClaimPermission {
        app_code: app_meta::RESOURCE_QUOTA_AP_CODE,
        codename: AppAuthPermissionCode::can_onboard_merchant,
    });
    let _empty = req.extensions_mut().insert::<AppAuthedClaim>(authed_claim);
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
    let mut authed_claim = itest_setup_auth_claim(usr_id);
    authed_claim.perms.push(AppAuthClaimPermission {
        app_code: app_meta::RESOURCE_QUOTA_AP_CODE,
        codename: AppAuthPermissionCode::can_create_charge,
    });
    authed_claim.quota.push(AppAuthClaimQuota {
        app_code: app_meta::RESOURCE_QUOTA_AP_CODE,
        mat_code: AppAuthQuotaMatCode::NumChargesPerOrder,
        maxnum: 5,
    });
    let _empty = req.extensions_mut().insert::<AppAuthedClaim>(authed_claim);
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
    let mut authed_claim = itest_setup_auth_claim(usr_id);
    authed_claim.perms.push(AppAuthClaimPermission {
        app_code: app_meta::RESOURCE_QUOTA_AP_CODE,
        codename: AppAuthPermissionCode::can_update_charge_progress,
    });
    let _empty = req.extensions_mut().insert::<AppAuthedClaim>(authed_claim);
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
    let mut authed_claim = itest_setup_auth_claim(usr_id);
    authed_claim.perms.push(AppAuthClaimPermission {
        app_code: app_meta::RESOURCE_QUOTA_AP_CODE,
        codename: AppAuthPermissionCode::can_capture_charge,
    });
    let _empty = req.extensions_mut().insert::<AppAuthedClaim>(authed_claim);
    let resp = call_service(app, req).await;
    assert_eq!(resp.status().as_u16(), expect_resp_status);
    let body_ctx = resp.into_body();
    body_ctx.try_into_bytes()
}

async fn itest_merchant_complete_refund(
    app: &ItestService!(),
    case_file: &str,
    order_id: &str,
    store_id: u32,
    usr_id: u32,
    expect_resp_status: u16,
) -> Result<ActixBytes, impl MessageBody> {
    let uri = format!("/{API_VERSION}/refund/{order_id}/complete/{store_id}");
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
    let mut authed_claim = itest_setup_auth_claim(usr_id);
    authed_claim.perms.push(AppAuthClaimPermission {
        app_code: app_meta::RESOURCE_QUOTA_AP_CODE,
        codename: AppAuthPermissionCode::can_finalize_refund,
    });
    let _empty = req.extensions_mut().insert::<AppAuthedClaim>(authed_claim);
    let resp = call_service(app, req).await;
    assert_eq!(resp.status().as_u16(), expect_resp_status);
    let body_ctx = resp.into_body();
    body_ctx.try_into_bytes()
}

async fn itest_merchant_report_chargelines(
    app: &ItestService!(),
    store_id: u32,
    usr_id: u32,
    expect_resp_status: u16,
) -> Result<ActixBytes, impl MessageBody> {
    let time_base = Local::now().to_utc();
    let t0 = (time_base - ChronoDuration::hours(2))
        .format("%Y-%m-%d-%H")
        .to_string();
    let t1 = (time_base + ChronoDuration::hours(2))
        .format("%Y-%m-%d-%H")
        .to_string();
    let uri =
        format!("/{API_VERSION}/store/{store_id}/order/charges?start_after={t0}&end_before={t1}");
    let req = TestRequest::with_uri(uri.as_str())
        .method(Method::GET)
        .append_header(Accept::json())
        .to_request();
    let authed_claim = itest_setup_auth_claim(usr_id);
    let _empty = req.extensions_mut().insert::<AppAuthedClaim>(authed_claim);
    let resp = call_service(app, req).await;
    let actual_resp_status = resp.status().as_u16();
    let body_ctx = resp.into_body();
    let result = body_ctx.try_into_bytes();
    assert_eq!(actual_resp_status, expect_resp_status);
    result
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
    let (mock_app, mock_shr_state) = itest_setup_app_server().await;
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

    {
        // - mock cron job that sync return request from order app
        let repo_rfd = app_repo_refund(mock_shr_state.datastore()).await.unwrap();
        let rpc_ctx = mock_shr_state.rpc_context();
        let result = SyncRefundReqUseCase::execute(repo_rfd, rpc_ctx).await;
        assert!(result.is_ok());
        let (num_order, num_lines) = result.unwrap();
        assert_eq!(num_order, 3);
        assert_eq!(num_lines, 5);
    }

    // --- partial refund ----
    let result = itest_merchant_complete_refund(
        &mock_app,
        "complete_refund_ok_1.json",
        expect_oid,
        mock_store_id,
        seller_usr_id,
        200,
    )
    .await;
    assert!(result.is_ok());

    // --- expect refund error ----
    let result = itest_merchant_complete_refund(
        &mock_app,
        "complete_refund_ok_1.json",
        "9001848b29", // not-existent order ID
        mock_store_id,
        seller_usr_id,
        404,
    )
    .await;
    assert!(result.is_ok());

    let result =
        itest_merchant_report_chargelines(&mock_app, mock_store_id, seller_usr_id, 200).await;
    assert!(result.is_ok());
    if let Ok(v) = result {
        // println!("[debug] request error : {:?} ", v);
        let body_raw = v.to_vec();
        let report = serde_json::from_slice::<JsnVal>(&body_raw).unwrap();
        let read_merchant_id = report
            .as_object()
            .unwrap()
            .get("merchant_id")
            .unwrap()
            .as_u64()
            .unwrap() as u32;
        assert_eq!(read_merchant_id, mock_store_id);
        let read_lines = report
            .as_object()
            .unwrap()
            .get("lines")
            .unwrap()
            .as_array()
            .unwrap();
        assert_eq!(read_lines.len(), 2);
    }
} // end of fn charge_stripe_ok
