use std::vec::Vec;

use ecommerce_common::logging::{app_log_event, AppLogLevel};

use crate::repository::{app_repo_currency, app_repo_product_price};
use crate::rpc::AppRpcClientReqProperty;
use crate::usecase::{CurrencyRateRefreshUseCase, EditProductPriceUseCase};
use crate::AppSharedState;

use super::dto::ProductPriceDto;
use super::{build_error_response, PyCelery};

pub(super) async fn store_products(
    req: AppRpcClientReqProperty,
    shr_state: AppSharedState,
) -> Vec<u8> {
    let ds = shr_state.datastore();
    let task_id = match PyCelery::get_task_id(&req) {
        Ok(t) => t,
        Err(e) => {
            return build_error_response(e).to_string().into_bytes();
        }
    };
    let repo = match app_repo_product_price(ds).await {
        Ok(r) => r,
        Err(e) => {
            return PyCelery::error_response(task_id, e)
                .to_string()
                .into_bytes();
        }
    };
    let result = PyCelery::deserialize_req::<Vec<String>, ProductPriceDto>(&req.msgbody);
    let s = match result {
        Ok((_arg, data)) => {
            let logctx = shr_state.log_context().clone();
            let result = EditProductPriceUseCase::execute(repo, data, logctx).await;
            if let Err(e) = result {
                PyCelery::error_response(task_id, e)
            } else {
                // complete successfully
                PyCelery::build_response(task_id.as_str(), "SUCCESS")
            }
        }
        Err(e) => PyCelery::error_response(task_id, e),
    };
    s.to_string().into_bytes()
}

pub(super) async fn currency_refresh(
    req: AppRpcClientReqProperty,
    shr_state: AppSharedState,
) -> Vec<u8> {
    let logctx = shr_state.log_context().clone();
    // this endpoint does not require any specific format for message body.
    let task_id = match PyCelery::get_task_id(&req) {
        Ok(t) => t,
        Err(e) => {
            return build_error_response(e).to_string().into_bytes();
        }
    };
    let ds = shr_state.datastore();
    let repo = match app_repo_currency(ds).await {
        Ok(v) => v,
        Err(e) => {
            app_log_event!(logctx, AppLogLevel::ERROR, "{:?}", e);
            return PyCelery::error_response(task_id, e)
                .to_string()
                .into_bytes();
        }
    };
    let exrate_ctx = shr_state.currency();
    let result = CurrencyRateRefreshUseCase::execute(repo, exrate_ctx, logctx).await;
    let resp = if let Err(e) = result {
        PyCelery::error_response(task_id, e)
    } else {
        PyCelery::build_response(task_id.as_str(), "SUCCESS")
    };
    resp.to_string().into_bytes()
}
