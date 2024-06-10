use std::boxed::Box;
use std::result::Result;
use std::sync::Arc;

use async_trait::async_trait;

use chrono::{DateTime, FixedOffset, Local};
use deadpool_lapin::{Config as DeadpConfig, Pool, PoolConfig, Runtime, Timeouts as DeadpTimeouts};
use lapin::options::{BasicPublishOptions, ConfirmSelectOptions};
use lapin::protocol::basic::AMQPProperties;
use lapin::publisher_confirm::Confirmation;
use lapin::{Channel, ConnectionProperties, Error as LapinError};
use serde::Deserialize;

use ecommerce_common::confidentiality::AbstractConfidentiality;
use ecommerce_common::config::{AppAmqpBindingCfg, AppRpcAmqpCfg};
use ecommerce_common::logging::{app_log_event, AppLogContext, AppLogLevel};

use crate::app_meta;

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
    _binding_cfg: Arc<Vec<AppAmqpBindingCfg>>,
    // TODO, hash map for keeping rpc reply
}
struct AppAmqpRpcClient {
    _logctx: Arc<AppLogContext>,
    _binding_cfg: Arc<Vec<AppAmqpBindingCfg>>,
    _chn: Channel,
}
struct AppAmqpRpcPublishEvent {
    _binding_cfg: Arc<Vec<AppAmqpBindingCfg>>,
    _chn: Channel,
    _time: DateTime<FixedOffset>,
}

impl From<LapinError> for AppRpcErrorReason {
    fn from(value: LapinError) -> Self {
        match value {
            LapinError::IOError(ioe) => Self::SysIo(ioe.kind(), ioe.to_string()),
            LapinError::ParsingError(e) => Self::CorruptedPayload(e.to_string()),
            LapinError::SerialisationError(e) => Self::CorruptedPayload(e.to_string()),
            LapinError::ChannelsLimitReached => Self::InternalConfig("channel-limit".to_string()),
            LapinError::InvalidChannel(num) => {
                Self::InternalConfig(format!("invalid-channel: {num}"))
            }
            LapinError::InvalidConnectionState(state) => {
                Self::LowLevelConn(format!("conn-state: {:?}", state))
            }
            LapinError::InvalidChannelState(state) => {
                Self::LowLevelConn(format!("channel-state: {:?}", state))
            }
            LapinError::ProtocolError(e) => Self::LowLevelConn(e.to_string()),
            LapinError::MissingHeartbeatError => {
                Self::LowLevelConn("amqp-no-heartbeat".to_string())
            }
            LapinError::InvalidProtocolVersion(ver) => {
                Self::LowLevelConn(format!("amqp-version: {ver}"))
            }
            _ => Self::NotSupport,
        }
    }
} // end of AppRpcErrorReason

#[async_trait]
impl AbsRpcClientContext for AppAmqpRpcContext {
    async fn acquire(&self) -> Result<Box<dyn AbstractRpcClient>, AppRpcCtxError> {
        let conn =
            self._pool.get().await.map_err(|e| {
                Self::_map_err_acquire(AppRpcErrorReason::LowLevelConn(e.to_string()))
            })?;
        let _chn = conn
            .create_channel()
            .await
            .map_err(|e| Self::_map_err_acquire(e.into()))?;
        _chn.confirm_select(ConfirmSelectOptions { nowait: false })
            .await
            .map_err(|e| Self::_map_err_acquire(e.into()))?; // TODO, configure exactly once for new connection
        let _binding_cfg = self._binding_cfg.clone();
        let _logctx = self._logctx.clone();
        let obj = AppAmqpRpcClient {
            _logctx,
            _chn,
            _binding_cfg,
        };
        Ok(Box::new(obj))
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
        let _binding_cfg = app_cfg.bindings.clone();
        Ok(Self {
            _logctx,
            _pool,
            _binding_cfg,
        })
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
    fn _map_err_acquire(reason: AppRpcErrorReason) -> AppRpcCtxError {
        AppRpcCtxError {
            fn_label: AppRpcErrorFnLabel::AcquireClientConn,
            reason,
        }
    }
} // end of impl AppAmqpRpcContext

#[async_trait]
impl AbstractRpcClient for AppAmqpRpcClient {
    async fn send_request(
        mut self: Box<Self>,
        props: AppRpcClientRequest,
    ) -> Result<Box<dyn AbstractRpcPublishEvent>, AppRpcCtxError> {
        let AppRpcClientRequest {
            mut id,
            message,
            route,
        } = props;
        let AppAmqpRpcClient {
            _logctx,
            _binding_cfg,
            _chn,
        } = *self;
        let bind_cfg = Self::try_get_binding(_binding_cfg.as_ref(), route.as_str())?;
        let reply_cfg = bind_cfg.reply.as_ref().ok_or(Self::_map_err_sendreq(
            AppRpcErrorReason::InternalConfig("amqp-reply-cfg-missing".to_string()),
        ))?;
        let now = Local::now().fixed_offset();
        let corr_id_prefix = reply_cfg.correlation_id_prefix.as_str();
        id.insert(0, '.');
        id.insert_str(0, corr_id_prefix);
        // To create a responsive application, message broker has to return
        // unroutable message whenever the given routing key goes wrong.
        let options = BasicPublishOptions {
            mandatory: true,
            immediate: false,
        };
        let properties = AMQPProperties::default()
            .with_correlation_id(id.into())
            .with_app_id(app_meta::LABAL.into())
            .with_reply_to(reply_cfg.queue.as_str().into())
            .with_content_encoding("utf-8".into())
            .with_content_type("application/json".into())
            .with_delivery_mode(if bind_cfg.durable { 2 } else { 1 })
            .with_timestamp(now.timestamp() as u64);
        let confirm = _chn
            .basic_publish(
                bind_cfg.exchange.as_str(),
                bind_cfg.routing_key.as_str(),
                options,
                &message,
                properties,
            )
            .await
            .map_err(|e| Self::_map_err_sendreq(e.into()))?
            .await
            .map_err(|e| Self::_map_err_sendreq(e.into()))?;
        app_log_event!(
            _logctx,
            AppLogLevel::DEBUG,
            "publish-confirm: {:?}",
            confirm
        );
        Self::convert_confirm_to_error(confirm).map_err(Self::_map_err_sendreq)?;
        let evt = AppAmqpRpcPublishEvent {
            _binding_cfg,
            _chn,
            _time: now,
        };
        Ok(Box::new(evt))
    } // end of fn send_request
} // end of impl AppAmqpRpcClient

impl AppAmqpRpcClient {
    #[allow(clippy::needless_lifetimes)]
    fn try_get_binding<'a, 'b>(
        src: &'a [AppAmqpBindingCfg],
        given_route: &'b str,
    ) -> Result<&'a AppAmqpBindingCfg, AppRpcCtxError> {
        src.iter()
            .find(|c| c.routing_key.as_str() == given_route)
            .ok_or(Self::_map_err_sendreq(AppRpcErrorReason::InvalidRoute(
                given_route.to_string(),
            )))
    }
    fn convert_confirm_to_error(value: Confirmation) -> Result<(), AppRpcErrorReason> {
        let detail = match value {
            Confirmation::NotRequested => {
                // implicitly mean `confirm-select` does not take effect
                Some("amqp-confirm-failure".to_string())
            }
            Confirmation::Nack(_msg) => Some("amqp-unexpected-nack".to_string()),
            Confirmation::Ack(msg) => msg.map(|r| {
                format!(
                    "acker: {:?}, reply-code: {:?}, reply-detail: {:?}",
                    r.acker, r.reply_code, r.reply_text
                )
            }),
        };
        detail.map_or_else(|| Ok(()), |d| Err(AppRpcErrorReason::RequestConfirm(d)))
    }
    fn _map_err_sendreq(reason: AppRpcErrorReason) -> AppRpcCtxError {
        AppRpcCtxError {
            fn_label: AppRpcErrorFnLabel::ClientSendReq,
            reason,
        }
    }
} // end of impl AppAmqpRpcClient

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
