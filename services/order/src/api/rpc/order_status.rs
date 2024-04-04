use serde_json::Value as JsnVal;
use std::vec::Vec;

use crate::error::{AppError, AppErrorCode};
use crate::repository::{app_repo_order, app_repo_order_return};
use crate::rpc::AppRpcClientReqProperty;
use crate::usecase::{
    OrderDiscardUnpaidItemsUseCase, OrderPaymentUpdateUseCase, OrderReplicaInventoryUseCase,
    OrderReplicaPaymentUseCase, OrderReplicaRefundUseCase,
};
use crate::AppSharedState;

use super::build_error_response;
use super::dto::{
    OrderPaymentUpdateDto, OrderReplicaInventoryReqDto, OrderReplicaPaymentReqDto,
    OrderReplicaRefundReqDto,
};

macro_rules! common_setup {
    ($target_dto:ty, $shr_state:ident, $repo_gen:ident, $serial:expr) => {{
        let ds = $shr_state.datastore();
        match serde_json::from_slice::<$target_dto>($serial) {
            Ok(v) => match $repo_gen(ds).await {
                Ok(repo) => Ok((v, repo)),
                Err(e) => Err(e),
            },
            Err(e) => {
                let e = AppError {
                    code: AppErrorCode::InvalidJsonFormat,
                    detail: Some(e.to_string()),
                };
                Err(e)
            }
        }
    }};
}

pub(super) async fn read_reserved_payment(
    req: AppRpcClientReqProperty,
    shr_state: AppSharedState,
) -> Vec<u8> {
    match common_setup!(
        OrderReplicaPaymentReqDto,
        shr_state,
        app_repo_order,
        req.msgbody.as_slice()
    ) {
        Ok((v, repo)) => {
            let uc = OrderReplicaPaymentUseCase { repo };
            match uc.execute(v.order_id).await {
                Ok(resp) => serde_json::to_vec(&resp).unwrap(),
                Err(e) => build_error_response(e).to_string().into_bytes(),
            }
        }
        Err(e) => build_error_response(e).to_string().into_bytes(),
    }
}

pub(super) async fn read_cancelled_refund(
    req: AppRpcClientReqProperty,
    shr_state: AppSharedState,
) -> Vec<u8> {
    match common_setup!(
        OrderReplicaRefundReqDto,
        shr_state,
        app_repo_order_return,
        req.msgbody.as_slice()
    ) {
        Ok((v, repo)) => {
            let uc = OrderReplicaRefundUseCase { repo };
            match uc.execute(v).await {
                Ok(resp) => serde_json::to_vec(&resp).unwrap(),
                Err(e) => build_error_response(e).to_string().into_bytes(),
            }
        }
        Err(e) => build_error_response(e).to_string().into_bytes(),
    }
}

pub(super) async fn read_reserved_inventory(
    req: AppRpcClientReqProperty,
    shr_state: AppSharedState,
) -> Vec<u8> {
    let ds = shr_state.datastore();
    let ret_repo = match app_repo_order_return(ds).await {
        Ok(v) => v,
        Err(e) => {
            return build_error_response(e).to_string().into_bytes();
        }
    };
    match common_setup!(
        OrderReplicaInventoryReqDto,
        shr_state,
        app_repo_order,
        req.msgbody.as_slice()
    ) {
        Ok((v, o_repo)) => {
            let logctx = shr_state.log_context().clone();
            let uc = OrderReplicaInventoryUseCase {
                logctx,
                o_repo,
                ret_repo,
            };
            match uc.execute(v).await {
                Ok(resp) => serde_json::to_vec(&resp).unwrap(),
                Err(e) => build_error_response(e).to_string().into_bytes(),
            }
        }
        Err(e) => build_error_response(e).to_string().into_bytes(),
    }
}

pub(super) async fn update_paid_lines(
    req: AppRpcClientReqProperty,
    shr_state: AppSharedState,
) -> Vec<u8> {
    match common_setup!(
        OrderPaymentUpdateDto,
        shr_state,
        app_repo_order,
        req.msgbody.as_slice()
    ) {
        Ok((v, repo)) => {
            let uc = OrderPaymentUpdateUseCase { repo };
            match uc.execute(v).await {
                Ok(resp) => serde_json::to_vec(&resp).unwrap(),
                Err(e) => build_error_response(e).to_string().into_bytes(),
            }
        }
        Err(e) => build_error_response(e).to_string().into_bytes(),
    }
}

pub(super) async fn discard_unpaid_lines(
    req: AppRpcClientReqProperty,
    shr_state: AppSharedState,
) -> Vec<u8> {
    // it is invoked by scheduled job, no message in the RPC request
    match common_setup!(JsnVal, shr_state, app_repo_order, req.msgbody.as_slice()) {
        Ok((_v, repo)) => {
            let logctx = shr_state.log_context().clone();
            let uc = OrderDiscardUnpaidItemsUseCase::new(repo, logctx);
            let _ = uc.execute().await;
            vec![]
        }
        Err(e) => build_error_response(e).to_string().into_bytes(),
    }
}
