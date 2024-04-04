use std::boxed::Box;
use std::collections::HashMap;
use std::result::Result as DefaultResult;
use std::sync::atomic;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use chrono::offset::FixedOffset;
use chrono::{DateTime, Local};
use serde::Deserialize;
use tokio::sync::{Mutex, RwLock};
use tokio::time::sleep;

use amqprs::callbacks::{DefaultChannelCallback, DefaultConnectionCallback};
use amqprs::channel::{
    BasicAckArguments, BasicConsumeArguments, BasicPublishArguments, Channel,
    ConfirmSelectArguments, QueueBindArguments, QueueDeclareArguments,
};
use amqprs::connection::{Connection as AmqpConnection, OpenConnectionArguments};
use amqprs::consumer::AsyncConsumer;
use amqprs::error::Error as AmqpError;
use amqprs::{BasicProperties, Deliver, FieldTable, FieldValue};

use super::{
    AbsRpcClientCtx, AbsRpcServerCtx, AbstractRpcClient, AbstractRpcContext,
    AppRpcClientReqProperty, AppRpcReply, AppRpcRouteHdlrFn,
};
use crate::api::rpc::{py_celery_reply_status, PyCeleryRespStatus};
use crate::confidentiality::AbstractConfidentiality;
use crate::config::{AppAmqpBindingCfg, AppAmqpBindingReplyCfg, AppRpcAmqpCfg};
use crate::constant::{app_meta, HTTP_CONTENT_TYPE_JSON};
use crate::error::{AppError, AppErrorCode};
use crate::logging::{app_log_event, AppLogContext, AppLogLevel};
use crate::{generate_custom_uid, AppSharedState};

#[derive(Deserialize)]
struct BrokerSecret {
    host: String,
    port: u16,
    username: String,
    password: String,
}

// TODO
// - currently all received replies are kept in in-memory structure, however
//   it might not be enough for user growth or scalability. Consider distributed
//   caching like Redis or (non-relational) database persistence
// - periodically remove old entries by last update
struct AmqpClientRecvReply(Mutex<HashMap<String, Option<Vec<u8>>>>);

struct AmqpChannelWrapper {
    chn: Channel,
    subscribe_send_q: Arc<atomic::AtomicBool>,
    subscribe_reply_q: Arc<atomic::AtomicBool>,
}

pub(super) struct AmqpRpcContext {
    // currently use single connection/channel for all publish/consume
    // operations, it will swtich to existing pool like the crate
    // `deadpool` (TODO)
    inner_conn: Mutex<Option<AmqpConnection>>,
    inner_chn: RwLock<Option<AmqpChannelWrapper>>,
    conn_opts: OpenConnectionArguments,
    bindings: Arc<Vec<AppAmqpBindingCfg>>,
    logctx: Arc<AppLogContext>,
    recv_reply: Arc<AmqpClientRecvReply>,
}
struct AmqpRpcClientHandler {
    bindings: Arc<Vec<AppAmqpBindingCfg>>,
    recv_reply: Arc<AmqpClientRecvReply>,
    channel: Channel,
    reply_evt: Option<InnerRecvReplyEvent>,
}
struct InnerRecvReplyEvent {
    corr_id: String,
    py_celery: bool,
    retry: usize, // number of retries on waiting for reply
    intvl: u64,   // time interval in milliseconds for refreshing RPC reply data
}

struct InnerServer {}

struct InnerServerConsumer {
    shr_state: AppSharedState,
    log_ctx: Arc<AppLogContext>,
    route_hdlr: AppRpcRouteHdlrFn,
    _tag: String,
}
struct InnerClientConsumer {
    log_ctx: Arc<AppLogContext>,
    dstore: Arc<AmqpClientRecvReply>,
    _tag: String,
}

impl From<AmqpError> for AppError {
    fn from(value: AmqpError) -> Self {
        let (code, detail) = match value {
            AmqpError::UriError(s) => (AppErrorCode::InvalidRouteConfig, s),
            AmqpError::ConnectionOpenError(s) => (AppErrorCode::RpcRemoteUnavail, s),
            AmqpError::ConnectionCloseError(s) => (AppErrorCode::RpcRemoteUnavail, s),
            AmqpError::ConnectionUseError(s) => (AppErrorCode::Unknown, s + ", conn-misuse"),
            AmqpError::ChannelOpenError(s) => (AppErrorCode::RpcRemoteInvalidReply, s),
            AmqpError::ChannelCloseError(s) => (AppErrorCode::RpcRemoteInvalidReply, s),
            AmqpError::ChannelUseError(s) => (AppErrorCode::Unknown, s + ", channel-misuse"),
            AmqpError::NetworkError(s) => (AppErrorCode::Unknown, s + ", network-error"),
            AmqpError::InternalChannelError(s) => (AppErrorCode::Unknown, s + ", internal-channel"),
            _others => (AppErrorCode::Unknown, format!("rpc-amqp-undefined-err")),
        };
        AppError {
            code,
            detail: Some(detail),
        }
    }
}

#[async_trait]
impl AbsRpcClientCtx for AmqpRpcContext {
    async fn acquire(&self, num_retry: u8) -> DefaultResult<Box<dyn AbstractRpcClient>, AppError> {
        let channel_wrapper = self.try_acquire_channel(num_retry).await?;
        let _done = channel_wrapper
            .init_client(
                self.bindings.clone(),
                self.recv_reply.clone(),
                self.logctx.clone(),
            )
            .await?;
        let AmqpChannelWrapper {
            chn,
            subscribe_send_q: _,
            subscribe_reply_q: _,
        } = channel_wrapper;
        let obj = AmqpRpcClientHandler {
            reply_evt: None,
            bindings: self.bindings.clone(),
            channel: chn,
            recv_reply: self.recv_reply.clone(),
        };
        Ok(Box::new(obj))
    }
}
#[async_trait]
impl AbsRpcServerCtx for AmqpRpcContext {
    async fn server_start(
        &self,
        shr_state: AppSharedState,
        route_hdlr: AppRpcRouteHdlrFn,
    ) -> DefaultResult<(), AppError> {
        let channel_wrapper = self.try_acquire_channel(2).await?;
        let _done = channel_wrapper
            .init_server(self.bindings.clone(), shr_state, route_hdlr)
            .await?;
        // TODO, notify to return, for graceful terminate
        Ok(())
    }
}

impl AbstractRpcContext for AmqpRpcContext {
    fn label(&self) -> &'static str {
        "AMQP"
    }
}

impl AmqpRpcContext {
    pub(crate) fn build(
        cfg: &AppRpcAmqpCfg,
        logctx: Arc<AppLogContext>,
        confidential: Arc<Box<dyn AbstractConfidentiality>>,
    ) -> DefaultResult<Box<dyn AbstractRpcContext>, AppError> {
        let serial = confidential.try_get_payload(cfg.confidential_id.as_str())?;
        let conn_opts = match serde_json::from_str::<BrokerSecret>(serial.as_str()) {
            Ok(s) => OpenConnectionArguments::new(
                s.host.as_str(),
                s.port,
                s.username.as_str(),
                s.password.as_str(),
            )
            .virtual_host(cfg.attributes.vhost.as_str())
            .heartbeat(cfg.attributes.timeout_secs)
            .finish(),
            Err(e) => {
                let detail = e.to_string() + ", secret-parsing-error, source: AmqpRpcContext";
                return Err(AppError {
                    code: AppErrorCode::InvalidJsonFormat,
                    detail: Some(detail),
                });
            }
        };
        let obj = Self {
            conn_opts,
            logctx,
            bindings: cfg.bindings.clone(),
            inner_conn: Mutex::new(None),
            inner_chn: RwLock::new(None),
            recv_reply: Arc::new(AmqpClientRecvReply::default()),
        };
        Ok(Box::new(obj))
    }

    async fn try_acquire_channel(
        &self,
        num_retry: u8,
    ) -> DefaultResult<AmqpChannelWrapper, AppError> {
        let mut result = Err(AppError {
            code: AppErrorCode::Unknown,
            detail: Some(format!("AbsRpcClientCtx::acquire, AmqpRpcContext")),
        });
        for _ in 0..num_retry {
            result = self.ensure_conn_channel().await;
            if result.is_ok() {
                break;
            }
        }
        let out = result?;
        Ok(out)
    }

    async fn _create_conn(&self) -> DefaultResult<AmqpConnection, AppError> {
        // TODO, distinguish low-level network error and auth failure
        let c = AmqpConnection::open(&self.conn_opts).await?;
        c.register_callback(DefaultConnectionCallback).await?;
        assert!(c.is_open());
        Ok(c)
    }

    async fn try_get_channel(&self) -> Option<AmqpChannelWrapper> {
        let guard = self.inner_chn.read().await;
        if let Some(chn_wrapper) = guard.as_ref() {
            chn_wrapper.try_clone()
        } else {
            None
        }
    }
    async fn ensure_channel(
        &self,
        conn: &AmqpConnection,
    ) -> DefaultResult<AmqpChannelWrapper, AppError> {
        // give null channel-ID, let broker randomly generate channel ID
        let result = self.try_get_channel().await;
        let chn_wrapper = if let Some(c) = result {
            c
        } else {
            let c = AmqpChannelWrapper::try_create(conn).await?;
            let mut guard = self.inner_chn.write().await;
            *guard = Some(c.clone());
            c
        };
        Ok(chn_wrapper)
    }
    async fn ensure_conn_channel(&self) -> DefaultResult<AmqpChannelWrapper, AppError> {
        let mut guard = self.inner_conn.lock().await;
        if guard.as_ref().is_none() {
            let c = self._create_conn().await?;
            *guard = Some(c);
        }
        if let Some(conn) = guard.as_ref() {
            match self.ensure_channel(&conn).await {
                Ok(chn_wrapper) => Ok(chn_wrapper),
                Err(e) => {
                    if conn.is_open() {
                        Err(e.into())
                    } else {
                        let conn = guard.take().unwrap();
                        let _result = conn.close().await; // drop and ignore any error
                        let conn = self._create_conn().await?;
                        *guard = Some(conn);
                        let conn = guard.as_ref().unwrap();
                        let chn_wrapper = self.ensure_channel(&conn).await?;
                        Ok(chn_wrapper)
                    }
                }
            }
        } else {
            let d = "amqp-conn-new-fail".to_string();
            Err(AppError {
                code: AppErrorCode::DataCorruption,
                detail: Some(d),
            })
        }
    } // end of fn ensure_conn_channel
} // end of impl AmqpRpcContext

impl Clone for AmqpChannelWrapper {
    fn clone(&self) -> Self {
        Self {
            chn: self.chn.clone(),
            subscribe_send_q: self.subscribe_send_q.clone(),
            subscribe_reply_q: self.subscribe_reply_q.clone(),
        }
    }
}
impl AmqpChannelWrapper {
    fn try_clone(&self) -> Option<Self> {
        if self.chn.is_connection_open() && self.chn.is_open() {
            Some(self.clone())
        } else {
            None
        }
    }
    async fn try_create(conn: &AmqpConnection) -> DefaultResult<Self, AppError> {
        let chn = conn.open_channel(None).await?;
        chn.register_callback(DefaultChannelCallback).await?;
        let subscribe_send_q = Arc::new(atomic::AtomicBool::new(false));
        let subscribe_reply_q = Arc::new(atomic::AtomicBool::new(false));
        let obj = Self {
            chn,
            subscribe_reply_q,
            subscribe_send_q,
        };
        Ok(obj)
    }

    async fn init_server(
        &self,
        bindings: Arc<Vec<AppAmqpBindingCfg>>,
        shr_state: AppSharedState,
        route_hdlr: AppRpcRouteHdlrFn,
    ) -> DefaultResult<bool, AppError> {
        let already_done = self.subscribe_send_q.swap(true, atomic::Ordering::Acquire);
        if already_done {
            return Ok(already_done);
        }
        let log_ctx_p = shr_state.log_context().clone();
        let combo = (0..bindings.len()).zip(bindings.iter());
        for (idx, bind_cfg) in combo {
            if bind_cfg.ensure_declare {
                InnerServer::ensure_send_queue(&self.chn, bind_cfg).await?;
            }
            if let Some(r_cfg) = &bind_cfg.reply {
                InnerServer::ensure_reply_queue(&self.chn, r_cfg).await?;
            }
            if bind_cfg.subscribe {
                let consumer =
                    InnerServerConsumer::new(shr_state.clone(), route_hdlr, idx.to_string());
                let c_tag = consumer.tag().clone();
                let args = BasicConsumeArguments::default()
                    .no_wait(false)
                    .manual_ack(true)
                    .exclusive(false)
                    .queue(bind_cfg.queue.clone())
                    .consumer_tag(c_tag.clone())
                    .finish();
                let _result = self.chn.basic_consume(consumer, args).await?;
                app_log_event!(
                    log_ctx_p,
                    AppLogLevel::DEBUG,
                    "consumer-tag:{} route:{}, queue:{}",
                    c_tag,
                    bind_cfg.routing_key.as_str(),
                    bind_cfg.queue.as_str()
                );
            }
        } // end of loop
        self.subscribe_send_q.store(true, atomic::Ordering::Release);
        Ok(false)
    } // end of fn init_server

    async fn init_client(
        &self,
        bindings: Arc<Vec<AppAmqpBindingCfg>>,
        recv_dstore: Arc<AmqpClientRecvReply>,
        logctx: Arc<AppLogContext>,
    ) -> DefaultResult<bool, AppError> {
        let already_done = self.subscribe_reply_q.swap(true, atomic::Ordering::Acquire);
        if already_done {
            return Ok(already_done);
        } // run this function exactly once
        if let Err(e) = self
            .chn
            .confirm_select(ConfirmSelectArguments::new(false))
            .await
        {
            // only RPC client needs to turn on confirm mode in the broker
            let mut e: AppError = e.into();
            let detail = e.detail.as_mut().unwrap();
            detail.push_str(", confirm_select");
            return Err(e);
        }
        let combo = (0..bindings.len()).zip(bindings.iter());
        for (idx, bind_cfg) in combo {
            if let Some(rcfg) = &bind_cfg.reply {
                let consumer =
                    InnerClientConsumer::new(logctx.clone(), recv_dstore.clone(), idx.to_string());
                let c_tag = consumer.tag().clone();
                app_log_event!(
                    logctx,
                    AppLogLevel::DEBUG,
                    "consumer-tag:{}, route-queue:{}",
                    c_tag.as_str(),
                    rcfg.queue.as_str()
                );
                let args = BasicConsumeArguments::default()
                    .no_wait(false)
                    .manual_ack(false)
                    .exclusive(false)
                    .queue(rcfg.queue.clone())
                    .consumer_tag(c_tag.clone())
                    .finish();
                let result = self.chn.basic_consume(consumer, args).await;
                if let Err(e) = result {
                    app_log_event!(
                        logctx,
                        AppLogLevel::ERROR,
                        "consumer-tag:{}, route-queue:{}, error:{:?}",
                        c_tag.as_str(),
                        rcfg.queue.as_str(),
                        e
                    );
                    return Err(e.into());
                }
            } // TODO, record number of consumers added to the same channel
        } // end of loop
        self.subscribe_reply_q
            .store(true, atomic::Ordering::Release);
        Ok(false)
    } // end of fn
} // end of impl AmqpChannelWrapper

#[async_trait]
impl AbstractRpcClient for AmqpRpcClientHandler {
    async fn send_request(
        mut self: Box<Self>,
        req: AppRpcClientReqProperty,
    ) -> DefaultResult<Box<dyn AbstractRpcClient>, AppError> {
        let (route, content, t_start) = (req.route, req.msgbody, req.start_time);
        let bind_cfg = Self::try_get_binding(self.bindings.as_ref(), route.as_str())?;
        let (reply_q_name, corr_id_prefix) = if let Some(r_cfg) = &bind_cfg.reply {
            (r_cfg.queue.as_str(), r_cfg.correlation_id_prefix.as_str())
        } else {
            let e = AppError {
                code: AppErrorCode::InvalidInput,
                detail: Some("rpc-client-seq, empty-reply-cfg".to_string()),
            };
            return Err(e);
        };
        let mut corr_id = generate_custom_uid(app_meta::MACHINE_CODE)
            .into_bytes()
            .into_iter()
            .map(|b| format!("{:02x}", b))
            .collect::<Vec<String>>()
            .join("");
        corr_id.insert(0, '.');
        corr_id.insert_str(0, corr_id_prefix);
        let mut properties = BasicProperties::default()
            .with_app_id(app_meta::LABAL)
            .with_content_type(HTTP_CONTENT_TYPE_JSON)
            .with_content_encoding("utf-8")
            .with_persistence(bind_cfg.durable)
            .with_reply_to(reply_q_name)
            .with_correlation_id(corr_id.as_str())
            .with_timestamp(t_start.timestamp() as u64)
            .finish();
        let properties = if let Some(py_tsk_path) = &bind_cfg.python_celery_task {
            let mut extra_headers = FieldTable::new();
            extra_headers.insert(
                "id".try_into().unwrap(),
                FieldValue::S(corr_id.clone().try_into().unwrap()),
            );
            extra_headers.insert(
                "task".try_into().unwrap(),
                FieldValue::S(py_tsk_path.clone().try_into().unwrap()),
            );
            extra_headers.insert(
                "content_type".try_into().unwrap(),
                FieldValue::S(HTTP_CONTENT_TYPE_JSON.try_into().unwrap()),
            );
            properties.with_headers(extra_headers).finish()
        } else {
            properties
        };
        let py_celery = bind_cfg.python_celery_task.is_some();
        let args = BasicPublishArguments::default()
            .exchange(bind_cfg.exchange.clone())
            .routing_key(bind_cfg.routing_key.clone())
            // To create a responsive application, message broker has to return
            // unroutable message whenever the given routing key goes wrong.
            .mandatory(true)
            .immediate(false) // RabbitMQ v3 removed this flag, I always set it to false
            // , the crate `amqp-rs` reserves this flag for backward
            // compatibility
            .finish();
        if let Err(e) = self.channel.basic_publish(properties, content, args).await {
            let mut e: AppError = e.into();
            if matches!(e.code, AppErrorCode::Unknown) {
                e.code = AppErrorCode::RpcPublishFailure;
            }
            return Err(e);
        }
        // update at the end , due to borrow / mutability constraint at compile time
        self.recv_reply.claim(corr_id.as_str()).await?;
        self.as_mut().reply_evt = {
            let evt = InnerRecvReplyEvent {
                py_celery,
                corr_id,
                retry: 20usize,
                intvl: 500u64,
            };
            Some(evt)
        };
        Ok(self)
    } // end of fn send_request

    async fn receive_response(&mut self) -> DefaultResult<AppRpcReply, AppError> {
        if let Some(evt) = self.reply_evt.as_ref() {
            let mut celery_status = PyCeleryRespStatus::ERROR;
            for _ in 0..evt.retry {
                match self.recv_reply.fetch(evt.corr_id.as_str()).await {
                    Ok(body) => {
                        let done = if evt.py_celery {
                            celery_status = py_celery_reply_status(&body)?;
                            matches!(celery_status, PyCeleryRespStatus::SUCCESS)
                        } else {
                            true
                        };
                        if done {
                            return Ok(AppRpcReply { body });
                        }
                    }
                    Err(e) => {
                        if matches!(e.code, AppErrorCode::RpcReplyNotReady) {
                            sleep(Duration::from_millis(evt.intvl)).await;
                        } else {
                            return Err(e);
                        }
                    }
                }
            }
            if evt.py_celery && !matches!(celery_status, PyCeleryRespStatus::SUCCESS) {
                let detail = format!(
                    "py-celery, status:{:?}, corr-id:{}",
                    celery_status,
                    evt.corr_id.as_str()
                );
                Err(AppError {
                    code: AppErrorCode::RpcConsumeFailure,
                    detail: Some(detail),
                })
            } else {
                let detail = format!("rpc-client, corr-id:{}", evt.corr_id.as_str());
                Err(AppError {
                    code: AppErrorCode::RpcReplyNotReady,
                    detail: Some(detail),
                })
            }
        } else {
            let detail = format!("rpc-client-recv-reply, missing-corr-id");
            Err(AppError {
                code: AppErrorCode::DataCorruption,
                detail: Some(detail),
            })
        }
    } // end of fn receive_response
} // end of impl AbstractRpcClient for AmqpRpcHandler

impl AmqpRpcClientHandler {
    fn try_get_binding<'a, 'b>(
        src: &'a Vec<AppAmqpBindingCfg>,
        route_key: &'b str,
    ) -> DefaultResult<&'a AppAmqpBindingCfg, AppError> {
        let result = src
            .iter()
            .position(|bind_cfg| bind_cfg.routing_key.as_str() == route_key);
        if let Some(idx) = result {
            let bind_cfg = src.get(idx).unwrap();
            Ok(bind_cfg)
        } else {
            let d = format!("binding-cfg-not-found, {route_key}");
            Err(AppError {
                code: AppErrorCode::InvalidInput,
                detail: Some(d),
            })
        }
    }
}

impl InnerServer {
    async fn ensure_send_queue(
        channel: &Channel,
        cfg: &AppAmqpBindingCfg,
    ) -> DefaultResult<(), AppError> {
        let ttl_millis = cfg.ttl_secs as i32 * 1000;
        let max_num_msgs = cfg.max_length as i32;
        let mut properties = FieldTable::new();
        properties.insert(
            "x-message-ttl".try_into().unwrap(),
            FieldValue::I(ttl_millis),
        );
        properties.insert(
            "x-max-length".try_into().unwrap(),
            FieldValue::I(max_num_msgs),
        );
        // note the flag `passive` only checks whether the queue exists,
        // the broker reports `OK` if exists or ambigious error if not.
        let args = QueueDeclareArguments::new(cfg.queue.as_str())
            .durable(cfg.durable)
            .passive(!cfg.ensure_declare)
            .auto_delete(false)
            .no_wait(false)
            .arguments(properties)
            .finish();
        let result = channel.queue_declare(args).await?;
        if let Some((_q_name, _msg_cnt, _consumer_cnt)) = result {
            // TODO, logging debug message
            //println!("[debug] queue-declare-ok: {_q_name}, {_msg_cnt}, {_consumer_cnt}");
        }
        let args = QueueBindArguments::new(
            cfg.queue.as_str(),
            cfg.exchange.as_str(),
            cfg.routing_key.as_str(),
        )
        .no_wait(false)
        .finish();
        channel.queue_bind(args).await?;
        Ok(())
    } // end of fn ensure_send_queue

    async fn ensure_reply_queue(
        channel: &Channel,
        cfg: &AppAmqpBindingReplyCfg,
    ) -> DefaultResult<(), AppError> {
        let (queue, ttl_secs) = (cfg.queue.as_str(), cfg.ttl_secs);
        let ttl_millis = ttl_secs as i32 * 1000;
        let max_num_msgs = cfg.max_length as i32;
        let mut properties = FieldTable::new();
        properties.insert(
            "x-message-ttl".try_into().unwrap(),
            FieldValue::I(ttl_millis),
        );
        properties.insert(
            "x-max-length".try_into().unwrap(),
            FieldValue::I(max_num_msgs),
        );
        let args = QueueDeclareArguments::new(queue)
            .durable(cfg.durable)
            .passive(false)
            .auto_delete(false)
            .no_wait(false)
            .arguments(properties)
            .finish();
        let _result = channel.queue_declare(args).await?;
        Ok(())
    }
} // end of impl InnerServer

impl InnerServerConsumer {
    fn new(shr_state: AppSharedState, route_hdlr: AppRpcRouteHdlrFn, tag_postfix: String) -> Self {
        let _tag = Self::generate_tag(tag_postfix);
        let log_ctx = shr_state.log_context().clone();
        Self {
            _tag,
            log_ctx,
            shr_state,
            route_hdlr,
        }
    }
    fn generate_tag(postfix: String) -> String {
        let thread_id = std::thread::current().id();
        let (timefmt, nsecs) = {
            let now = Local::now().fixed_offset();
            (now.to_rfc3339(), now.timestamp_subsec_nanos())
        };
        format!("server-{:?}-{}-{}-{}", thread_id, timefmt, nsecs, postfix)
    }
    fn tag(&self) -> &String {
        &self._tag
    }

    async fn try_send_response(
        channel: &Channel,
        req_props: BasicProperties,
        t_end: DateTime<FixedOffset>,
        content: Vec<u8>,
    ) -> DefaultResult<Option<String>, AppError> {
        let (reply_to, corr_id) = (req_props.reply_to(), req_props.correlation_id());
        if reply_to.is_none() {
            return Ok(Some("reply-to".to_string()));
        } else if corr_id.is_none() {
            return Ok(Some("correlation-id".to_string()));
        }
        let resp_props = BasicProperties::default()
            .with_app_id(app_meta::LABAL)
            .with_content_type(HTTP_CONTENT_TYPE_JSON)
            .with_content_encoding("utf-8")
            .with_correlation_id(corr_id.unwrap().as_str())
            .with_timestamp(t_end.timestamp() as u64)
            .finish(); // delivery mode can be omitted ?
        let args = BasicPublishArguments::default() // use default a-non exchange
            .routing_key(reply_to.unwrap().clone())
            .mandatory(true)
            .immediate(false)
            .finish();
        channel.basic_publish(resp_props, content, args).await?;
        Ok(None)
    } // end of fn try_send_response

    async fn _consume(
        &mut self,
        channel: &Channel,
        deliver: Deliver,
        req_props: BasicProperties,
        content: Vec<u8>,
    ) -> DefaultResult<Option<String>, AppError> {
        let local_t0 = Local::now().fixed_offset();
        let start_time = match req_props.timestamp() {
            Some(ts) => match ts.try_into() {
                Ok(ts2) => DateTime::from_timestamp(ts2, 0u32)
                    .unwrap_or(local_t0.into())
                    .fixed_offset(),
                Err(_e) => local_t0,
            },
            None => local_t0,
        };
        let req = AppRpcClientReqProperty {
            msgbody: content,
            start_time,
            route: deliver.routing_key().clone(),
        };
        let hdlr_fn = self.route_hdlr;
        let resp_body = hdlr_fn(req, self.shr_state.clone()).await?;
        let local_t1 = Local::now().fixed_offset();
        let missing = Self::try_send_response(channel, req_props, local_t1, resp_body).await?;
        let ack_args = BasicAckArguments::new(deliver.delivery_tag(), false);
        channel.basic_ack(ack_args).await?;
        Ok(missing)
    }
} // end of impl InnerServerConsumer

#[async_trait]
impl AsyncConsumer for InnerServerConsumer {
    async fn consume(
        &mut self,
        channel: &Channel,
        deliver: Deliver,
        basic_properties: BasicProperties,
        content: Vec<u8>,
    ) {
        let log_ctx_p = self.log_ctx.clone();
        let route_key_log = deliver.routing_key().clone();
        let part_content_log = {
            let sz = std::cmp::min(20, content.len());
            (&content[..sz]).to_vec()
        };
        app_log_event!(
            log_ctx_p,
            AppLogLevel::DEBUG,
            "route:{}, content:{:?}",
            route_key_log,
            part_content_log
        );
        let result = self
            ._consume(channel, deliver, basic_properties, content)
            .await;
        match result {
            Ok(r) => {
                if let Some(m) = r {
                    app_log_event!(
                        log_ctx_p,
                        AppLogLevel::WARNING,
                        "route:{}, content:{:?}, \
                               missing:{}",
                        route_key_log,
                        part_content_log,
                        m
                    );
                }
            }
            Err(e) => {
                app_log_event!(
                    log_ctx_p,
                    AppLogLevel::ERROR,
                    "route:{}, content:{:?}, \
                               error: {:?}",
                    route_key_log,
                    part_content_log,
                    e
                );
            }
        }
    } // end of fn consume
} // end of impl InnerServerConsumer

impl InnerClientConsumer {
    fn new(
        log_ctx: Arc<AppLogContext>,
        dstore: Arc<AmqpClientRecvReply>,
        tag_postfix: String,
    ) -> Self {
        let _tag = Self::generate_tag(tag_postfix);
        Self {
            _tag,
            log_ctx,
            dstore,
        }
    }
    fn generate_tag(postfix: String) -> String {
        let thread_id = std::thread::current().id();
        let (timefmt, nsecs) = {
            let now = Local::now().fixed_offset();
            (now.to_rfc3339(), now.timestamp_subsec_nanos())
        };
        format!("client-{:?}-{}-{}-{}", thread_id, timefmt, nsecs, postfix)
    } // consumer-tag on message broker side has to uniquely identify a specific active
      // consumer, otherwise the broker might respond with ambiguous error (RabbitMQ)
    fn tag(&self) -> &String {
        &self._tag
    }

    async fn _consume(
        &mut self,
        resp_props: BasicProperties,
        content: Vec<u8>,
    ) -> DefaultResult<(), AppError> {
        if let Some(corr_id) = resp_props.correlation_id() {
            self.dstore.update(corr_id.as_str(), content).await?;
            Ok(())
        } else {
            let detail = Some("missing-correlation-id".to_string());
            Err(AppError {
                code: AppErrorCode::RpcConsumeFailure,
                detail,
            })
        }
    }
} // end of impl InnerClientConsumer

#[async_trait]
impl AsyncConsumer for InnerClientConsumer {
    async fn consume(
        &mut self,
        _channel: &Channel,
        deliver: Deliver,
        basic_properties: BasicProperties,
        content: Vec<u8>,
    ) {
        let log_ctx_p = self.log_ctx.clone();
        let route_key_log = deliver.routing_key().clone();
        let corr_id_nonexist_log = "none".to_string();
        let corr_id_log = basic_properties
            .correlation_id()
            .unwrap_or(&corr_id_nonexist_log)
            .clone();
        app_log_event!(
            log_ctx_p,
            AppLogLevel::DEBUG,
            "route:{route_key_log}, \
                       corr-id:{corr_id_log}"
        );
        let result = self._consume(basic_properties, content).await;
        if let Err(e) = result {
            app_log_event!(
                log_ctx_p,
                AppLogLevel::ERROR,
                "route:{route_key_log}, \
                        corr-id:{corr_id_log}, error: {:?}",
                e
            );
        }
    }
} // end of impl InnerClientConsumer

impl Default for AmqpClientRecvReply {
    fn default() -> Self {
        Self(Mutex::new(HashMap::new()))
    }
}
impl AmqpClientRecvReply {
    async fn claim(&self, key: &str) -> DefaultResult<(), AppError> {
        const MAX_SAVED_LIMIT: usize = 100; // TODO, parameterize
        let mut guard = self.0.lock().await;
        if guard.len() < MAX_SAVED_LIMIT {
            if guard.contains_key(key) {
                let detail = format!("rpc-reply-store, claim-duplicate, key:{key}");
                Err(AppError {
                    code: AppErrorCode::InvalidInput,
                    detail: Some(detail),
                })
            } else {
                let result = guard.insert(key.to_string(), None);
                assert!(result.is_none());
                Ok(())
            }
        } else {
            let detail = format!(
                "rpc-reply-store, claim, actual:{}, limit:{}",
                guard.len(),
                MAX_SAVED_LIMIT
            );
            Err(AppError {
                code: AppErrorCode::ExceedingMaxLimit,
                detail: Some(detail),
            })
        }
    } // TODO, discard hash-map entries if nobody fetches them for a long time

    async fn update(
        &self,
        key: &str,
        content: Vec<u8>,
    ) -> DefaultResult<Option<Vec<u8>>, AppError> {
        let mut guard = self.0.lock().await;
        if let Some(entry) = guard.get_mut(key) {
            let prev = entry.take();
            *entry = Some(content);
            Ok(prev)
        } else {
            let detail = format!("rpc-reply-store, fetch-non-exist, key:{key}");
            Err(AppError {
                code: AppErrorCode::InvalidInput,
                detail: Some(detail),
            })
        }
    }

    async fn fetch(&self, key: &str) -> DefaultResult<Vec<u8>, AppError> {
        let guard = self.0.lock().await;
        if let Some(entry) = guard.get(key) {
            if let Some(content) = entry {
                Ok(content.clone())
            } else {
                let detail = format!("rpc-reply-store, in-progress, key:{key}");
                Err(AppError {
                    code: AppErrorCode::RpcReplyNotReady,
                    detail: Some(detail),
                })
            }
        } else {
            let detail = format!("rpc-reply-store, fetch-non-exist, key:{key}");
            Err(AppError {
                code: AppErrorCode::InvalidInput,
                detail: Some(detail),
            })
        }
    }
} // end of impl AmqpClientRecvReply
