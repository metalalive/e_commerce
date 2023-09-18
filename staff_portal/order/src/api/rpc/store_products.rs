use std::result::Result as DefaultResult;
use std::vec::Vec;

use crate::AppSharedState;
use crate::error::{AppError, AppErrorCode};
use crate::logging::{app_log_event, AppLogLevel};
use crate::rpc::AppRpcClientReqProperty;
use super::{build_error_response, py_celery_deserialize};


use serde::Deserialize;
use chrono::DateTime;
use chrono::offset::Local;

#[derive(Deserialize)]
struct StoreProductDeleteDto {
    items:Option<Vec<u64>>,
    pkgs :Option<Vec<u64>>
}

#[derive(Deserialize)]
struct StoreProductEditDto {
    price: u32,
    start_after: DateTime<Local>,
    end_before: DateTime<Local>,
    // Note: This order-processing application doesn't need to know the meaning
    // of the field `product type` from this API endpoint, it is just for identifying
    // specific product in specific storefront. There is no need to convert the value
    // at here.
    product_type: u8,
    product_id: u64, // TODO, declare type alias
}

#[derive(Deserialize)]
struct StoreProductDto {
    s_id: u32, // store ID
    rm_all: bool,
    deleting: StoreProductDeleteDto,
    updating: Vec<StoreProductEditDto>,
    creating: Vec<StoreProductEditDto>
}

pub(super) async fn process(req:AppRpcClientReqProperty, shr_state:AppSharedState) -> Vec<u8>
{
    match py_celery_deserialize(&req.msgbody) {
        Ok((_arg, kwarg, _meta)) => match serde_json::from_value(kwarg) {
            Ok(d) => {
                let data: StoreProductDto = d;
                let logctx = shr_state.log_context().clone();
                app_log_event!(logctx, AppLogLevel::INFO, "implementation not done");
                Vec::new()
            },
            Err(e) => {
                let e = AppError {code:AppErrorCode::InvalidJsonFormat,
                    detail: Some(e.to_string()) };
                let err = build_error_response(e);
                err.to_string().into_bytes()
            }
        },
        Err(e) => build_error_response(e).to_string().into_bytes()
    }
} // end of fn process


