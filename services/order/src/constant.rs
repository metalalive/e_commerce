use std::hash::Hash;
use std::str::FromStr;

use crate::WebApiHdlrLabel;
use crate::error::{AppError, AppErrorCode};

pub mod app_meta {
    pub const LABAL:&'static str = "order";
    pub const MACHINE_CODE:u8 = 1;
    // TODO, machine code to UUID generator should be configurable
    pub const RESOURCE_QUOTA_AP_CODE: u8 = 4;
}

pub const ENV_VAR_SYS_BASE_PATH :&'static str = "SYS_BASE_PATH";
pub const ENV_VAR_SERVICE_BASE_PATH :&'static str = "SERVICE_BASE_PATH";
pub const ENV_VAR_CONFIG_FILE_PATH :&'static str  = "CONFIG_FILE_PATH";

pub const EXPECTED_ENV_VAR_LABELS : [&'static str; 3] = [
    ENV_VAR_SYS_BASE_PATH,    ENV_VAR_SERVICE_BASE_PATH,
    ENV_VAR_CONFIG_FILE_PATH
];

pub mod limit {
    pub const MAX_ITEMS_STORED_PER_MODEL: u32 = 2200u32;
    pub const MAX_ORDER_LINES_PER_REQUEST: usize = 65535;
    pub const MAX_DB_CONNECTIONS : u32 = 10000u32;
    pub const MAX_SECONDS_DB_IDLE : u16 = 600u16;
    pub const MIN_SECS_INTVL_REQ: u16 = 3;
}

pub(crate) mod api
{
    use std::result::Result as DefaultResult;
    use super::{AppError, AppErrorCode, app_meta, WebApiHdlrLabel};

    #[allow(non_camel_case_types)]
    pub(crate) struct web {}

    impl web {
        pub(crate) const ADD_PRODUCT_POLICY: WebApiHdlrLabel = "modify_product_policy";
        pub(crate) const CREATE_NEW_ORDER: WebApiHdlrLabel = "create_new_order";
        pub(crate) const ACCESS_EXISTING_ORDER: WebApiHdlrLabel = "access_existing_order";
        pub(crate) const RETURN_OLINES_REQ: WebApiHdlrLabel = "return_lines_request";
    }

    #[allow(non_camel_case_types)]
    pub(crate) struct rpc {}

    impl rpc {
        pub(crate) const EDIT_PRODUCT_PRICE: WebApiHdlrLabel = "update_store_products";
        pub(crate) const STOCK_LEVEL_EDIT: WebApiHdlrLabel = "stock_level_edit";
        pub(crate) const STOCK_RETURN_CANCELLED: WebApiHdlrLabel = "stock_return_cancelled";
        pub(crate) const ORDER_RSV_READ_INVENTORY: WebApiHdlrLabel = "order_reserved_replica_inventory";
        pub(crate) const ORDER_RSV_READ_PAYMENT: WebApiHdlrLabel   = "order_reserved_replica_payment";
        pub(crate) const ORDER_RET_READ_REFUND: WebApiHdlrLabel    = "order_returned_replica_refund";
        pub(crate) const ORDER_RSV_UPDATE_PAYMENT: WebApiHdlrLabel = "order_reserved_update_payment";
        pub(crate) const ORDER_RSV_DISCARD_UNPAID: WebApiHdlrLabel = "order_reserved_discard_unpaid";
        
        pub(crate) fn extract_handler_label(path:&str) -> DefaultResult<&str, AppError>
        {
            let mut tokens = path.split(".").collect::<Vec<&str>>();
            if tokens.len() == 3 {
                Self::check_header_label(tokens.remove(0)) ?;
                Self::check_service_label(tokens.remove(0)) ?;
                let out = Self::check_hdlr_label(tokens.remove(0)) ?;
                Ok(out)
            } else {
                let detail = format!("incorrect-format, tokens-length:{}", tokens.len());
                Err(AppError {code: AppErrorCode::InvalidInput, detail: Some(detail) })
            }
        }
        fn check_header_label(label:&str) -> DefaultResult<(), AppError>
        {
            if label == "rpc" {
                Ok(())
            } else {
                let detail = format!("incorrect-header:{label}");
                Err(AppError {code: AppErrorCode::InvalidInput, detail: Some(detail)})
            }
        }
        fn check_service_label(label:&str) -> DefaultResult<(), AppError>
        {
            if label == app_meta::LABAL {
                Ok(())
            } else {
                let detail = format!("incorrect-service:{label}");
                Err(AppError {code: AppErrorCode::InvalidInput, detail: Some(detail)})
            }
        }
        fn check_hdlr_label(label:&str) -> DefaultResult<&str, AppError>
        {
            let valid_labels = [ Self::EDIT_PRODUCT_PRICE,  Self::STOCK_LEVEL_EDIT, 
                 Self::STOCK_RETURN_CANCELLED,   Self::ORDER_RSV_READ_INVENTORY  , 
                 Self::ORDER_RSV_READ_PAYMENT,   Self::ORDER_RET_READ_REFUND     ,
                 Self::ORDER_RSV_UPDATE_PAYMENT, Self::ORDER_RSV_DISCARD_UNPAID  ,
            ];
            if valid_labels.contains(&label) {
                Ok(label)
            } else {
                let detail = format!("incorrect-handler:{label}");
                Err(AppError {code: AppErrorCode::InvalidInput, detail: Some(detail)})
            }
        }
    } // end of inner-struct rpc
} // end of inner-mod api


pub(crate) const HTTP_CONTENT_TYPE_JSON: &str = "application/json";

#[derive(Debug, Eq, Hash)]
pub enum ProductType {Item, Package, Unknown(u8)}

impl From<u8> for ProductType {
    fn from(value: u8) -> Self {
        match value {
            1 => Self::Item,
            2 => Self::Package,
            _others => Self::Unknown(value),
        }
    }
}
impl Into<u8> for ProductType {
    fn into(self) -> u8 {
        match self {
            Self::Unknown(v) => v,
            Self::Item => 1,
            Self::Package => 2
        }
    }
}
impl PartialEq for ProductType {
    fn eq(&self, other: &Self) -> bool {
        let a:u8 = self.clone().into();
        let b:u8 = other.clone().into();
        a == b
    }
    fn ne(&self, other: &Self) -> bool {
        !self.eq(other)
    }
}
impl Clone for ProductType {
    fn clone(&self) -> Self {
        match self {
            Self::Item => Self::Item,
            Self::Unknown(v) => Self::Unknown(v.clone()),
            Self::Package => Self::Package
        }
    }
}
impl FromStr for ProductType {
    type Err = AppError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.parse::<u8>() {
            Ok(v) => Ok(Self::from(v)),
            Err(e) => {
                let detail = format!("product-type, actual:{}, error:{}",
                                      s, e);
                Err(Self::Err {code: AppErrorCode::DataCorruption,
                    detail:Some(detail) })
            }
        }
    }
}

pub(crate) const REGEX_EMAIL_RFC5322 : &'static str = r#"(?:[a-z0-9!#$%&'*+/=?^_`{|}~-]+(?:\.[a-z0-9!#$%&'*+/=?^_`{|}~-]+)*|"(?:[\x01-\x08\x0b\x0c\x0e-\x1f\x21\x23-\x5b\x5d-\x7f]|\\[\x01-\x09\x0b\x0c\x0e-\x7f])*")@(?:(?:[a-z0-9](?:[a-z0-9-]*[a-z0-9])?\.)+[a-z0-9](?:[a-z0-9-]*[a-z0-9])?|\[(?:(?:(2(5[0-5]|[0-4][0-9])|1[0-9][0-9]|[1-9]?[0-9]))\.){3}(?:(2(5[0-5]|[0-4][0-9])|1[0-9][0-9]|[1-9]?[0-9])|[a-z0-9-]*[a-z0-9]:(?:[\x01-\x08\x0b\x0c\x0e-\x1f\x21-\x5a\x53-\x7f]|\\[\x01-\x09\x0b\x0c\x0e-\x7f])+)\])"#;

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

