use std::boxed::Box;
use std::collections::HashMap;
use std::result::Result;
use std::sync::Arc;

use async_trait::async_trait;

use chrono::{DateTime, Local, Utc};
use deadpool_lapin::{Config as DeadpConfig, Pool, PoolConfig, Runtime, Timeouts as DeadpTimeouts};
use futures_util::StreamExt;
use lapin::message::Delivery;
use lapin::options::{
    BasicConsumeOptions, BasicPublishOptions, ConfirmSelectOptions, QueueDeclareOptions,
};
use lapin::protocol::basic::AMQPProperties;
use lapin::publisher_confirm::Confirmation;
use lapin::topology::TopologyDefinition;
use lapin::types::FieldTable;
use lapin::{Channel, ConnectionProperties, Consumer, Error as LapinError};
use serde::Deserialize;
use tokio::sync::{oneshot, Mutex};
use tokio::time::sleep;

use ecommerce_common::confidentiality::AbstractConfidentiality;
use ecommerce_common::config::{AppAmqpBindingCfg, AppRpcAmqpCfg};
use ecommerce_common::logging::{app_log_event, AppLogContext, AppLogLevel};

use crate::{app_meta, hard_limit};

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

struct InnerClientReplySend(Mutex<HashMap<String, oneshot::Sender<Vec<u8>>>>);

pub(super) struct AppAmqpRpcContext {
    _logctx: Arc<AppLogContext>,
    _pool: Pool,
    _binding_cfg: Arc<Vec<AppAmqpBindingCfg>>,
    _reply_sender: Arc<InnerClientReplySend>,
}
struct AppAmqpRpcClient {
    _logctx: Arc<AppLogContext>,
    _binding_cfg: Arc<Vec<AppAmqpBindingCfg>>,
    _chn: Channel,
    _reply_sender: Arc<InnerClientReplySend>,
}
struct AppAmqpRpcPublishEvent {
    _binding_cfg: Arc<Vec<AppAmqpBindingCfg>>,
    _chn: Channel,
    _time: DateTime<Utc>,
    _reply_recv: Option<oneshot::Receiver<Vec<u8>>>,
}

struct InnerClientConsumer {
    consumer: Consumer,
    logctx: Arc<AppLogContext>,
    _reply_sender: Arc<InnerClientReplySend>,
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

fn generate_consumer_tag(label: &str) -> String {
    let thread_id = std::thread::current().id();
    let (timefmt, nsecs) = {
        let now = Local::now().fixed_offset();
        (now.to_rfc3339(), now.timestamp_subsec_nanos())
    };
    format!("{}-{:?}-{}-{}", label, thread_id, timefmt, nsecs)
}

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
            .await // do confirm every time when channel is open
            .map_err(|e| Self::_map_err_acquire(e.into()))?;
        let declare_history = conn.topology();
        let declared = self.ensure_replyq(&declare_history, _chn.clone()).await?;
        if declared {
            self.start_consume_replyq(_chn.clone()).await?;
        }
        let obj = AppAmqpRpcClient {
            _logctx: self._logctx.clone(),
            _binding_cfg: self._binding_cfg.clone(),
            _reply_sender: self._reply_sender.clone(),
            _chn,
        };
        Ok(Box::new(obj))
    }
} // end of impl AppAmqpRpcContext

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
        Ok(Self {
            _logctx,
            _pool,
            _binding_cfg: app_cfg.bindings.clone(),
            _reply_sender: Arc::new(InnerClientReplySend(Mutex::new(HashMap::new()))),
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

    /// the return boolean indicates whether the call to this function
    /// actually declares the queues (true), or it is just skipped (false)
    async fn ensure_replyq(
        &self,
        declare_history: &TopologyDefinition,
        chn: Channel,
    ) -> Result<bool, AppRpcCtxError> {
        let undeclared = self
            ._binding_cfg
            .iter()
            .filter_map(|cfg| cfg.reply.as_ref())
            .filter(|cfg| {
                !declare_history
                    .queues
                    .iter()
                    .any(|q| q.name.as_str() == cfg.queue.as_str())
            })
            .collect::<Vec<_>>();
        let declared = !undeclared.is_empty();
        if declared {
            let logctx = self._logctx.as_ref();
            app_log_event!(
                logctx,
                AppLogLevel::DEBUG,
                "num-q-to-declare: {}",
                undeclared.len()
            );
        }
        for cfg in undeclared {
            let options = QueueDeclareOptions {
                passive: false,
                durable: cfg.durable,
                exclusive: false,
                auto_delete: false,
                nowait: false,
            };
            let ttl_millis = cfg.ttl_secs as i32 * 1000;
            let mut args = FieldTable::default();
            args.insert("x-message-ttl".into(), ttl_millis.into());
            args.insert("x-max-length".into(), (cfg.max_length as i32).into());
            let _q = chn
                .queue_declare(cfg.queue.as_str(), options, args)
                .await
                .map_err(|e| Self::_map_err_acquire(e.into()))?;
        } // end of loop
        Ok(declared)
    } // end of fn ensure_replyq

    async fn start_consume_replyq(&self, chn: Channel) -> Result<(), AppRpcCtxError> {
        let cfg_iter = self
            ._binding_cfg
            .iter()
            .filter_map(|cfg| cfg.reply.as_ref());
        for cfg in cfg_iter {
            let qname = cfg.queue.as_str();
            let options = BasicConsumeOptions {
                no_local: false,
                no_ack: true,
                exclusive: false,
                nowait: false,
            };
            let consumer = chn
                .basic_consume(
                    qname,
                    generate_consumer_tag(qname).as_str(),
                    options,
                    FieldTable::default(),
                )
                .await
                .map_err(|e| Self::_map_err_acquire(e.into()))?;
            let wrapper = InnerClientConsumer {
                consumer,
                _reply_sender: self._reply_sender.clone(),
                logctx: self._logctx.clone(),
            };
            let _handle = tokio::task::spawn(wrapper.start_consume());
        } // end of loop
        Ok(())
    } // end of fn start_consume_replyq

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
            usr_id,
            time,
            message,
            route,
        } = props;
        let AppAmqpRpcClient {
            _logctx,
            _binding_cfg,
            _chn,
            _reply_sender,
        } = *self;
        let bind_cfg = Self::try_get_binding(_binding_cfg.as_ref(), route.as_str())?;
        let reply_cfg = bind_cfg.reply.as_ref().ok_or(Self::_map_err_sendreq(
            AppRpcErrorReason::InternalConfig("amqp-reply-cfg-missing".to_string()),
        ))?;
        let id = {
            let corr_id_prefix = reply_cfg.correlation_id_prefix.as_str();
            let mut t = time.format("%Y%m%d.%H%M%S").to_string();
            t.insert(0, '.');
            t.insert_str(0, usr_id.to_string().as_str());
            t.insert(0, '.');
            t.insert_str(0, corr_id_prefix);
            t
        };
        let properties = AMQPProperties::default()
            .with_correlation_id(id.as_str().into())
            .with_app_id(app_meta::LABAL.into())
            .with_reply_to(reply_cfg.queue.as_str().into())
            .with_content_encoding("utf-8".into())
            .with_content_type("application/json".into())
            .with_delivery_mode(if bind_cfg.durable { 2 } else { 1 })
            .with_timestamp(time.timestamp() as u64);
        // To create a responsive application, message broker has to return
        // unroutable message whenever the given routing key goes wrong.
        let confirm = _chn
            .basic_publish(
                bind_cfg.exchange.as_str(),
                bind_cfg.routing_key.as_str(),
                BasicPublishOptions {
                    mandatory: true,
                    immediate: false,
                },
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
        let (sender, recv) = oneshot::channel();
        _reply_sender.insert(id, sender).await;
        let evt = AppAmqpRpcPublishEvent {
            _binding_cfg,
            _reply_recv: Some(recv),
            _chn,
            _time: time,
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

impl InnerClientConsumer {
    async fn start_consume(self) {
        let Self {
            mut consumer,
            logctx,
            _reply_sender,
        } = self;
        let tag = consumer.tag();
        while let Some(v) = consumer.next().await {
            let delivered = match v {
                Ok(d) => d,
                Err(e) => {
                    Self::report_error(tag.as_str(), e, logctx.clone());
                    break;
                }
            }; // TODO, figure out whether lapin returns error for connection lost
            if let Err(e) = _reply_sender.try_send(delivered).await {
                app_log_event!(
                    logctx,
                    AppLogLevel::WARNING,
                    "consumer-task: {tag}, reason:{:?}",
                    e
                );
            }
        } // end of loop
        app_log_event!(logctx, AppLogLevel::DEBUG, "end-of-consumer-task: {tag}");
    } // end of fn start_consume

    fn report_error(tag: &str, e: LapinError, logctx: Arc<AppLogContext>) {
        let cond = matches!(e, LapinError::InvalidChannelState(_))
            || matches!(e, LapinError::InvalidConnectionState(_));
        if cond {
            app_log_event!(
                logctx,
                AppLogLevel::WARNING,
                "consumer-task: {tag}, connection issue: {:?}",
                e
            );
        } else {
            app_log_event!(
                logctx,
                AppLogLevel::ERROR,
                "consumer-task: {tag}, error: {:?}",
                e
            );
        }
    }
} // end of impl InnerClientConsumer

impl InnerClientReplySend {
    async fn insert(&self, key: String, value: oneshot::Sender<Vec<u8>>) {
        let mut guard = self.0.lock().await;
        let _discarded = guard.insert(key, value);
    }

    async fn try_send(&self, delivered: Delivery) -> Result<(), String> {
        let (props, msg) = (delivered.properties, delivered.data);
        let key = props
            .correlation_id()
            .as_ref()
            .ok_or("missing-corr-id".to_string())?;
        let mut guard = self.0.lock().await;
        let sender = guard
            .remove(key.as_str())
            .ok_or(format!("invalid-corr-id: {}", key))?;
        sender
            .send(msg)
            .map_err(|_d| format!("fail-pass-msg: {}", key))?;
        Ok(())
    }
} // end of impl InnerClientReplySend

#[async_trait]
impl AbstractRpcPublishEvent for AppAmqpRpcPublishEvent {
    async fn receive_response(&mut self) -> Result<AppRpcReply, AppRpcCtxError> {
        let recv = self
            ._reply_recv
            .take()
            .ok_or(Self::_map_err_recv_resp("already-received"))?;
        let max_time = std::time::Duration::from_secs(hard_limit::RPC_WAIT_FOR_REPLY as u64);
        let message = tokio::select! {
            r = recv => r.map_err(Self::_map_err_recv_resp),
            _ = sleep(max_time) => Err(Self::_map_err_recv_resp("timeout")),
        }?;
        Ok(AppRpcReply { message })
    }
}

impl AppAmqpRpcPublishEvent {
    fn _map_err_recv_resp(detail: impl ToString) -> AppRpcCtxError {
        AppRpcCtxError {
            fn_label: AppRpcErrorFnLabel::ClientRecvResp,
            reason: AppRpcErrorReason::ReplyFailure(detail.to_string()),
        }
    }
}
