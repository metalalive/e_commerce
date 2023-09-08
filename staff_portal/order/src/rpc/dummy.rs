use std::result::Result as DefaultResult;
use std::boxed::Box;
use async_trait::async_trait;

use crate::error::AppError;
use super::{AbstractRpcContext, AbstractRpcClient, AppRpcReply, AppRpcClientReqProperty};

pub(super) struct DummyRpcContext {}
pub(super) struct DummyRpcHandler {}

#[async_trait]
impl AbstractRpcContext for DummyRpcContext {
    async fn acquire(&self, _num_retry:u8)
        -> DefaultResult<Box<dyn AbstractRpcClient>, AppError>
    {
        let hdlr = DummyRpcHandler{};
        Ok(Box::new(hdlr))
    }
    fn label(&self) -> &'static str { "dummy" }
}

impl DummyRpcContext {
    pub(crate) fn build () -> DefaultResult<Box<dyn AbstractRpcContext> , AppError>
    {
        let obj = Self{};
        Ok(Box::new(obj))
    }
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

