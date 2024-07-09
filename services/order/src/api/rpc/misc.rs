use std::vec::Vec;

use ecommerce_common::logging::{app_log_event, AppLogLevel};

use crate::repository::{app_repo_currency, app_repo_product_price};
use crate::rpc::AppRpcClientReqProperty;
use crate::usecase::{CurrencyRateRefreshUseCase, EditProductPriceUseCase};
use crate::AppSharedState;

use super::dto::ProductPriceDto;
use super::{build_error_response, py_celery_deserialize_req};

pub(super) async fn store_products(
    req: AppRpcClientReqProperty,
    shr_state: AppSharedState,
) -> Vec<u8> {
    let ds = shr_state.datastore();
    let repo = match app_repo_product_price(ds).await {
        Ok(r) => r,
        Err(e) => {
            return build_error_response(e).to_string().into_bytes();
        }
    };
    let result = py_celery_deserialize_req::<Vec<String>, ProductPriceDto>(&req.msgbody);
    match result {
        Ok((_arg, data)) => {
            let logctx = shr_state.log_context().clone();
            let result = EditProductPriceUseCase::execute(repo, data, logctx).await;
            if let Err(e) = result {
                build_error_response(e).to_string().into_bytes()
            } else {
                Vec::new()
            } // complete successfully
        }
        Err(e) => build_error_response(e).to_string().into_bytes(),
    } // end of match statement
}

pub(super) async fn currency_refresh(
    _req: AppRpcClientReqProperty,
    shr_state: AppSharedState,
) -> Vec<u8> {
    let logctx = shr_state.log_context().clone();
    // this endpoint does not require any specific format for message body.
    let ds = shr_state.datastore();
    let repo = match app_repo_currency(ds).await {
        Ok(v) => v,
        Err(e) => {
            app_log_event!(logctx, AppLogLevel::ERROR, "{:?}", e);
            return build_error_response(e).to_string().into_bytes();
        }
    };
    let exrate_ctx = shr_state.currency();
    let result = CurrencyRateRefreshUseCase::execute(repo, exrate_ctx, logctx).await;
    if let Err(e) = result {
        build_error_response(e).to_string().into_bytes()
    } else {
        Vec::new()
    }
}
