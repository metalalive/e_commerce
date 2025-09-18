use std::vec::Vec;

use ecommerce_common::adapter::rpc;
use ecommerce_common::error::AppErrorCode;
use ecommerce_common::logging::{app_log_event, AppLogLevel};

use crate::error::AppError;
use crate::repository::app_repo_order;
use crate::rpc::AppRpcClientReqProperty;
use crate::usecase::StockLevelUseCase;
use crate::AppSharedState;

use super::build_error_response;
use super::dto::{
    InventoryEditStockLevelDto, StockLevelPresentDto, StockLevelReturnDto, StockReturnErrorDto,
};

macro_rules! common_setup {
    ($req_type:ty, $shr_state:ident, $serial:expr, $uc_fn:expr, $resp_type:ty) => {{
        let logctx_p = $shr_state.log_context().clone();
        app_log_event!(logctx_p, AppLogLevel::DEBUG, "{:?}", $serial);
        let reqbody = match serde_json::from_slice::<$req_type>($serial) {
            Ok(rb) => rb,
            Err(e) => {
                app_log_event!(logctx_p, AppLogLevel::ERROR, "{}", e);
                let e = AppError {
                    code: AppErrorCode::InvalidJsonFormat,
                    detail: Some(e.to_string()),
                };
                return build_error_response(e).to_string().into_bytes();
            }
        };
        let ds = $shr_state.datastore();
        let repo = match app_repo_order(ds).await {
            Ok(r) => r,
            Err(e) => {
                app_log_event!(logctx_p, AppLogLevel::ERROR, "{}", e);
                return build_error_response(e).to_string().into_bytes();
            }
        };
        match $uc_fn(reqbody, repo, logctx_p.clone()).await {
            Ok(uc_resp) => {
                let result = rpc::base_response::<$resp_type>(2, "SUCCESS", Some(uc_resp));
                let r = result.unwrap();
                let resp_raw = serde_json::to_vec(&r).unwrap();
                app_log_event!(logctx_p, AppLogLevel::DEBUG, "{:?}", resp_raw);
                resp_raw
            }
            Err(e) => {
                app_log_event!(logctx_p, AppLogLevel::ERROR, "{}", e);
                build_error_response(e).to_string().into_bytes()
            }
        }
    }};
} // end of common_setup

pub(super) async fn inventory_edit(
    req: AppRpcClientReqProperty,
    shr_state: AppSharedState,
) -> Vec<u8> {
    common_setup!(
        Vec<InventoryEditStockLevelDto>,
        shr_state,
        req.msgbody.as_slice().trim_ascii(),
        StockLevelUseCase::try_edit,
        Vec<StockLevelPresentDto>
    )
}

pub(super) async fn inventory_return_cancelled(
    req: AppRpcClientReqProperty,
    shr_state: AppSharedState,
) -> Vec<u8> {
    common_setup!(
        StockLevelReturnDto,
        shr_state,
        req.msgbody.as_slice(),
        StockLevelUseCase::try_return,
        Vec<StockReturnErrorDto>
    )
}
