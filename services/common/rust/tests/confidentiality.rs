use std::io::ErrorKind;

use ecommerce_common::confidentiality::{AbstractConfidentiality, UserSpaceConfidentiality};
use ecommerce_common::constant::env_vars::SERVICE_BASEPATH;
use ecommerce_common::error::AppErrorCode;

fn ut_setup() -> (String, &'static str) {
    let app_base_path = std::env::var(SERVICE_BASEPATH).unwrap();
    let secret_lpath = "/tests/examples/confidential_demo.json";
    (app_base_path, secret_lpath)
}

#[test]
fn userspace_access_ok() {
    let (app_base_path, secret_lpath) = ut_setup();
    let fullpath = app_base_path + secret_lpath;
    let hdlr = UserSpaceConfidentiality::build(fullpath);
    // ------------
    let result = hdlr.try_get_payload("amqp_broker/1");
    assert_eq!(result.is_ok(), true);
    let cre = result.unwrap();
    // ------------
    let result = hdlr.try_get_payload("backend_apps/databases/abc_service/PORT");
    assert_eq!(result.is_ok(), true);
    assert_eq!(result.unwrap(), "\"1236\"");
    // ------------
    let result = hdlr.try_get_payload("elasticsearch/nodes/1/port");
    assert_eq!(result.is_ok(), true);
    let port_str = result.unwrap();
    assert_eq!(port_str, "9202");
    let port_num = port_str.parse::<u16>().unwrap();
    assert_eq!(port_num, 9202u16);
    let back: serde_json::Value = serde_json::from_str(port_str.as_str()).unwrap();
    assert_eq!(back.is_number(), true);
    // ------------
    let result = hdlr.try_get_payload("amqp_broker/1");
    assert_eq!(result.is_ok(), true);
    let result = hdlr.try_get_payload("amqp_broker/1");
    assert_eq!(result.is_ok(), true);
    let cre2 = result.unwrap();
    assert!(cre.len() > 0);
    assert!(cre2.len() > 0);
    assert_eq!(cre, cre2);
    // ------------
    let result = hdlr.try_get_payload("backend_apps/databases/abc_service/HOST");
    let cre3 = result.unwrap();
    let back: serde_json::Value = serde_json::from_str(cre3.as_str()).unwrap();
    assert_eq!(back.is_string(), true);
}

#[test]
fn userspace_access_missing_content() {
    let (app_base_path, secret_lpath) = ut_setup();
    let fullpath = app_base_path + secret_lpath;
    let hdlr = UserSpaceConfidentiality::build(fullpath);
    let result = hdlr.try_get_payload("backend_apps/nonexist-field");
    assert_eq!(result.is_err(), true);
    let err = result.unwrap_err();
    assert_eq!(err.code, AppErrorCode::NoConfidentialityCfg);
    assert!(err.detail.contains("object"));
    // ------------
    let result = hdlr.try_get_payload("amqp_broker/999");
    assert_eq!(result.is_err(), true);
    let err = result.unwrap_err();
    assert_eq!(err.code, AppErrorCode::NoConfidentialityCfg);
    assert!(err.detail.contains("array"));
    // ------------
    let result = hdlr.try_get_payload("amqp_broker/55s");
    assert_eq!(result.is_err(), true);
    let err = result.unwrap_err();
    assert_eq!(err.code, AppErrorCode::NoConfidentialityCfg);
    assert!(err.detail.contains("path-error"));
}

#[test]
fn userspace_source_not_exist() {
    let (app_base_path, _) = ut_setup();
    let secret_lpath = "/unknown/path/to/source.xxx";
    let fullpath = app_base_path + secret_lpath;
    let hdlr = UserSpaceConfidentiality::build(fullpath);
    let result = hdlr.try_get_payload("amqp_broker/0");
    assert_eq!(result.is_err(), true);
    let err = result.unwrap_err();
    assert_eq!(err.code, AppErrorCode::IOerror(ErrorKind::NotFound));
}
