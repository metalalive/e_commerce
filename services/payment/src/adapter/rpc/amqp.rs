use std::boxed::Box;
use std::result::Result;

use async_trait::async_trait;

use super::{
    AbsRpcClientContext, AbstractRpcClient, AbstractRpcContext, AbstractRpcPublishEvent,
    AppRpcClientRequest, AppRpcCtxError, AppRpcErrorFnLabel, AppRpcReply,
};

pub(super) struct AppAmqpRpcContext;
struct AppAmqpRpcClient;
struct AppAmqpRpcPublishEvent;

#[async_trait]
impl AbsRpcClientContext for AppAmqpRpcContext {
    async fn acquire(&self) -> Result<Box<dyn AbstractRpcClient>, AppRpcCtxError> {
        // let obj = AppAmqpRpcClient;
        // Ok(Box::new(obj))
        Err(AppRpcCtxError {
            fn_label: AppRpcErrorFnLabel::AcquireClientConn,
        })
    }
}

impl AbstractRpcContext for AppAmqpRpcContext {}

#[async_trait]
impl AbstractRpcClient for AppAmqpRpcClient {
    async fn send_request(
        mut self: Box<Self>,
        _props: AppRpcClientRequest,
    ) -> Result<Box<dyn AbstractRpcPublishEvent>, AppRpcCtxError> {
        //let evt = AppAmqpRpcPublishEvent;
        //Ok(Box::new(evt))
        Err(AppRpcCtxError {
            fn_label: AppRpcErrorFnLabel::ClientSendReq,
        })
    }
}

#[async_trait]
impl AbstractRpcPublishEvent for AppAmqpRpcPublishEvent {
    async fn receive_response(&mut self) -> Result<AppRpcReply, AppRpcCtxError> {
        // Ok(AppRpcReply { message: Vec::new() })
        Err(AppRpcCtxError {
            fn_label: AppRpcErrorFnLabel::ClientRecvResp,
        })
    }
}
