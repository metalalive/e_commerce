use std::result::Result as DefaultResult;
use std::boxed::Box;
use std::sync::Arc;
use async_trait::async_trait;

use crate::AppRpcAmqpCfg;
use crate::error::{AppError, AppErrorCode};
use super::{AbstractRpcContext, AbstractRpcHandler, AppRpcPublishProperty, AppRpcPublishedResult, AppRpcConsumeProperty, AppRpcConsumeResult};

pub(super) struct AmqpRpcContext {}
pub(super) struct AmqpRpcHandler {}


#[async_trait]
impl AbstractRpcContext for AmqpRpcContext {
    async fn acquire(&self, _num_retry:u8)
        -> DefaultResult<Arc<Box<dyn AbstractRpcHandler>>, AppError>
    {
        Err(AppError { code: AppErrorCode::Unknown, detail: None })
    }
    fn label(&self) -> &'static str { "AMQP" }
}

impl AmqpRpcContext {
    pub(crate) fn build (_cfg: &AppRpcAmqpCfg) -> DefaultResult<Box<dyn AbstractRpcContext> , AppError>
    {
        let obj = Self{};
        Ok(Box::new(obj))
    }
}


#[async_trait]
impl AbstractRpcHandler for AmqpRpcHandler {
    async fn publish(&mut self, _props:AppRpcPublishProperty)
        -> DefaultResult<AppRpcPublishedResult, AppError>
    {
        Err(AppError { code: AppErrorCode::Unknown, detail: None })
    }

    async fn consume(&mut self, _props:AppRpcConsumeProperty)
        -> DefaultResult<AppRpcConsumeResult, AppError>
    {
        Err(AppError { code: AppErrorCode::Unknown, detail: None })
    }
}

