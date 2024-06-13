mod amqp;

use std::boxed::Box;
use std::marker::{Send, Sync};
use std::result::Result;
use std::sync::Arc;

use async_trait::async_trait;

use ecommerce_common::confidentiality::AbstractConfidentiality;
use ecommerce_common::config::AppRpcCfg;
use ecommerce_common::logging::AppLogContext;

use amqp::AppAmqpRpcContext;

pub enum AppRpcErrorFnLabel {
    InitCtx,
    AcquireClientConn,
    ClientSendReq,
    ClientRecvResp,
}
pub enum AppRpcErrorReason {
    NotSupport,
    InvalidCredential,
    CorruptedCredential,
    SysIo(std::io::ErrorKind, String),
    LowLevelConn(String),
    InvalidRoute(String),
    InternalConfig(String),
    CorruptedPayload(String),
    RequestConfirm(String),
    ReplyFailure(String),
}
pub struct AppRpcCtxError {
    pub fn_label: AppRpcErrorFnLabel,
    pub reason: AppRpcErrorReason,
}

// Note:
// As of rust v1.75 , the language does not support async trait method
// which returns `dyn Trait` type , so I still use crate `async-trait` at here

#[async_trait]
pub trait AbsRpcClientContext: Sync + Send {
    async fn acquire(&self) -> Result<Box<dyn AbstractRpcClient>, AppRpcCtxError>;
}

pub trait AbstractRpcContext: AbsRpcClientContext {}

#[async_trait]
pub trait AbstractRpcClient: Sync + Send {
    async fn send_request(
        mut self: Box<Self>,
        props: AppRpcClientRequest,
    ) -> Result<Box<dyn AbstractRpcPublishEvent>, AppRpcCtxError>;
}

#[async_trait]
pub trait AbstractRpcPublishEvent: Sync + Send {
    async fn receive_response(&mut self) -> Result<AppRpcReply, AppRpcCtxError>;
}

pub struct AppRpcClientRequest {
    pub id: String, // will be postfix of correlation-id in basic property
    pub message: Vec<u8>,
    pub route: String,
}

pub struct AppRpcReply {
    pub message: Vec<u8>,
}

pub(crate) fn build_context(
    cfg: &AppRpcCfg,
    cfdntl: Arc<Box<dyn AbstractConfidentiality>>,
    logctx: Arc<AppLogContext>,
) -> Result<Box<dyn AbstractRpcContext>, AppRpcCtxError> {
    if let AppRpcCfg::AMQP(c) = cfg {
        let obj = AppAmqpRpcContext::try_build(c, cfdntl, logctx)?;
        Ok(Box::new(obj))
    } else {
        Err(AppRpcCtxError {
            fn_label: AppRpcErrorFnLabel::InitCtx,
            reason: AppRpcErrorReason::NotSupport,
        })
    }
}
