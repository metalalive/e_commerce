pub mod app_meta {
    pub const LABAL: &str = "order";
    pub const MACHINE_CODE: u8 = 1;
    // TODO, machine code to UUID generator should be configurable
    pub const RESOURCE_QUOTA_AP_CODE: u8 = 4;
}

pub mod hard_limit {
    pub const MAX_ITEMS_STORED_PER_MODEL: u32 = 2200u32;
    pub const MAX_ORDER_LINES_PER_REQUEST: usize = 65535;
    pub const MAX_DB_CONNECTIONS: u32 = 10000u32;
    pub const MAX_SECONDS_DB_IDLE: u16 = 600u16;
    pub const MIN_SECS_INTVL_REQ: u16 = 3;
    pub const MAX_NUM_CARTS_PER_USER: u8 = 5; // TODO, configurable in user-mgt app
}

pub(crate) mod api {
    use super::app_meta;
    use crate::error::AppError;
    use crate::WebApiHdlrLabel;
    use ecommerce_common::error::AppErrorCode;
    use std::result::Result as DefaultResult;

    #[allow(non_camel_case_types)]
    pub(crate) struct web {}

    impl web {
        pub(crate) const ADD_PRODUCT_POLICY: WebApiHdlrLabel = "modify_product_policy";
        pub(crate) const CREATE_NEW_ORDER: WebApiHdlrLabel = "create_new_order";
        pub(crate) const ACCESS_EXISTING_ORDER: WebApiHdlrLabel = "access_existing_order";
        pub(crate) const RETURN_OLINES_REQ: WebApiHdlrLabel = "return_lines_request";
        pub(crate) const RETRIEVE_CART_LINES: WebApiHdlrLabel = "retrieve_cart_lines";
        pub(crate) const MODIFY_CART_LINES: WebApiHdlrLabel = "modify_cart_lines";
        pub(crate) const DISCARD_CART: WebApiHdlrLabel = "discard_cart";
    }

    #[allow(non_camel_case_types)]
    pub(crate) struct rpc {}

    impl rpc {
        pub(crate) const EDIT_PRODUCT_PRICE: WebApiHdlrLabel = "update_store_products";
        pub(crate) const CURRENCY_RATE_REFRESH: WebApiHdlrLabel = "currency_exrate_refresh";
        pub(crate) const STOCK_LEVEL_EDIT: WebApiHdlrLabel = "stock_level_edit";
        pub(crate) const STOCK_RETURN_CANCELLED: WebApiHdlrLabel = "stock_return_cancelled";
        pub(crate) const ORDER_RSV_READ_INVENTORY: WebApiHdlrLabel =
            "order_reserved_replica_inventory";
        pub(crate) const ORDER_RSV_READ_PAYMENT: WebApiHdlrLabel = "order_reserved_replica_payment";
        pub(crate) const ORDER_RET_READ_REFUND: WebApiHdlrLabel = "order_returned_replica_refund";
        pub(crate) const ORDER_RSV_UPDATE_PAYMENT: WebApiHdlrLabel =
            "order_reserved_update_payment";
        pub(crate) const ORDER_RSV_DISCARD_UNPAID: WebApiHdlrLabel =
            "order_reserved_discard_unpaid";

        pub(crate) fn extract_handler_label(path: &str) -> DefaultResult<&str, AppError> {
            let mut tokens = path.split('.').collect::<Vec<&str>>();
            if tokens.len() == 3 {
                Self::check_header_label(tokens.remove(0))?;
                Self::check_service_label(tokens.remove(0))?;
                let out = Self::check_hdlr_label(tokens.remove(0))?;
                Ok(out)
            } else {
                let detail = format!("incorrect-rpc-route, tokens:{:?}", tokens);
                Err(AppError {
                    code: AppErrorCode::InvalidInput,
                    detail: Some(detail),
                })
            }
        }
        fn check_header_label(label: &str) -> DefaultResult<(), AppError> {
            if label == "rpc" {
                Ok(())
            } else {
                let detail = format!("incorrect-header:{label}");
                Err(AppError {
                    code: AppErrorCode::InvalidInput,
                    detail: Some(detail),
                })
            }
        }
        fn check_service_label(label: &str) -> DefaultResult<(), AppError> {
            if label == app_meta::LABAL {
                Ok(())
            } else {
                let detail = format!("incorrect-service:{label}");
                Err(AppError {
                    code: AppErrorCode::InvalidInput,
                    detail: Some(detail),
                })
            }
        }
        fn check_hdlr_label(label: &str) -> DefaultResult<&str, AppError> {
            let valid_labels = [
                Self::EDIT_PRODUCT_PRICE,
                Self::CURRENCY_RATE_REFRESH,
                Self::STOCK_LEVEL_EDIT,
                Self::STOCK_RETURN_CANCELLED,
                Self::ORDER_RSV_READ_INVENTORY,
                Self::ORDER_RSV_READ_PAYMENT,
                Self::ORDER_RET_READ_REFUND,
                Self::ORDER_RSV_UPDATE_PAYMENT,
                Self::ORDER_RSV_DISCARD_UNPAID,
            ];
            if valid_labels.contains(&label) {
                Ok(label)
            } else {
                let detail = format!("unrecognised-rpc-handler:{label}");
                Err(AppError {
                    code: AppErrorCode::InvalidInput,
                    detail: Some(detail),
                })
            }
        }
    } // end of inner-struct rpc
} // end of inner-mod api

pub(crate) const HTTP_CONTENT_TYPE_JSON: &str = "application/json";
