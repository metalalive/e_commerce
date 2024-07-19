mod ut_common;
use std::collections::HashMap;

use ecommerce_common::config::{App3rdPartyCfg, AppCfgHardLimit, AppCfgInitArgs, AppConfig};
use ecommerce_common::constant::env_vars::{CFG_FILEPATH, SERVICE_BASEPATH, SYS_BASEPATH};
use ecommerce_common::error::{AppCfgError, AppErrorCode};

use ut_common::EXAMPLE_REL_PATH;

fn ut_mock_limit() -> AppCfgHardLimit {
    AppCfgHardLimit {
        nitems_per_inmem_table: 2200,
        num_db_conns: 10000,
        seconds_db_idle: 600,
    }
}

#[test]
fn cfg_extract_arg_nonexist_sys_path() {
    let args = [
        (
            CFG_FILEPATH.to_string(),
            "relative/to/mycfg.json".to_string(),
        ),
        (SYS_BASEPATH.to_string(), "/path/sys".to_string()),
        (SERVICE_BASEPATH.to_string(), "/path/service".to_string()),
    ];
    let args = AppCfgInitArgs {
        limit: ut_mock_limit(),
        env_var_map: HashMap::from(args),
    };
    let result = AppConfig::new(args);
    assert!(result.is_err());
    let err = result.err().unwrap();
    // it is normal to get File Not Found error, I don't really assign valid file paths.
    assert_eq!(
        err.code,
        AppErrorCode::IOerror(std::io::ErrorKind::NotFound)
    );
}

#[test]
fn cfg_extract_arg_missing_sys_path() {
    let args = AppCfgInitArgs {
        limit: ut_mock_limit(),
        env_var_map: HashMap::new(),
    };
    let result = AppConfig::new(args);
    assert!(result.is_err());
    let err = result.err().unwrap();
    assert_eq!(err.code, AppErrorCode::MissingSysBasePath);
}

#[test]
fn cfg_extract_arg_missing_service_path() {
    let args = [(SYS_BASEPATH.to_string(), "/path/sys".to_string())];
    let args = AppCfgInitArgs {
        limit: ut_mock_limit(),
        env_var_map: HashMap::from(args),
    };
    let result = AppConfig::new(args);
    assert!(result.is_err());
    let err = result.err().unwrap();
    assert_eq!(err.code, AppErrorCode::MissingAppBasePath);
}

#[test]
fn parse_ext_cfg_file_ok() {
    let service_basepath = std::env::var(SERVICE_BASEPATH).unwrap();
    const CFG_FILEPATH: &str = "config_ok.json";
    let fullpath = service_basepath + EXAMPLE_REL_PATH + CFG_FILEPATH;
    let result = AppConfig::parse_from_file(fullpath, ut_mock_limit());
    assert!(result.is_ok());
    let actual = result.unwrap();
    assert!(!actual.listen.api_version.is_empty());
    assert!(!actual.listen.host.is_empty());
    assert!(actual.listen.port > 0);
    assert!(!actual.listen.routes.is_empty());
    assert!(!actual.logging.handlers.is_empty());
    assert!(!actual.logging.loggers.is_empty());
    assert!(actual.stack_sz_kb > 0);
    for route in actual.listen.routes.iter() {
        assert_eq!(route.path.is_empty(), false);
        assert_eq!(route.handler.is_empty(), false);
    }
    for loghdlr in actual.logging.handlers.iter() {
        assert_eq!(loghdlr.alias.is_empty(), false);
    }
    for logger in actual.logging.loggers.iter() {
        assert_eq!(logger.alias.is_empty(), false);
        assert_eq!(logger.handlers.is_empty(), false);
    }
    assert!(actual.third_parties.is_some());
    if let Some(t) = &actual.third_parties {
        assert_eq!(t.len(), 2);
        t.iter()
            .map(|v| match v.as_ref() {
                App3rdPartyCfg::dev {
                    name,
                    host,
                    port: _,
                    confidentiality_path: _,
                } => {
                    assert_eq!(name.as_str(), "external-service-01");
                    assert_eq!(host.as_str(), "api.ext.service01.com");
                }
                App3rdPartyCfg::test { name, data_src } => {
                    assert_eq!(name.as_str(), "external-service-02");
                    assert_eq!(data_src.as_str(), "/path/to/test-data");
                }
            })
            .count();
    }
} // end of fn parse_ext_cfg_file_ok

fn _parse_ext_cfg_file_error_common(cfg_filepath: &str, expect_err: AppErrorCode) -> AppCfgError {
    let service_basepath = std::env::var(SERVICE_BASEPATH).unwrap();
    let fullpath = service_basepath + EXAMPLE_REL_PATH + cfg_filepath;
    let result = AppConfig::parse_from_file(fullpath, ut_mock_limit());
    assert!(result.is_err());
    let err = result.err().unwrap();
    assert_eq!(err.code, expect_err);
    err
}

#[test]
fn parse_ext_cfg_file_missing_fields() {
    _parse_ext_cfg_file_error_common(
        "config_missing_logging.json",
        AppErrorCode::InvalidJsonFormat,
    );
    let _ = _parse_ext_cfg_file_error_common(
        "config_web_empty_host.json",
        AppErrorCode::InvalidJsonFormat,
    );
    // println!("error detail: {}", x.detail.unwrap());
}

#[test]
fn parse_ext_cfg_file_invalid_api_version() {
    _parse_ext_cfg_file_error_common(
        "config_invalid_api_version.json",
        AppErrorCode::InvalidVersion,
    );
}

#[test]
fn parse_ext_cfg_file_listener_invalid_fields() {
    _parse_ext_cfg_file_error_common(
        "config_web_empty_routes.json",
        AppErrorCode::NoRouteApiServerCfg,
    );
    _parse_ext_cfg_file_error_common(
        "config_invalid_route.json",
        AppErrorCode::InvalidRouteConfig,
    );
    _parse_ext_cfg_file_error_common(
        "config_rpc_empty_bindings.json",
        AppErrorCode::NoRouteApiServerCfg,
    );
}

#[test]
fn parse_ext_cfg_file_log_invalid_fields() {
    _parse_ext_cfg_file_error_common("config_log_no_handler.json", AppErrorCode::NoLogHandlerCfg);
    _parse_ext_cfg_file_error_common("config_log_no_logger.json", AppErrorCode::NoLoggerCfg);
    _parse_ext_cfg_file_error_common(
        "config_logger_without_handler.json",
        AppErrorCode::NoHandlerInLoggerCfg,
    );
    _parse_ext_cfg_file_error_common(
        "config_logger_with_nonexist_handler.json",
        AppErrorCode::InvalidHandlerLoggerCfg,
    );
}

#[test]
fn parse_ext_cfg_file_dstore_exceed_limit() {
    _parse_ext_cfg_file_error_common(
        "config_dstore_inmem_exceed_max_items.json",
        AppErrorCode::ExceedingMaxLimit,
    );
    _parse_ext_cfg_file_error_common(
        "config_dstore_sqldb_exceed_max_conns.json",
        AppErrorCode::ExceedingMaxLimit,
    );
}
