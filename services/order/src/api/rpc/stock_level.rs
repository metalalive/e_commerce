use std::vec::Vec;

use crate::error::{AppError, AppErrorCode};
use crate::logging::{app_log_event, AppLogLevel};
use crate::repository::app_repo_order;
use crate::rpc::AppRpcClientReqProperty;
use crate::usecase::StockLevelUseCase;
use crate::AppSharedState;

use super::build_error_response;
use super::dto::{InventoryEditStockLevelDto, StockLevelReturnDto};

macro_rules! common_setup {
    ($req_type:ty, $shr_state:ident, $serial:expr, $uc_fn:expr) => {{
        let reqbody = match serde_json::from_slice::<$req_type>($serial) {
            Ok(rb) => rb,
            Err(e) => {
                let e = AppError {
                    code: AppErrorCode::InvalidJsonFormat,
                    detail: Some(e.to_string()),
                };
                return build_error_response(e).to_string().into_bytes();
            }
        };
        let ds = $shr_state.datastore();
        let logctx_p = $shr_state.log_context().clone();
        match app_repo_order(ds).await {
            Ok(repo) => match $uc_fn(reqbody, repo, logctx_p.clone()).await {
                Ok(r) => serde_json::to_vec(&r).unwrap(),
                Err(e) => {
                    app_log_event!(
                        logctx_p,
                        AppLogLevel::ERROR,
                        "[fail-edit-stock-level]:{}",
                        e
                    );
                    build_error_response(e).to_string().into_bytes()
                }
            },
            Err(e) => build_error_response(e).to_string().into_bytes(),
        }
    }};
}

pub(super) async fn inventory_edit(
    req: AppRpcClientReqProperty,
    shr_state: AppSharedState,
) -> Vec<u8> {
    common_setup!(
        Vec<InventoryEditStockLevelDto>,
        shr_state,
        req.msgbody.as_slice(),
        StockLevelUseCase::try_edit
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
        StockLevelUseCase::try_return
    )
}
