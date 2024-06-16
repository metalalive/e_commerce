use std::boxed::Box;
use std::result::Result;
use std::sync::Arc;

use async_trait::async_trait;
use ecommerce_common::adapter::rpc::MockDataSource;
use ecommerce_common::config::{AppBasepathCfg, AppRpcMockCfg};
use ecommerce_common::logging::AppLogContext;
use tokio::sync::Mutex as AsyncMutex;

use super::{
    AbsRpcClientContext, AbstractRpcClient, AbstractRpcContext, AbstractRpcPublishEvent,
    AppRpcClientRequest, AppRpcCtxError, AppRpcErrorFnLabel, AppRpcErrorReason, AppRpcReply,
};

pub(super) struct AppMockRpcContext {
    inner: Arc<AsyncMutex<MockDataSource>>,
}
struct AppMockRpcClient {
    inner: Arc<AsyncMutex<MockDataSource>>,
}
struct AppMockRpcPublishEvent {
    msg: Option<Vec<u8>>,
}

impl AppMockRpcContext {
    pub(super) fn try_build(
        basepath: &AppBasepathCfg,
        cfg: &AppRpcMockCfg,
        _logctx: Arc<AppLogContext>,
    ) -> Result<Self, AppRpcCtxError> {
        let data = MockDataSource::try_build(basepath, cfg).map_err(Self::map_err_init)?;
        Ok(Self {
            inner: Arc::new(AsyncMutex::new(data)),
        })
    }

    fn map_err_init(detail: String) -> AppRpcCtxError {
        AppRpcCtxError {
            fn_label: AppRpcErrorFnLabel::InitCtx,
            reason: AppRpcErrorReason::InternalConfig(detail),
        }
    }
}

#[async_trait]
impl AbsRpcClientContext for AppMockRpcContext {
    async fn acquire(&self) -> Result<Box<dyn AbstractRpcClient>, AppRpcCtxError> {
        let obj = AppMockRpcClient {
            inner: self.inner.clone(),
        };
        Ok(Box::new(obj))
    }
}

impl AbstractRpcContext for AppMockRpcContext {}

#[async_trait]
impl AbstractRpcClient for AppMockRpcClient {
    async fn send_request(
        mut self: Box<Self>,
        props: AppRpcClientRequest,
    ) -> Result<Box<dyn AbstractRpcPublishEvent>, AppRpcCtxError> {
        let mut guard = self.inner.lock().await;
        let value = guard
            .extract(props.route.as_str(), props.usr_id)
            .map_err(|detail| AppRpcCtxError {
                fn_label: AppRpcErrorFnLabel::AcquireClientConn,
                reason: AppRpcErrorReason::InvalidRoute(detail),
            })?;
        let evt = AppMockRpcPublishEvent { msg: Some(value) };
        Ok(Box::new(evt))
    }
}

#[async_trait]
impl AbstractRpcPublishEvent for AppMockRpcPublishEvent {
    async fn receive_response(&mut self) -> Result<AppRpcReply, AppRpcCtxError> {
        self.msg
            .take()
            .map(|message| AppRpcReply { message })
            .ok_or(AppRpcCtxError {
                fn_label: AppRpcErrorFnLabel::ClientRecvResp,
                reason: AppRpcErrorReason::ReplyFailure("already-taken".to_string()),
            })
    }
}
