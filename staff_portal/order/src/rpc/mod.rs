mod dummy;
mod amqp;

use std::boxed::Box;
use std::vec::Vec;
use std::result::Result as DefaultResult;
use std::marker::{Send, Sync};

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
    async fn acquire(&self, num_retry:u8) -> DefaultResult<Box<dyn AbstractRpcClient>, AppError>;
    
    fn label (&self) -> &'static str ;
}

#[async_trait]
pub trait AbstractRpcClient : Send + Sync {
    async fn send_request(mut self:Box<Self> , props:AppRpcClientReqProperty)
        -> DefaultResult<Box<dyn AbstractRpcClient>, AppError>;

    async fn receive_response(&mut self) -> DefaultResult<AppRpcReply, AppError>;
}

pub struct AppRpcClientReqProperty {
    pub retry:u8,
    pub msgbody:Vec<u8>,
    pub route:String
}

pub struct AppRpcReply {
    pub body:Vec<u8>,
}


