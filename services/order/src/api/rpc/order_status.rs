use serde_json::Value as JsnVal;
use std::any::type_name;
use std::collections::HashMap;
use std::vec::Vec;

use ecommerce_common::adapter::rpc;
use ecommerce_common::api::rpc::dto::{
    OrderLineReplicaRefundDto, OrderPaymentUpdateDto, OrderPaymentUpdateErrorDto,
    OrderReplicaPaymentDto, OrderReplicaPaymentReqDto, OrderReplicaRefundReqDto,
};
use ecommerce_common::error::AppErrorCode;
use ecommerce_common::logging::{app_log_event, AppLogLevel};

use crate::api::rpc::dto::OrderReplicaInventoryDto;
use crate::error::AppError;
use crate::repository::{app_repo_order, app_repo_order_return};
use crate::rpc::AppRpcClientReqProperty;
use crate::usecase::{
    OrderDiscardUnpaidItemsUseCase, OrderPaymentUpdateUseCase, OrderReplicaInventoryUseCase,
    OrderReplicaPaymentUseCase, OrderReplicaRefundUseCase,
};
use crate::AppSharedState;

use super::build_error_response;
use super::dto::OrderReplicaInventoryReqDto;

macro_rules! common_setup {
    ($target_dto:ty, $shr_state:ident, $repo_gen:ident, $serial:expr) => {{
        let lctx = $shr_state.log_context();
        let ds = $shr_state.datastore();
        let reqtyp = type_name::<$target_dto>();
        app_log_event!(lctx, AppLogLevel::DEBUG, "req-body-type: {reqtyp}");
        match serde_json::from_slice::<$target_dto>($serial) {
            Ok(v) => $repo_gen(ds).await.map(|repo| (v, repo)),
            Err(e) => {
                app_log_event!(
                    lctx,
                    AppLogLevel::WARNING,
                    "serial-raw:{:?}, e:{:?}",
                    $serial,
                    e
                );
                Err(AppError {
                    code: AppErrorCode::InvalidJsonFormat,
                    detail: Some(e.to_string()),
                })
            }
        }
    }};
}

pub(super) async fn read_reserved_payment(
    req: AppRpcClientReqProperty,
    shr_state: AppSharedState,
) -> Vec<u8> {
    let resp = match common_setup!(
        OrderReplicaPaymentReqDto,
        shr_state,
        app_repo_order,
        req.msgbody.as_slice()
    ) {
        Ok((v, repo)) => {
            let uc = OrderReplicaPaymentUseCase { repo };
            match uc.execute(v.order_id).await {
                Ok(uc_resp) => {
                    let r =
                        rpc::base_response::<OrderReplicaPaymentDto>(2, "SUCCESS", Some(uc_resp));
                    r.unwrap()
                }
                Err(e) => build_error_response(e),
            }
        }
        Err(e) => build_error_response(e),
    };
    resp.to_string().into_bytes()
}

pub(super) async fn read_cancelled_refund(
    req: AppRpcClientReqProperty,
    shr_state: AppSharedState,
) -> Vec<u8> {
    let o_repo = match app_repo_order(shr_state.datastore()).await {
        Ok(v) => v,
        Err(e) => {
            return build_error_response(e).to_string().into_bytes();
        }
    };
    let resp = match common_setup!(
        OrderReplicaRefundReqDto,
        shr_state,
        app_repo_order_return,
        req.msgbody.as_slice()
    ) {
        Ok((v, ret_repo)) => {
            let uc = OrderReplicaRefundUseCase { ret_repo, o_repo };
            match uc.execute(v).await {
                Ok(uc_resp) => {
                    let r = rpc::base_response::<HashMap<String, Vec<OrderLineReplicaRefundDto>>>(
                        2,
                        "SUCCESS",
                        Some(uc_resp),
                    );
                    r.unwrap()
                }
                Err(e) => build_error_response(e),
            }
        }
        Err(e) => build_error_response(e),
    };
    resp.to_string().into_bytes()
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
    let resp = match common_setup!(
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
                Ok(uc_resp) => {
                    let r =
                        rpc::base_response::<OrderReplicaInventoryDto>(2, "SUCCESS", Some(uc_resp));
                    r.unwrap()
                }
                Err(e) => build_error_response(e),
            }
        }
        Err(e) => build_error_response(e),
    };
    resp.to_string().into_bytes()
}

pub(super) async fn update_paid_lines(
    req: AppRpcClientReqProperty,
    shr_state: AppSharedState,
) -> Vec<u8> {
    let resp = match common_setup!(
        OrderPaymentUpdateDto,
        shr_state,
        app_repo_order,
        req.msgbody.as_slice()
    ) {
        Ok((v, repo)) => {
            let uc = OrderPaymentUpdateUseCase { repo };
            match uc.execute(v).await {
                Ok(uc_resp) => {
                    let r = rpc::base_response::<OrderPaymentUpdateErrorDto>(
                        2,
                        "SUCCESS",
                        Some(uc_resp),
                    );
                    r.unwrap()
                }
                Err(e) => build_error_response(e),
            }
        }
        Err(e) => build_error_response(e),
    };
    resp.to_string().into_bytes()
}

pub(super) async fn discard_unpaid_lines(
    req: AppRpcClientReqProperty,
    shr_state: AppSharedState,
) -> Vec<u8> {
    // it is invoked by scheduled job, no message in the RPC request
    let resp = match common_setup!(JsnVal, shr_state, app_repo_order, req.msgbody.as_slice()) {
        Ok((_v, repo)) => {
            let logctx = shr_state.log_context().clone();
            let uc = OrderDiscardUnpaidItemsUseCase::new(repo, logctx);
            if let Err(e) = uc.execute().await {
                build_error_response(e)
            } else {
                let r = rpc::base_response::<u8>(2, "SUCCESS", None);
                r.unwrap()
            }
        }
        Err(e) => build_error_response(e),
    };
    resp.to_string().into_bytes()
}
