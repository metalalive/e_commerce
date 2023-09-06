use std::result::Result as DefaultResult;
use std::boxed::Box;
use std::sync::Arc;
use async_trait::async_trait;

use crate::error::AppError;
use super::{AbstractRpcContext, AbstractRpcHandler, AppRpcPublishedResult,
    AppRpcConsumeResult, AppRpcPublishProperty, AppRpcConsumeProperty};

pub(super) struct DummyRpcContext {}
pub(super) struct DummyRpcHandler {}

#[async_trait]
impl AbstractRpcContext for DummyRpcContext {
    async fn acquire(&self, _num_retry:u8)
        -> DefaultResult<Arc<Box<dyn AbstractRpcHandler>>, AppError>
    {
        let hdlr = DummyRpcHandler{};
        Ok(Arc::new(Box::new(hdlr)))
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
impl AbstractRpcHandler for DummyRpcHandler {
    async fn publish(&mut self, _props:AppRpcPublishProperty)
        -> DefaultResult<AppRpcPublishedResult, AppError>
    {
        Ok(AppRpcPublishedResult {
            reply_route: "rpc.dummy.route".to_string(),
            job_id: "rpc.dummy.jobid".to_string()
        })
    }

    async fn consume(&mut self, _props:AppRpcConsumeProperty)
        -> DefaultResult<AppRpcConsumeResult, AppError>
    {
        Ok(AppRpcConsumeResult {body: "{}".to_string(), properties:Some(_props) })
    }
}

