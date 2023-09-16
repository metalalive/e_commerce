mod dummy;
mod amqp;

use std::boxed::Box;
use std::vec::Vec;
use std::sync::Arc;
use std::result::Result as DefaultResult;
use std::marker::{Send, Sync};

use async_trait::async_trait;

use crate::AppRpcCfg;
use crate::error::AppError;
use crate::rpc::dummy::DummyRpcContext;
use crate::rpc::amqp::AmqpRpcContext;
use crate::confidentiality::AbstractConfidentiality;

pub(crate) fn build_context (cfg: &AppRpcCfg, confidential:Arc<Box<dyn AbstractConfidentiality>>)
    -> Box<dyn AbstractRpcContext>
{
    match cfg {
        AppRpcCfg::dummy => DummyRpcContext::build(),
        AppRpcCfg::AMQP(detail_cfg) => AmqpRpcContext::build(detail_cfg, confidential),
    }
}

#[async_trait]
pub trait AbsRpcClientCtx : Send + Sync {
    async fn acquire(&self, num_retry:u8) -> DefaultResult<Box<dyn AbstractRpcClient>, AppError> ;
}
#[async_trait]
pub trait AbsRpcServerCtx : Send + Sync {
    async fn acquire(&self, num_retry:u8) -> DefaultResult<Box<dyn AbstractRpcServer>, AppError> ;
}

pub trait AbstractRpcContext : AbsRpcClientCtx + AbsRpcServerCtx
{
    fn label (&self) -> &'static str ;
}

#[async_trait]
impl AbsRpcServerCtx for Box<dyn AbstractRpcContext> {
    async fn acquire(&self, num_retry:u8) -> DefaultResult<Box<dyn AbstractRpcServer>, AppError>
    { // let box pointer of the trait object directly invoke the methods.
        let tobj = self.as_ref();
        AbsRpcServerCtx::acquire(tobj, num_retry).await
    }
} // TODO, deref coersion might achieve the same result ? figure out
#[async_trait]
impl AbsRpcClientCtx for Box<dyn AbstractRpcContext> {
    async fn acquire(&self, num_retry:u8) -> DefaultResult<Box<dyn AbstractRpcClient>, AppError>
    {
        let tobj = self.as_ref();
        AbsRpcClientCtx::acquire(tobj, num_retry).await
    }
}

#[async_trait]
pub trait AbstractRpcClient : Send + Sync {
    async fn send_request(mut self:Box<Self> , props:AppRpcClientReqProperty)
        -> DefaultResult<Box<dyn AbstractRpcClient>, AppError>;

    async fn receive_response(&mut self) -> DefaultResult<AppRpcReply, AppError>;
}

#[async_trait]
pub trait AbstractRpcServer : Send + Sync {
    async fn send_response(mut self:Box<Self>, props:AppRpcReply)
        -> DefaultResult<(), AppError>;

    async fn receive_request(&mut self)
        -> DefaultResult<AppRpcClientReqProperty, AppError>;
}


pub struct AppRpcClientReqProperty {
    pub retry:u8,
    pub msgbody:Vec<u8>,
    pub route:String
}

pub struct AppRpcReply {
    pub body:Vec<u8>,
}

