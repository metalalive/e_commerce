use std::result::Result as DefaultResult;
use std::vec::Vec;

use crate::AppSharedState;
use crate::rpc::AppRpcClientReqProperty;
use crate::error::{AppError, AppErrorCode} ;

pub async fn route_to_handler(req:AppRpcClientReqProperty, shr_state:AppSharedState)
    -> DefaultResult<Vec<u8>, AppError>
{
    let err = AppError { code: AppErrorCode::NotImplemented,
        detail: Some("rpc-handler-route-table".to_string()) };
    Err(err)
}

