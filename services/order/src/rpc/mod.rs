#[cfg(feature = "amqprs")]
mod amqp;
mod dummy;

use std::boxed::Box;
use std::future::Future;
use std::marker::{Send, Sync};
use std::pin::Pin;
use std::result::Result as DefaultResult;
use std::sync::Arc;
use std::vec::Vec;

use async_trait::async_trait;
use chrono::{DateTime, FixedOffset};
use ecommerce_common::confidentiality::AbstractConfidentiality;
use ecommerce_common::error::AppErrorCode;
use ecommerce_common::logging::AppLogContext;

#[cfg(feature = "amqprs")]
use self::amqp::AmqpRpcContext;
use crate::error::AppError;
use crate::rpc::dummy::DummyRpcContext;
use crate::{AppRpcCfg, AppSharedState};

#[allow(unused_variables)]
pub(crate) fn build_context(
    cfg: &AppRpcCfg,
    logctx: Arc<AppLogContext>,
    confidential: Arc<Box<dyn AbstractConfidentiality>>,
) -> DefaultResult<Box<dyn AbstractRpcContext>, AppError> {
    match cfg {
        AppRpcCfg::dummy => Ok(DummyRpcContext::build()),
        AppRpcCfg::AMQP(detail_cfg) => {
            #[cfg(feature = "amqprs")]
            {
                AmqpRpcContext::build(detail_cfg, logctx, confidential)
            }
            #[cfg(not(feature = "amqprs"))]
            {
                let e = AppError {
                    code: AppErrorCode::FeatureDisabled,
                    detail: Some("rpc-amqp-build".to_string()),
                };
                Err(e)
            }
        }
        AppRpcCfg::Mock(_c) => {
            let e = AppError {
                code: AppErrorCode::NotImplemented,
                detail: Some("rpc-mock-build".to_string()),
            };
            Err(e)
        }
    }
} // end of fn build-context

pub type AppRpcRouteHdlrFn =
    fn(
        AppRpcClientReqProperty,
        AppSharedState,
    ) -> Pin<Box<dyn Future<Output = DefaultResult<Vec<u8>, AppError>> + Send + 'static>>;

#[async_trait]
pub trait AbsRpcClientCtx: Send + Sync {
    async fn acquire(&self, num_retry: u8) -> DefaultResult<Box<dyn AbstractRpcClient>, AppError>;
}
#[async_trait]
pub trait AbsRpcServerCtx: Send + Sync {
    async fn server_start(
        &self,
        shr_state: AppSharedState,
        route_hdlr: AppRpcRouteHdlrFn,
    ) -> DefaultResult<(), AppError>;
} // each implementation manages itw own workflow and resources e.g. connection object

pub trait AbstractRpcContext: AbsRpcClientCtx + AbsRpcServerCtx {
    fn label(&self) -> &'static str;
}

#[async_trait]
impl AbsRpcServerCtx for Box<dyn AbstractRpcContext> {
    async fn server_start(
        &self,
        shr_state: AppSharedState,
        route_hdlr: AppRpcRouteHdlrFn,
    ) -> DefaultResult<(), AppError> {
        // let box pointer of the trait object directly invoke the methods.
        let tobj = self.as_ref();
        AbsRpcServerCtx::server_start(tobj, shr_state, route_hdlr).await
    }
} // TODO, deref coersion might achieve the same result ? figure out
#[async_trait]
impl AbsRpcClientCtx for Box<dyn AbstractRpcContext> {
    async fn acquire(&self, num_retry: u8) -> DefaultResult<Box<dyn AbstractRpcClient>, AppError> {
        let tobj = self.as_ref();
        AbsRpcClientCtx::acquire(tobj, num_retry).await
    }
}

#[async_trait]
pub trait AbstractRpcClient: Send + Sync {
    async fn send_request(
        mut self: Box<Self>,
        props: AppRpcClientReqProperty,
    ) -> DefaultResult<Box<dyn AbstractRpcClient>, AppError>;

    async fn receive_response(&mut self) -> DefaultResult<AppRpcReply, AppError>;
}

pub struct AppRpcClientReqProperty {
    pub msgbody: Vec<u8>,
    pub correlation_id: Option<String>,
    pub start_time: DateTime<FixedOffset>, // TODO, handle idempotency on server side
    pub route: String,
}

pub struct AppRpcReply {
    pub body: Vec<u8>,
}
