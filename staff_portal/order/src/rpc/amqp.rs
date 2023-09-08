use std::result::Result as DefaultResult;
use std::boxed::Box;
use async_trait::async_trait;

use crate::AppRpcAmqpCfg;
use crate::error::{AppError, AppErrorCode};
use super::{AbstractRpcContext, AbstractRpcClient, AppRpcClientReqProperty, AppRpcReply};

pub(super) struct AmqpRpcContext {}
pub(super) struct AmqpRpcHandler {
    // retry:u8,
    // route:String,
    // corr_id: String
}


#[async_trait]
impl AbstractRpcContext for AmqpRpcContext {
    async fn acquire(&self, _num_retry:u8)
        -> DefaultResult<Box<dyn AbstractRpcClient>, AppError>
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
impl AbstractRpcClient for AmqpRpcHandler {
    async fn send_request(mut self:Box<Self>, _props:AppRpcClientReqProperty)
        -> DefaultResult<Box<dyn AbstractRpcClient>, AppError>
    {
        Err(AppError { code: AppErrorCode::Unknown, detail: None })
    }

    async fn receive_response(&mut self) -> DefaultResult<AppRpcReply, AppError>
    {
        Err(AppError { code: AppErrorCode::Unknown, detail: None })
    }
}

