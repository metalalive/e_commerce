mod amqp;
mod mock;

use std::boxed::Box;
use std::marker::{Send, Sync};
use std::result::Result;
use std::sync::Arc;

use async_trait::async_trait;
use chrono::{DateTime, Utc};

use ecommerce_common::confidentiality::AbstractConfidentiality;
use ecommerce_common::config::{AppBasepathCfg, AppRpcCfg};
use ecommerce_common::logging::AppLogContext;

use amqp::AppAmqpRpcContext;
use mock::AppMockRpcContext;

#[derive(Clone)]
pub enum AppRpcErrorFnLabel {
    InitCtx,
    AcquireClientConn,
    ClientSendReq,
    ClientRecvResp,
}
#[derive(Clone)]
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

// TODO, discard this when it is not essential to clone
// this clone trait is applied only for current workaround in lazy-init of  rpc connection pool.
// after upgradinf std library to v1.80, replace `OnceLock` with easier-to-implement `LazyLock`
// this clone trait will be no longer needed.
#[derive(Clone)]
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
    pub usr_id: u32,
    pub time: DateTime<Utc>,
    pub message: Vec<u8>,
    pub route: String,
}

pub struct AppRpcReply {
    pub message: Vec<u8>,
}

pub(crate) fn build_context(
    basepath: &AppBasepathCfg,
    cfg: &AppRpcCfg,
    cfdntl: Arc<Box<dyn AbstractConfidentiality>>,
    logctx: Arc<AppLogContext>,
) -> Result<Box<dyn AbstractRpcContext>, AppRpcCtxError> {
    if let AppRpcCfg::AMQP(c) = cfg {
        let obj = AppAmqpRpcContext::try_build(c, cfdntl, logctx)?;
        Ok(Box::new(obj))
    } else if let AppRpcCfg::Mock(c) = cfg {
        let obj = AppMockRpcContext::try_build(basepath, c, logctx)?;
        Ok(Box::new(obj))
    } else {
        Err(AppRpcCtxError {
            fn_label: AppRpcErrorFnLabel::InitCtx,
            reason: AppRpcErrorReason::NotSupport,
        })
    }
}
