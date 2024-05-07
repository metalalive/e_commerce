use serde_json::{from_value as json_from_value, json};
use std::env;
use std::fs::{remove_file, File};

use ecommerce_common::constant::env_vars::{SERVICE_BASEPATH, SYS_BASEPATH};
use order::logging::{AppLogContext, AppLogLevel};
use order::{to_3rdparty_level, AppBasepathCfg, AppLoggingCfg};

#[test]
fn init_log_context_ok() {
    let sys_path = env::var(SYS_BASEPATH).unwrap();
    let app_path = env::var(SERVICE_BASEPATH).unwrap();
    // ---- setup
    let basepath = AppBasepathCfg {
        system: sys_path.clone(),
        service: app_path,
    };
    let log_file_path = "tmp/log/test/order_unit_test.log";
    let logger_keys = ["should-be-module-path", "another-module-hier"];
    let cfg = {
        let val = json!({
            "handlers" : [
                {"alias": "errlog-file-456", "min_level": "WARNING",
                 "path": log_file_path,  "destination": "localfs"},
                {"alias": "std-output-123",  "min_level": "ERROR",
                 "destination": "console"}
            ],
            "loggers" : [
                {"alias": logger_keys[0],
                 "handlers": ["errlog-file-456", "std-output-123"],
                 "level": "INFO"},
                {"alias": logger_keys[1],
                 "handlers": ["errlog-file-456"] }
            ]
        });
        json_from_value::<AppLoggingCfg>(val).unwrap()
    };
    let actual = AppLogContext::new(&basepath, &cfg);
    for key in logger_keys {
        let result = actual.get_assigner(key);
        assert_eq!(result.is_some(), true);
        let logger = result.unwrap();
        tracing::dispatcher::with_default(logger, || {
            const LVL: tracing::Level = to_3rdparty_level!(AppLogLevel::ERROR);
            tracing::event!(LVL, "invoked by unit test");
        });
    }
    {
        let fullpath = sys_path + "/" + log_file_path;
        let result = File::open(fullpath.clone());
        assert_eq!(result.is_ok(), true);
        let f = result.unwrap();
        drop(f);
        let result = remove_file(fullpath);
        assert_eq!(result.is_ok(), true);
    }
} // end of init_log_context_ok
