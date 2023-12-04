use std::vec::Vec;

use crate::AppSharedState;
use crate::error::{AppError, AppErrorCode};
use crate::rpc::AppRpcClientReqProperty;
use crate::usecase::EditProductPriceUseCase;
use crate::repository::app_repo_product_price;
use super::{build_error_response, py_celery_deserialize};

pub(super) async fn process(req:AppRpcClientReqProperty, shr_state:AppSharedState) -> Vec<u8>
{
    let ds = shr_state.datastore();
    let repo = match app_repo_product_price(ds).await {
        Ok(r) => r,
        Err(e) => { return build_error_response(e).to_string().into_bytes(); }
    };
    match py_celery_deserialize(&req.msgbody) {
        Ok((_arg, kwarg, _meta)) => match serde_json::from_value(kwarg) {
            Ok(data) => {
                let logctx = shr_state.log_context().clone();
                let result = EditProductPriceUseCase::execute(repo, data, logctx).await;
                if let Err(e) = result {
                    build_error_response(e).to_string().into_bytes()
                } else { // complete successfully
                    Vec::new()
                }
            },
            Err(e) => {
                let e = AppError {code:AppErrorCode::InvalidJsonFormat,
                    detail: Some(e.to_string()) };
                let err = build_error_response(e);
                err.to_string().into_bytes()
            }
        },
        Err(e) => build_error_response(e).to_string().into_bytes()
    } // end of match statement
} // end of fn process

