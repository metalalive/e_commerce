use std::collections::HashMap;
use std::env;
use std::io::ErrorKind;
use std::sync::atomic::Ordering;

use axum::body::Body as AxumBody;
use axum::extract::{Json as ExtractJson, State as ExtractState};
use axum::http::{
    header as HttpHeader, HeaderMap as HttpHeaderMap, HeaderValue as HttpHeaderValue,
    StatusCode as HttpStatusCode,
};
use axum::response::IntoResponse;
use axum::routing;
use http_body_util::BodyExt;
use hyper::Request;
use serde::{Deserialize, Serialize};
use tower::{Service, ServiceBuilder};

use ecommerce_common::constant::env_vars::SERVICE_BASEPATH;
use ecommerce_common::error::AppErrorCode;
use ecommerce_common::logging::{app_log_event, AppLogLevel};

use crate::{ut_setup_share_state, MockConfidential, EXAMPLE_REL_PATH};
use order::api::web::ApiRouteTableType;
use order::network::{app_web_service, middleware, net_listener};
use order::AppSharedState;

#[derive(Deserialize, Serialize)]
struct UTendpointData {
    gram: u8,
}

async fn ut_endpoint_handler(
    ExtractState(appstate): ExtractState<AppSharedState>,
    ExtractJson(mut req_body): ExtractJson<UTendpointData>,
) -> impl IntoResponse {
    let logctx = appstate.log_context().clone();
    app_log_event!(logctx, AppLogLevel::INFO, "ut_endpoint_handler reached");
    req_body.gram += 1;
    let resp_ctype_val = HttpHeaderValue::from_str("application/json").unwrap();
    let mut hdr_map = HttpHeaderMap::new();
    hdr_map.insert(HttpHeader::CONTENT_TYPE, resp_ctype_val);
    let serial_resp_body = serde_json::to_string(&req_body).unwrap();
    (HttpStatusCode::OK, hdr_map, serial_resp_body)
}

fn ut_service_req_setup(method: &str, uri: &str) -> Request<AxumBody> {
    let body = {
        let d = UTendpointData { gram: 76 };
        let d = serde_json::to_string(&d).unwrap();
        AxumBody::from(d)
    };
    let req = Request::builder()
        .method(method)
        .uri(uri)
        .header("content-type", "application/json")
        .body(body)
        .unwrap();
    req
}

#[tokio::test]
async fn app_web_service_ok() {
    let shr_state = ut_setup_share_state("config_ok_no_sqldb.json", Box::new(MockConfidential {}));
    let cfg = shr_state.config().clone();
    let rtable: ApiRouteTableType =
        HashMap::from([("gram_increment", routing::post(ut_endpoint_handler))]);
    let (mut service, num_routes) = app_web_service(&cfg.api_server.listen, rtable, shr_state);
    assert_eq!(num_routes, 1);
    let req = ut_service_req_setup("POST", "/1.0.33/gram/increment");
    let result = service.call(req).await;
    assert!(result.is_ok());
    let mut r = result.unwrap();
    assert_eq!(r.status(), HttpStatusCode::OK);
    let rawfrm = r.body_mut().frame().await.unwrap();
    let rawdata = rawfrm.unwrap().into_data().unwrap().to_vec();
    let data = serde_json::from_slice::<UTendpointData>(rawdata.as_slice()).unwrap();
    assert_eq!(data.gram, 77);
} // end of fn app_web_service_ok

#[tokio::test]
#[ignore]
async fn net_server_listener_ok_1() {
    // some platforms seem to allow callers to reuse the same port so the same port
    // binding function can be invoked several times, set this case to `ignore` and
    // let users run this test case in their own local environment.
    let result = net_listener("localhost".to_string(), 8086).await;
    assert!(result.is_ok());
    let result = net_listener("localhost".to_string(), 8086).await;
    assert!(result.is_err());
    if let Err(e) = result {
        assert_eq!(e.code, AppErrorCode::IOerror(ErrorKind::AddrInUse));
    }
}

#[tokio::test]
async fn net_server_listener_ok_2() {
    let result = net_listener("localhost".to_string(), 65535).await;
    assert!(result.is_ok());
    let result = net_listener("nonexist.org.12345".to_string(), 0).await;
    assert!(result.is_err());
    if let Err(e) = result {
        assert_eq!(e.code, AppErrorCode::IOerror(ErrorKind::AddrNotAvailable));
    }
}

#[test]
fn middleware_cors_ok() {
    let service_basepath = env::var(SERVICE_BASEPATH).unwrap();
    let cfg_path = service_basepath + EXAMPLE_REL_PATH + "cors_ok.json";
    let result = middleware::cors(cfg_path);
    assert!(result.is_ok());
}

#[test]
fn middleware_cors_error_cfg() {
    let service_basepath = env::var(SERVICE_BASEPATH).unwrap();
    let cfg_path = service_basepath + EXAMPLE_REL_PATH + "cors_invalid_header.json";
    let result = middleware::cors(cfg_path);
    assert!(result.is_err());
    if let Err(e) = result {
        assert_eq!(e.code, AppErrorCode::InvalidInput);
    }
}

#[tokio::test]
async fn middleware_req_body_limit() {
    let shr_state = ut_setup_share_state("config_ok_no_sqldb.json", Box::new(MockConfidential {}));
    let cfg = shr_state.config().clone();
    let rtable: ApiRouteTableType =
        HashMap::from([("gram_increment", routing::post(ut_endpoint_handler))]);
    let (service, num_routes) = app_web_service(&cfg.api_server.listen, rtable, shr_state);
    assert_eq!(num_routes, 1);
    let req = ut_service_req_setup("POST", "/1.0.33/gram/increment");
    let reqlm = middleware::req_body_limit(2);
    let middlewares = ServiceBuilder::new().layer(reqlm);
    let mut service = service.layer(middlewares);
    let result = service.call(req).await;
    assert!(result.is_ok());
    let r = result.unwrap();
    assert_eq!(r.status(), HttpStatusCode::PAYLOAD_TOO_LARGE);
}

#[tokio::test]
async fn middleware_shutdown_detection() {
    let shr_state = ut_setup_share_state("config_ok_no_sqldb.json", Box::new(MockConfidential {}));
    let cfg = shr_state.config().clone();
    let (mock_flag, mock_num_reqs) = (shr_state.shutdown(), shr_state.num_requests());
    let rtable: ApiRouteTableType =
        HashMap::from([("modify_product_policy", routing::put(ut_endpoint_handler))]);
    let (leaf_service, num_routes) = app_web_service(&cfg.api_server.listen, rtable, shr_state);
    assert_eq!(num_routes, 1);
    let sh_detect =
        middleware::ShutdownDetectionLayer::new(mock_flag.clone(), mock_num_reqs.clone());
    let middlewares = ServiceBuilder::new().layer(sh_detect);
    let mut final_service = leaf_service.layer(middlewares);
    // -------------
    let req = ut_service_req_setup("PUT", "/1.0.33/policy/products");
    let result = final_service.call(req).await;
    assert!(result.is_ok());
    let res = result.unwrap();
    assert_eq!(res.status(), HttpStatusCode::OK);
    let actual_num_reqs = mock_num_reqs.load(Ordering::Relaxed);
    assert_eq!(actual_num_reqs, 0);
    // -------------
    let _ = mock_flag.store(true, Ordering::Relaxed);
    let req = ut_service_req_setup("PUT", "/1.0.33/policy/products");
    let result = final_service.call(req).await;
    let res = result.unwrap();
    assert_eq!(res.status(), HttpStatusCode::SERVICE_UNAVAILABLE);
    let actual_num_reqs = mock_num_reqs.load(Ordering::Relaxed);
    assert_eq!(actual_num_reqs, 0);
} // end of fn middleware_shutdown_detection
