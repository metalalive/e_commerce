mod dummy;
#[cfg(feature="amqprs")]
mod amqp;

use std::boxed::Box;
use std::pin::Pin;
use std::vec::Vec;
use std::sync::Arc;
use std::future::Future;
use std::result::Result as DefaultResult;
use std::marker::{Send, Sync};

use async_trait::async_trait;
use chrono::{DateTime, FixedOffset};

use crate::{AppRpcCfg, AppSharedState};
use crate::error::{AppError, AppErrorCode};
use crate::confidentiality::AbstractConfidentiality;
use crate::rpc::dummy::DummyRpcContext;
#[cfg(feature="amqprs")]
use self::amqp::AmqpRpcContext;

pub(crate) fn build_context (cfg: &AppRpcCfg, confidential:Arc<Box<dyn AbstractConfidentiality>>)
    -> DefaultResult<Box<dyn AbstractRpcContext>, AppError>
{
    match cfg {
        AppRpcCfg::dummy => Ok(DummyRpcContext::build()),
        AppRpcCfg::AMQP(detail_cfg) => {
            #[cfg(feature="amqprs")]
            {
                AmqpRpcContext::build(detail_cfg, confidential)
            }
            #[cfg(not(feature="amqprs"))]
            {
                let e = AppError { code: AppErrorCode::FeatureDisabled,
                            detail: Some(format!("rpc-amqp-build")) };
                Err(e)
            }
        }
    }
}

pub type AppRpcRouteHdlrFn = fn(AppRpcClientReqProperty, AppSharedState)
    -> Pin<Box<dyn Future<Output=DefaultResult<Vec<u8>, AppError>> + Send + 'static>> ;

#[async_trait]
pub trait AbsRpcClientCtx : Send + Sync {
    async fn acquire(&self, num_retry:u8) -> DefaultResult<Box<dyn AbstractRpcClient>, AppError> ;
}
#[async_trait]
pub trait AbsRpcServerCtx : Send + Sync {
    async fn server_start(
        &self, shr_state:AppSharedState, route_hdlr: AppRpcRouteHdlrFn
    ) -> DefaultResult<(), AppError> ;
} // each implementation manages itw own workflow and resources e.g. connection object

pub trait AbstractRpcContext : AbsRpcClientCtx + AbsRpcServerCtx
{
    fn label (&self) -> &'static str ;
}

#[async_trait]
impl AbsRpcServerCtx for Box<dyn AbstractRpcContext> {
    async fn server_start(
        &self, shr_state:AppSharedState, route_hdlr: AppRpcRouteHdlrFn
    ) -> DefaultResult<(), AppError>
    { // let box pointer of the trait object directly invoke the methods.
        let tobj = self.as_ref();
        AbsRpcServerCtx::server_start(tobj, shr_state, route_hdlr).await
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

pub struct AppRpcClientReqProperty {
    pub retry:u8, // TODO, remove
    pub msgbody:Vec<u8>,
    pub start_time: DateTime<FixedOffset>, // TODO, handle idempotency on server side
    pub route:String
}

pub struct AppRpcReply {
    pub body:Vec<u8>,
}
