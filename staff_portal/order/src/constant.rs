use crate::WebApiHdlrLabel;

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

