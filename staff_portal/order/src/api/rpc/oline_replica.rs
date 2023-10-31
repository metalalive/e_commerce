use std::vec::Vec;

use crate::AppSharedState;
use crate::error::{AppError, AppErrorCode};
use crate::rpc::AppRpcClientReqProperty;
use crate::repository::{app_repo_order, AbsOrderRepo};
use crate::usecase::{OrderReplicaPaymentUseCase, OrderReplicaInventoryUseCase};

use super::build_error_response;
use super::dto::OrderReplicaReqDto;

async fn common_setup (shr_state:AppSharedState, serial:&[u8])
    ->  Result<(String, Box<dyn AbsOrderRepo>) , AppError>
{
    match serde_json::from_slice::<OrderReplicaReqDto>(serial)
    {
        Ok(v) => {
            let ds = shr_state.datastore();
            let repo = app_repo_order(ds).await?;
            Ok((v.order_id, repo))
        },
        Err(e) => {
            let e = AppError {code:AppErrorCode::InvalidJsonFormat,
                   detail:Some(e.to_string()) };
            Err(e)
        }
    }
} // end of fn common_setup


pub(super) async fn read_reserved_payment (req:AppRpcClientReqProperty,
                                           shr_state:AppSharedState) -> Vec<u8>
{
    match common_setup(shr_state, req.msgbody.as_slice()).await
    {
        Ok((oid, repo)) => {
            let uc = OrderReplicaPaymentUseCase {repo};
            match uc.execute(oid).await {
                Ok(resp) => serde_json::to_vec(&resp).unwrap(),
                Err(e) => build_error_response(e).to_string().into_bytes()
            }
        },
        Err(e) => build_error_response(e).to_string().into_bytes()
    }
}

pub(super) async fn read_reserved_inventory (req:AppRpcClientReqProperty,
                                             shr_state:AppSharedState) -> Vec<u8>
{
    match common_setup(shr_state, req.msgbody.as_slice()).await
    {
        Ok((oid, repo)) => {
            let uc = OrderReplicaInventoryUseCase {repo};
            match uc.execute(oid).await {
                Ok(resp) => serde_json::to_vec(&resp).unwrap(),
                Err(e) => build_error_response(e).to_string().into_bytes()
            }
        }
        Err(e) => build_error_response(e).to_string().into_bytes()
    }
}
