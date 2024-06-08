use std::boxed::Box;
use std::result::Result;
use std::sync::Arc;

use async_trait::async_trait;

use deadpool_lapin::{Config as DeadpConfig, Pool, PoolConfig, Runtime, Timeouts as DeadpTimeouts};
use lapin::ConnectionProperties;
use serde::Deserialize;

use ecommerce_common::confidentiality::AbstractConfidentiality;
use ecommerce_common::config::AppRpcAmqpCfg;
use ecommerce_common::logging::AppLogContext;

use super::{
    AbsRpcClientContext, AbstractRpcClient, AbstractRpcContext, AbstractRpcPublishEvent,
    AppRpcClientRequest, AppRpcCtxError, AppRpcErrorFnLabel, AppRpcErrorReason, AppRpcReply,
};

#[allow(clippy::upper_case_acronyms)]
#[derive(Deserialize)]
struct SECRET {
    host: String,
    port: u16,
    username: String,
    password: String,
}

pub(super) struct AppAmqpRpcContext {
    _logctx: Arc<AppLogContext>,
    _pool: Pool,
}
struct AppAmqpRpcClient;

struct AppAmqpRpcPublishEvent;

#[async_trait]
impl AbsRpcClientContext for AppAmqpRpcContext {
    async fn acquire(&self) -> Result<Box<dyn AbstractRpcClient>, AppRpcCtxError> {
        let conn = self._pool.get().await.unwrap();
        let _chn = conn.create_channel().await.unwrap();
        // TODO, finish implementation
        let obj = AppAmqpRpcClient;
        Ok(Box::new(obj))
        // Err(AppRpcCtxError {
        //     fn_label: AppRpcErrorFnLabel::AcquireClientConn,
        //     reason: AppRpcErrorReason::NotSupport
        // })
    }
}

impl AbstractRpcContext for AppAmqpRpcContext {}

impl AppAmqpRpcContext {
    pub(super) fn try_build(
        app_cfg: &AppRpcAmqpCfg,
        cfdntl: Arc<Box<dyn AbstractConfidentiality>>,
        _logctx: Arc<AppLogContext>,
    ) -> Result<Self, AppRpcCtxError> {
        let uri = Self::_setup_broker_uri(app_cfg, cfdntl)?;
        let cfg = Self::_setup_lapin_config(app_cfg, uri);
        let _pool = cfg
            .create_pool(Some(Runtime::Tokio1))
            .map_err(|e| Self::_map_err_init(AppRpcErrorReason::LowLevelConn(e.to_string())))?;
        Ok(Self { _logctx, _pool })
    }

    /// Note, `deadpool-lapin` does not apply `lapin::uri::AMQPUri` re-exported
    /// from crate `amq_protocol_uri` to pool configuration, the only way of specifying
    /// URI is to format these element to string in adcvance
    fn _setup_broker_uri(
        app_cfg: &AppRpcAmqpCfg,
        cfdntl: Arc<Box<dyn AbstractConfidentiality>>,
    ) -> Result<String, AppRpcCtxError> {
        let confidential_path = app_cfg.confidential_id.as_str();
        let serial = cfdntl
            .try_get_payload(confidential_path)
            .map_err(|_e| Self::_map_err_init(AppRpcErrorReason::InvalidCredential))?;
        let secret = serde_json::from_str::<SECRET>(serial.as_str())
            .map_err(|_e| Self::_map_err_init(AppRpcErrorReason::CorruptedCredential))?;
        let out = format!(
            "amqp://{}:{}@{}:{}/{}?channel_max={}&heartbeat={}",
            secret.username,
            secret.password,
            secret.host,
            secret.port,
            app_cfg.attributes.vhost.as_str(),
            app_cfg.attributes.max_channels,
            app_cfg.attributes.timeout_secs,
        );
        Ok(out)
    }

    fn _setup_lapin_config(app_cfg: &AppRpcAmqpCfg, uri: String) -> DeadpConfig {
        let timeout_secs = (app_cfg.attributes.timeout_secs as u64) << 2;
        let timeouts = DeadpTimeouts {
            wait: Some(std::time::Duration::new(timeout_secs, 0)),
            create: Some(std::time::Duration::new(timeout_secs, 0)),
            recycle: None,
        };
        let mut poolcfg = PoolConfig::new(app_cfg.max_connections as usize);
        poolcfg.timeouts = timeouts;
        DeadpConfig {
            connection_properties: ConnectionProperties::default(),
            url: Some(uri),
            pool: Some(poolcfg),
        }
    }

    fn _map_err_init(reason: AppRpcErrorReason) -> AppRpcCtxError {
        AppRpcCtxError {
            fn_label: AppRpcErrorFnLabel::InitCtx,
            reason,
        }
    }
} // end of impl AppAmqpRpcContext

#[async_trait]
impl AbstractRpcClient for AppAmqpRpcClient {
    async fn send_request(
        mut self: Box<Self>,
        _props: AppRpcClientRequest,
    ) -> Result<Box<dyn AbstractRpcPublishEvent>, AppRpcCtxError> {
        //let evt = AppAmqpRpcPublishEvent;
        //Ok(Box::new(evt))
        Err(AppRpcCtxError {
            fn_label: AppRpcErrorFnLabel::ClientSendReq,
            reason: AppRpcErrorReason::NotSupport,
        })
    }
}

#[async_trait]
impl AbstractRpcPublishEvent for AppAmqpRpcPublishEvent {
    async fn receive_response(&mut self) -> Result<AppRpcReply, AppRpcCtxError> {
        // Ok(AppRpcReply { message: Vec::new() })
        Err(AppRpcCtxError {
            fn_label: AppRpcErrorFnLabel::ClientRecvResp,
            reason: AppRpcErrorReason::NotSupport,
        })
    }
}
