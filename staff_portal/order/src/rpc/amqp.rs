use std::result::Result as DefaultResult;
use std::boxed::Box;
use std::sync::Arc;
use async_trait::async_trait;

use crate::AppRpcAmqpCfg;
use crate::confidentiality::AbstractConfidentiality;
use crate::error::{AppError, AppErrorCode};
use super::{
    AbsRpcClientCtx, AbstractRpcContext, AbstractRpcClient, AppRpcClientReqProperty,
    AppRpcReply, AbsRpcServerCtx, AbstractRpcServer
};

pub(super) struct AmqpRpcContext {
    _confidential: Arc<Box<dyn AbstractConfidentiality>>
}
pub(super) struct AmqpRpcHandler {
    // retry:u8,
    // route:String,
    // corr_id: String
}


#[async_trait]
impl AbsRpcClientCtx for AmqpRpcContext {
    async fn acquire (&self, _num_retry:u8)
        -> DefaultResult<Box<dyn AbstractRpcClient>, AppError>
    {
        Err(AppError { code: AppErrorCode::NotImplemented, detail: None })
    }
}
#[async_trait]
impl AbsRpcServerCtx for AmqpRpcContext {
    async fn acquire (&self, _num_retry:u8)
        -> DefaultResult<Box<dyn AbstractRpcServer>, AppError>
    {
        Err(AppError { code: AppErrorCode::NotImplemented, detail: None })
    }
}

impl AbstractRpcContext for AmqpRpcContext {
    fn label(&self) -> &'static str { "AMQP" }
}

impl AmqpRpcContext {
    pub(crate) fn build (_cfg: &AppRpcAmqpCfg, cfdntl:Arc<Box<dyn AbstractConfidentiality>>)
        -> Box<dyn AbstractRpcContext>
    {
        let obj = Self{_confidential:cfdntl};
        Box::new(obj)
    }
}


#[async_trait]
impl AbstractRpcClient for AmqpRpcHandler {
    async fn send_request(mut self:Box<Self>, _props:AppRpcClientReqProperty)
        -> DefaultResult<Box<dyn AbstractRpcClient>, AppError>
    {
        Err(AppError { code: AppErrorCode::NotImplemented, detail: None })
    }

    async fn receive_response(&mut self) -> DefaultResult<AppRpcReply, AppError>
    {
        Err(AppError { code: AppErrorCode::NotImplemented, detail: None })
    }
}

#[async_trait]
impl AbstractRpcServer for AmqpRpcHandler {
    async fn send_response(mut self:Box<Self>, _props:AppRpcReply)
        -> DefaultResult<(), AppError>
    {
        Err(AppError { code: AppErrorCode::NotImplemented, detail: None })
    }

    async fn receive_request(&mut self)
        -> DefaultResult<AppRpcClientReqProperty, AppError>
    {
        Err(AppError { code: AppErrorCode::NotImplemented, detail: None })
    }
}

