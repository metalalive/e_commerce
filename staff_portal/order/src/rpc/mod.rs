mod dummy;
mod amqp;

use std::boxed::Box;
use std::result::Result as DefaultResult;
use std::marker::{Send, Sync};
use std::sync::Arc;

use async_trait::async_trait;

use crate::AppRpcCfg;
use crate::error::AppError;
use crate::rpc::dummy::DummyRpcContext;
use crate::rpc::amqp::AmqpRpcContext;

pub(crate) fn build_context (cfg: &AppRpcCfg)
    -> DefaultResult<Box<dyn AbstractRpcContext> , AppError>
{
    match cfg {
        AppRpcCfg::dummy => DummyRpcContext::build(),
        AppRpcCfg::AMQP(detail_cfg) => AmqpRpcContext::build(detail_cfg),
    }
}

#[async_trait]
pub trait AbstractRpcContext : Send + Sync {
    async fn acquire(&self, num_retry:u8)
        -> DefaultResult<Arc<Box<dyn AbstractRpcHandler>>, AppError>;
    
    fn label (&self) -> &'static str ;
}

#[async_trait]
pub trait AbstractRpcHandler : Send + Sync {
    async fn publish(&mut self, props:AppRpcPublishProperty)
        -> DefaultResult<AppRpcPublishedResult, AppError>;

    async fn consume(&mut self, props:AppRpcConsumeProperty)
        -> DefaultResult<AppRpcConsumeResult, AppError>;
}

pub struct AppRpcPublishProperty {
    pub retry:u8,
    pub msgbody:String,
    pub route:String
}
pub struct AppRpcConsumeProperty{
    pub retry:u8,
    pub route:String,
    pub corr_id: String
}
pub struct AppRpcPublishedResult {
    pub reply_route:String,
    pub job_id: String
}
pub struct AppRpcConsumeResult {
    pub body:String,
    pub properties:Option<AppRpcConsumeProperty>
}

