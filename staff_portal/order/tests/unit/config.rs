use std::collections::HashMap;

use order::AppConfig;
use order::constant::{ENV_VAR_CONFIG_FILE_PATH, ENV_VAR_SECRET_FILE_PATH, ENV_VAR_SYS_BASE_PATH, ENV_VAR_SERVICE_BASE_PATH};
use order::error::AppErrorCode;

use crate::EXAMPLE_REL_PATH;

#[test]
fn cfg_extract_arg_ok()
{
    let args = [
        (ENV_VAR_CONFIG_FILE_PATH.to_string(), "relative/to/mycfg.json".to_string()),
        (ENV_VAR_SECRET_FILE_PATH.to_string(), "relative/to/secret.json".to_string()),
        (ENV_VAR_SYS_BASE_PATH.to_string(), "/path/sys".to_string()),
        (ENV_VAR_SERVICE_BASE_PATH.to_string(), "/path/service".to_string())
    ];
    let args = HashMap::from(args) ;
    let result = AppConfig::new(args);
    assert_eq!(result.is_err() , true);
    let err = result.err().unwrap();
    // it is normal to get File Not Found error, I don't really assign valid file paths.
    assert_eq!(err.code , AppErrorCode::IOerror(std::io::ErrorKind::NotFound));
}

#[test]
fn cfg_extract_arg_missing_sys_path()
{
    let args = [
        (ENV_VAR_SECRET_FILE_PATH.to_string(), "relative/to/secret.json".to_string()),
    ];
    let args = HashMap::from(args) ;
    let result = AppConfig::new(args);
    assert_eq!(result.is_err() , true);
    let err = result.err().unwrap();
    assert_eq!(err.code , AppErrorCode::MissingSysBasePath);
}

#[test]
fn cfg_extract_arg_missing_service_path()
{
    let args = [
        (ENV_VAR_SYS_BASE_PATH.to_string(), "/path/sys".to_string()),
    ];
    let args = HashMap::from(args) ;
    let result = AppConfig::new(args);
    assert_eq!(result.is_err() , true);
    let err = result.err().unwrap();
    assert_eq!(err.code , AppErrorCode::MissingAppBasePath);
}


#[test]
fn parse_ext_cfg_file_ok ()
{
    let service_basepath = std::env::var(ENV_VAR_SERVICE_BASE_PATH).unwrap() ;
    const CFG_FILEPATH : & str = "config_ok.json";
    let fullpath = service_basepath + EXAMPLE_REL_PATH + CFG_FILEPATH;
    let result = AppConfig::parse_from_file(fullpath);
    assert_eq!(result.is_ok() , true);
    let actual = result.unwrap();
    assert_eq!(actual.listen.api_version.is_empty(), false);
    assert_eq!(actual.listen.host.is_empty(), false);
    assert!(actual.listen.port > 0);
    assert_eq!(actual.listen.routes.is_empty() , false);
    assert_eq!(actual.logging.handlers.is_empty()  , false);
    assert_eq!(actual.logging.loggers.is_empty()  , false);
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
}


fn _parse_ext_cfg_file_error_common (cfg_filepath:&str, expect_err:AppErrorCode)
{
    let service_basepath = std::env::var(ENV_VAR_SERVICE_BASE_PATH).unwrap() ;
    let fullpath = service_basepath + EXAMPLE_REL_PATH + cfg_filepath;
    let result = AppConfig::parse_from_file(fullpath);
    assert_eq!(result.is_err() , true);
    let err = result.err().unwrap();
    assert_eq!(err.code , expect_err);
}

#[test]
fn parse_ext_cfg_file_missing_fields ()
{
    _parse_ext_cfg_file_error_common (
        "config_missing_logging.json",
        AppErrorCode::InvalidJsonFormat );
}

#[test]
fn parse_ext_cfg_file_invalid_api_version ()
{
    _parse_ext_cfg_file_error_common (
        "config_invalid_api_version.json",
        AppErrorCode::InvalidVersion );
}
 
#[test]
fn parse_ext_cfg_file_listener_invalid_fields ()
{
    _parse_ext_cfg_file_error_common (
        "config_empty_routes.json",
         AppErrorCode::NoRouteApiServerCfg);
    _parse_ext_cfg_file_error_common (
        "config_invalid_route.json",
         AppErrorCode::InvalidRouteConfig);
}


#[test]
fn parse_ext_cfg_file_log_invalid_fields ()
{
    _parse_ext_cfg_file_error_common (
        "config_log_no_handler.json",
         AppErrorCode::NoLogHandlerCfg);
    _parse_ext_cfg_file_error_common (
        "config_log_no_logger.json",
         AppErrorCode::NoLoggerCfg);
    _parse_ext_cfg_file_error_common (
        "config_logger_without_handler.json",
         AppErrorCode::NoHandlerInLoggerCfg);
    _parse_ext_cfg_file_error_common (
        "config_logger_with_nonexist_handler.json",
         AppErrorCode::InvalidHandlerLoggerCfg);
}

