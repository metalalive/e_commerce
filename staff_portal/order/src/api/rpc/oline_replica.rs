use std::vec::Vec;

use crate::AppSharedState;
use crate::error::{AppError, AppErrorCode};
use crate::model::OrderLineModel;
use crate::rpc::AppRpcClientReqProperty;
use crate::repository::app_repo_order;

use super::build_error_response;
use super::dto::{OrderReplicaReqDto, OrderReplicaPaymentDto, OrderReplicaInventoryDto};

async fn read_reserved_common (shr_state:AppSharedState, serial:&[u8])
    -> Result<(String, u32, Vec<OrderLineModel>), AppError>
{
    let order_id = match serde_json::from_slice::<OrderReplicaReqDto>(serial)
    {
        Ok(v) => v.order_id,
        Err(e) => {
            let e = AppError {code:AppErrorCode::InvalidJsonFormat,
                   detail:Some(e.to_string()) };
            return Err(e);
        }
    };
    let ds = shr_state.datastore();
    let repo = app_repo_order(ds).await?;
    // apply relaxed layered architecture at here, since these endpoints
    // are simply source part of CQRS pattern, use-case layer is not necessary
    let (usr_id, ms) = repo.fetch_olines(order_id.clone()).await ?;
    Ok((order_id, usr_id, ms))
} // end of fn read_reserved_common


pub(super) async fn read_reserved_payment (req:AppRpcClientReqProperty,
                                           shr_state:AppSharedState) -> Vec<u8>
{
    match read_reserved_common (shr_state, req.msgbody.as_slice()).await
    {
        Ok((oid, usr_id, olines)) => {
            let resp = OrderReplicaPaymentDto {oid, usr_id,
                lines: olines.into_iter().map(OrderLineModel::into).collect()
            };
            serde_json::to_vec(&resp).unwrap()
        },
        Err(e) => build_error_response(e).to_string().into_bytes()
    }
}

pub(super) async fn read_reserved_inventory (req:AppRpcClientReqProperty,
                                             shr_state:AppSharedState) -> Vec<u8>
{
    match read_reserved_common (shr_state, req.msgbody.as_slice()).await
    {
        Ok((oid, usr_id, olines)) => {
            let resp = OrderReplicaInventoryDto {oid, usr_id,
                lines: olines.into_iter().map(OrderLineModel::into).collect()
            };
            serde_json::to_vec(&resp).unwrap()
        },
        Err(e) => build_error_response(e).to_string().into_bytes()
    }
}
