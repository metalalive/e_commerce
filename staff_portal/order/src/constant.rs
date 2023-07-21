use crate::WebApiHdlrLabel;

pub const ENV_VAR_SYS_BASE_PATH :&'static str = "SYS_BASE_PATH";
pub const ENV_VAR_SERVICE_BASE_PATH :&'static str = "SERVICE_BASE_PATH";
pub const ENV_VAR_SECRET_FILE_PATH :&'static str  = "SECRET_FILE_PATH";
pub const ENV_VAR_CONFIG_FILE_PATH :&'static str  = "CONFIG_FILE_PATH";

pub const EXPECTED_ENV_VAR_LABELS : [&'static str; 4] = [
    ENV_VAR_SYS_BASE_PATH,    ENV_VAR_SERVICE_BASE_PATH,
    ENV_VAR_SECRET_FILE_PATH, ENV_VAR_CONFIG_FILE_PATH
];

pub(crate) const WEBAPI_ADD_PRODUCT_POLICY: WebApiHdlrLabel = "modify_product_policy";
pub(crate) const WEBAPI_CREATE_NEW_ORDER: WebApiHdlrLabel = "create_new_order";
pub(crate) const WEBAPI_ACCESS_EXISTING_ORDER: WebApiHdlrLabel = "access_existing_order";

pub(crate) const HTTP_CONTENT_TYPE_JSON: &str = "application/json";

pub(crate) mod logging {
    use serde::Deserialize;

    #[derive(Deserialize)]
    pub enum Level {TRACE, DEBUG, INFO, WARNING, ERROR, FATAL}
    
    #[derive(Deserialize)]
    #[serde(rename_all="lowercase")]
    pub enum Destination {
        CONSOLE, LOCALFS,
    } // TODO, Fluentd
}

