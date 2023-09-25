use std::vec::Vec;

use crate::AppSharedState;
use crate::error::{AppError, AppErrorCode};
use crate::rpc::AppRpcClientReqProperty;
use crate::repository::app_repo_order;
use crate::usecase::EditStockLevelUseCase;

use super::build_error_response;

pub(super) async fn inventory_edit(req:AppRpcClientReqProperty, shr_state:AppSharedState)
    -> Vec<u8>
{
    let reqbody = match serde_json::from_slice(req.msgbody.as_slice())
    {
        Ok(rb) => rb,
        Err(e) => {
            let e = AppError {code:AppErrorCode::InvalidJsonFormat,
                   detail:Some(e.to_string()) };
            return build_error_response(e).to_string().into_bytes();
        }
    };
    let logctx_p = shr_state.log_context().clone();
    let ds = shr_state.datastore();
    match app_repo_order(ds) {
        Ok(repo) => match EditStockLevelUseCase::execute(reqbody, repo, logctx_p).await {
            Ok(r) => serde_json::to_vec(&r).unwrap(),
            Err(e) => build_error_response(e).to_string().into_bytes()
        },
        Err(e) => build_error_response(e).to_string().into_bytes()
    }
}
