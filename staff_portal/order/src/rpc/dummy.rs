use std::result::Result as DefaultResult;
use std::boxed::Box;
use async_trait::async_trait;

use crate::error::AppError;
use super::{AbsRpcClientCtx, AbstractRpcContext, AbstractRpcClient, AppRpcReply,
    AppRpcClientReqProperty, AbsRpcServerCtx, AbstractRpcServer};

pub(super) struct DummyRpcContext {}
pub(super) struct DummyRpcHandler {}

#[async_trait]
impl AbsRpcClientCtx for DummyRpcContext {
    async fn acquire(&self, _num_retry:u8)
        -> DefaultResult<Box<dyn AbstractRpcClient>, AppError>
    {
        let hdlr = DummyRpcHandler{};
        Ok(Box::new(hdlr))
    }
}
#[async_trait]
impl AbsRpcServerCtx for DummyRpcContext {
    async fn acquire(&self, _num_retry:u8)
        -> DefaultResult<Box<dyn AbstractRpcServer>, AppError>
    {
        let hdlr = DummyRpcHandler{};
        Ok(Box::new(hdlr))
    }
}

impl AbstractRpcContext for DummyRpcContext {
    fn label(&self) -> &'static str { "dummy" }
}

impl DummyRpcContext {
    pub(crate) fn build () -> Box<dyn AbstractRpcContext>
    { Box::new(Self{}) }
}

#[async_trait]
impl AbstractRpcClient for DummyRpcHandler {
    async fn send_request(mut self:Box<Self>, _props:AppRpcClientReqProperty)
        -> DefaultResult<Box<dyn AbstractRpcClient>, AppError>
    { Ok(self) }

    async fn receive_response(&mut self) -> DefaultResult<AppRpcReply, AppError>
    {
        Ok(AppRpcReply {body: br#"{}"#.to_vec() })
    }
}

#[async_trait]
impl AbstractRpcServer for DummyRpcHandler {
    async fn send_response(mut self:Box<Self>, _props:AppRpcReply)
        -> DefaultResult<(), AppError>
    { Ok(()) }

    async fn receive_request(&mut self)
        -> DefaultResult<AppRpcClientReqProperty, AppError>
    { Ok(AppRpcClientReqProperty{ retry:0, msgbody:Vec::new(),
        route:String::new() })
    }
}

