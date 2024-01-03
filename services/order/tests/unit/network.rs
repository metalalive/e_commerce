use std::env;
use std::collections::HashMap;
use std::io::ErrorKind;

use order::error::AppErrorCode;
use serde::{Deserialize, Serialize};
use http_body::{Body, Limited};
use hyper::{Body as HyperBody, Request};
use tower::{Service, ServiceBuilder};
use axum::routing;
use axum::response::IntoResponse;
use axum::extract::{Json as ExtractJson, State as ExtractState};
use axum::http::{
    StatusCode as HttpStatusCode,
    HeaderMap as HttpHeaderMap,
    HeaderValue as HttpHeaderValue,
    header as HttpHeader
};

use order::AppSharedState;
use order::api::web::ApiRouteTableType;
use order::constant::ENV_VAR_SERVICE_BASE_PATH;
use order::logging::{AppLogLevel, app_log_event};
use order::network::{middleware, app_web_service, net_server_listener};
use crate::{ut_setup_share_state, EXAMPLE_REL_PATH, MockConfidential};



#[derive(Deserialize, Serialize)]
struct UTendpointData
{
    gram: u8,
}

async fn ut_endpoint_handler(
    ExtractState(appstate): ExtractState<AppSharedState>,
    ExtractJson(mut req_body): ExtractJson<UTendpointData> ) -> impl IntoResponse
{
    let logctx = appstate.log_context().clone();
    app_log_event!(logctx, AppLogLevel::INFO, "ut_endpoint_handler reached");
    req_body.gram += 1;
    let resp_ctype_val = HttpHeaderValue::from_str("application/json").unwrap();
    let mut hdr_map = HttpHeaderMap::new();
    hdr_map.insert(HttpHeader::CONTENT_TYPE, resp_ctype_val);
    let serial_resp_body = serde_json::to_string(&req_body).unwrap();
    (HttpStatusCode::OK, hdr_map, serial_resp_body)
}


fn ut_service_req_setup() -> Request<HyperBody>
{
    let body = {
        let d = UTendpointData { gram: 76};
        let d = serde_json::to_string(&d).unwrap();
        HyperBody::from(d)
    };
    let req = Request::post("/1.0.33/gram/increment")
        .header("content-type", "application/json")
        .body(body).unwrap();
    req
}

#[tokio::test]
async fn app_web_service_ok() {
    type UTestHttpBody = HyperBody;
    let shr_state = ut_setup_share_state("config_ok.json", Box::new(MockConfidential{}));
    let cfg = shr_state.config().clone();
    let rtable:ApiRouteTableType<UTestHttpBody> = HashMap::from([
        ("gram_increment", routing::post(ut_endpoint_handler)),
    ]);
    let (mut service, num_routes) = app_web_service::<UTestHttpBody>(
        &cfg.api_server.listen, rtable, shr_state);
    assert_eq!(num_routes, 1);
    let req = ut_service_req_setup();
    let result = service.call(req).await;
    assert!(result.is_ok());
    if let Ok(mut r) = result {
        assert_eq!(r.status(), HttpStatusCode::OK);
        let rawdata = r.body_mut().data().await;
        let rawdata = rawdata.unwrap().unwrap().to_vec();
        let data = serde_json::from_slice::<UTendpointData>(rawdata.as_slice()).unwrap();
        assert_eq!(data.gram, 77);
    }
} // end of fn app_web_service_ok

#[tokio::test]
async fn net_server_listener_ok() {
    let result = net_server_listener("localhost".to_string(), 8086);
    assert!(result.is_ok());
    let result = net_server_listener("localhost".to_string(), 8086);
    assert!(result.is_err());
    if let Err(e) = result {
        assert_eq!(e.code, AppErrorCode::IOerror(ErrorKind::AddrInUse));
    }
    let result = net_server_listener("localhost".to_string(), 65535);
    assert!(result.is_ok());
    let result = net_server_listener("nonexist.org.12345".to_string(), 0);
    assert!(result.is_err());
    if let Err(e) = result {
        assert_eq!(e.code, AppErrorCode::IOerror(ErrorKind::AddrNotAvailable));
    }
}

#[test]
fn middleware_cors_ok() {
    let service_basepath = env::var(ENV_VAR_SERVICE_BASE_PATH).unwrap(); 
    let cfg_path = service_basepath + EXAMPLE_REL_PATH + "cors_ok.json";
    let result = middleware::cors(cfg_path);
    assert!(result.is_ok());
}

#[test]
fn middleware_cors_error_cfg() {
    let service_basepath = env::var(ENV_VAR_SERVICE_BASE_PATH).unwrap(); 
    let cfg_path = service_basepath + EXAMPLE_REL_PATH + "cors_invalid_header.json";
    let result = middleware::cors(cfg_path);
    assert!(result.is_err());
    if let Err(e) = result {
        assert_eq!(e.code, AppErrorCode::InvalidInput);
    }
}

#[tokio::test]
async fn middleware_req_body_limit() {
    type UTestHttpBody = Limited<HyperBody>;
    let shr_state = ut_setup_share_state("config_ok.json", Box::new(MockConfidential{}));
    let cfg = shr_state.config().clone();
    let rtable:ApiRouteTableType<UTestHttpBody> = HashMap::from([
        ("gram_increment", routing::post(ut_endpoint_handler)),
    ]);
    let (service, num_routes) = app_web_service::<UTestHttpBody>(
        &cfg.api_server.listen, rtable, shr_state);
    assert_eq!(num_routes, 1);
    let req = ut_service_req_setup();
    let reqlm = middleware::req_body_limit(2);
    let middlewares1 = ServiceBuilder::new().layer(reqlm);
    let mut service = service.layer(middlewares1);
    let result = service.call(req).await;
    assert!(result.is_ok());
    if let Ok(r) = result {
        assert_eq!(r.status(), HttpStatusCode::PAYLOAD_TOO_LARGE);
    }
}
