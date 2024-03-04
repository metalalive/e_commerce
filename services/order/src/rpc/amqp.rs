use std::result::Result as DefaultResult;
use std::boxed::Box;
use std::sync::Arc;
use async_trait::async_trait;

use serde::Deserialize;
use amqprs::{BasicProperties, FieldTable, FieldValue};
use amqprs::connection::{OpenConnectionArguments, Connection as AmqpConnection};
use amqprs::consumer::DefaultConsumer;
use amqprs::channel::{
    Channel, QueueDeclareArguments, QueueBindArguments, ConfirmSelectArguments,
    BasicPublishArguments
};
use amqprs::callbacks::{DefaultConnectionCallback, DefaultChannelCallback};
use amqprs::error::Error as AmqpError;
use tokio::sync::{Mutex, RwLock};

use crate::{generate_custom_uid, AppSharedState};
use crate::confidentiality::AbstractConfidentiality;
use crate::config::{AppRpcAmqpCfg, AppAmqpBindingCfg, AppAmqpBindingReplyCfg};
use crate::constant::{HTTP_CONTENT_TYPE_JSON, app_meta};
use crate::error::{AppError, AppErrorCode};
use super::{
    AbsRpcClientCtx, AbstractRpcContext, AbstractRpcClient, AppRpcClientReqProperty,
    AppRpcReply, AbsRpcServerCtx, AppRpcRouteHdlrFn
};

#[derive(Deserialize)]
struct BrokerSecret {
    host    : String,
    port    : u16,
    username: String,
    password: String
}

pub(super) struct AmqpRpcContext {
    // currently use single connection/channel for all publish/consume
    // operations, it will swtich to existing pool like the crate
    // `deadpool` (TODO)
    inner_conn: Mutex<Option<AmqpConnection>>,
    inner_chn:  RwLock<Option<Channel>>,
    conn_opts: OpenConnectionArguments,
	bindings: Arc<Vec<AppAmqpBindingCfg>>
}
struct AmqpRpcClientHandler {
	bindings: Arc<Vec<AppAmqpBindingCfg>>,
    chosen_bind_idx: Option<usize>,
    channel: Channel,
}

struct InnerServer {}

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
        AppError { code, detail: Some(detail) }
    }
}


#[async_trait]
impl AbsRpcClientCtx for AmqpRpcContext {
    async fn acquire (&self, num_retry:u8)
        -> DefaultResult<Box<dyn AbstractRpcClient>, AppError>
    {
        let (channel, _reconnected) = self.try_acquire_channel(num_retry).await ?;
        let obj = AmqpRpcClientHandler { channel, chosen_bind_idx:None,
                  bindings:self.bindings.clone() };
        Ok(Box::new(obj))
    }
}
#[async_trait]
impl AbsRpcServerCtx for AmqpRpcContext
{
    async fn server_start(
        &self, shr_state:AppSharedState, route_hdlr: AppRpcRouteHdlrFn
    ) -> DefaultResult<(), AppError> 
    {
        let (channel, reconnected) = self.try_acquire_channel(2).await ?;
        if reconnected {
            InnerServer::init(self.bindings.clone(), channel,
                              shr_state, route_hdlr).await ?;
        }
        // TODO, notufy to return
        Ok(())
    }
}

impl AbstractRpcContext for AmqpRpcContext {
    fn label(&self) -> &'static str { "AMQP" }
}

impl AmqpRpcContext {
    pub(crate) fn build (cfg: &AppRpcAmqpCfg, confidential: Arc<Box<dyn AbstractConfidentiality>>)
        -> DefaultResult<Box<dyn AbstractRpcContext>, AppError> 
    { // TODO, logging error message
        let serial = confidential.try_get_payload(cfg.confidential_id.as_str())?;
        let conn_opts = match serde_json::from_str::<BrokerSecret>(serial.as_str())
        {
            Ok(s) => OpenConnectionArguments::new(s.host.as_str(), s.port,
                s.username.as_str(), s.password.as_str())
                .virtual_host(cfg.attributes.vhost.as_str())
                .heartbeat(cfg.attributes.timeout_secs).finish() ,
            Err(e) => {
                let detail = e.to_string() + ", secret-parsing-error, source: AmqpRpcContext";
                return Err(AppError { code: AppErrorCode::InvalidJsonFormat, detail: Some(detail) });
            }
        };
        let obj = Self { conn_opts, bindings:cfg.bindings.clone(),
                    inner_conn: Mutex::new(None), inner_chn: RwLock::new(None) };
        Ok(Box::new(obj))
    }
    
    async fn try_acquire_channel (&self, num_retry:u8)
        -> DefaultResult<(Channel, bool), AppError>
    {
        let mut result = Err(AppError { code: AppErrorCode::Unknown,
                         detail: Some(format!("AbsRpcClientCtx::acquire, AmqpRpcContext")) });
        for _ in 0..num_retry {
            result = self.ensure_conn_channel().await;
            if result.is_ok() { break; }
        }
        let out = result?;
        Ok(out)
    }

    async fn _create_conn(&self) -> DefaultResult<AmqpConnection, AppError>
    { // TODO, distinguish low-level network error and auth failure
        let c = AmqpConnection::open(&self.conn_opts).await?;
        c.register_callback(DefaultConnectionCallback).await?;
        assert!(c.is_open());
        Ok(c)
    }
    
    async fn try_get_channel(&self) -> Option<Channel>
    {
        let guard = self.inner_chn.read().await;
        if let Some(chn) = guard.as_ref() {
            if chn.is_connection_open() && chn.is_open() {
                Some(chn.clone())
            } else { None }
        } else { None }
    }
    async fn ensure_channel(&self, conn:&AmqpConnection)
        -> DefaultResult<(Channel, bool), AppError>
    { // give null channel-ID, let broker randomly generate channel ID
        let result = self.try_get_channel().await;
        let reconnected = !result.is_some();
        let channel = if let Some(chn) = result {
            chn
        } else {
            let chn = conn.open_channel(None).await?;
            chn.register_callback(DefaultChannelCallback).await?;
            let mut guard = self.inner_chn.write().await;
            *guard = Some(chn.clone());
            chn
        };
        assert!(channel.is_connection_open());
        assert!(channel.is_open());
        Ok((channel, reconnected))
    }
    async fn ensure_conn_channel(&self) -> DefaultResult<(Channel, bool), AppError>
    {
        let mut guard = self.inner_conn.lock().await;
        if guard.as_ref().is_none() {
            let c = self._create_conn().await?;
            *guard = Some(c);
        }
        if let Some(conn) = guard.as_ref() {
            match self.ensure_channel(&conn).await {
                Ok((chn, reconnected)) => Ok((chn, reconnected)),
                Err(e) =>
                    if conn.is_open() { Err(e.into()) }
                    else {
                        let conn = guard.take().unwrap();
                        let _result = conn.close().await; // drop and ignore any error
                        let conn = self._create_conn().await?;
                        *guard = Some(conn);
                        let conn = guard.as_ref().unwrap();
                        let (chn, reconnected) = self.ensure_channel(&conn).await ?;
                        Ok((chn, reconnected))
                    }
            }
        } else {
            let d = "amqp-conn-new-fail".to_string();
            Err(AppError { code: AppErrorCode::DataCorruption, detail: Some(d) })
        }
    } // end of fn ensure_conn_channel
} // end of impl AmqpRpcContext


#[async_trait]
impl AbstractRpcClient for AmqpRpcClientHandler
{
    async fn send_request(mut self:Box<Self>, req:AppRpcClientReqProperty)
        -> DefaultResult<Box<dyn AbstractRpcClient>, AppError>
    {
        let (route, content, t_start) = (req.route, req.msgbody, req.start_time);
        let (idx, bind_cfg) = try_get_binding(self.bindings.as_ref(), route.as_str())?;
        let (reply_q_name, corr_id_prefix) = if let Some(r_cfg) = &bind_cfg.reply {
            (r_cfg.queue.as_str(), r_cfg.correlation_id_prefix.as_str())
        } else {
            let e = AppError { code: AppErrorCode::InvalidInput,
                    detail: Some("rpc-client-seq, empty-reply-cfg".to_string()) };
            return Err(e);
        }; // only RPC client needs to turn on confirm mode in the broker
        self.channel.confirm_select(ConfirmSelectArguments::new(false)).await?;
        let mut corr_id = generate_custom_uid(app_meta::MACHINE_CODE).into_bytes()
            .into_iter().map(|b| format!("{:02x}",b))
            .collect::<Vec<String>>().join("");
        corr_id.insert(0, '.');
        corr_id.insert_str(0, corr_id_prefix);
        let properties = BasicProperties::default().with_app_id(app_meta::LABAL)
            .with_content_type(HTTP_CONTENT_TYPE_JSON).with_content_encoding("utf-8")
            .with_persistence(bind_cfg.durable).with_reply_to(reply_q_name)
            .with_correlation_id(corr_id.as_str())
            .with_timestamp(t_start.timestamp() as u64).finish();
        let args = BasicPublishArguments::default().exchange(bind_cfg.exchange.clone()) 
            .routing_key(bind_cfg.routing_key.clone()) 
            // To create a responsive application, message broker has to return
            // unroutable message whenever the given routing key goes wrong.
            .mandatory(true)
            .immediate(false) // RabbitMQ v3 removed this flag, I always set it to false
                              // , the crate `amqp-rs` reserves this flag for backward
                              // compatibility
            .finish();
        self.channel.basic_publish(properties, content, args).await?;
        // update at the end , due to borrow / mutability constraint at compile time
        self.as_mut().chosen_bind_idx = Some(idx);
        Ok(self)
    } // end of fn send_request

    async fn receive_response(&mut self) -> DefaultResult<AppRpcReply, AppError>
    {
        Err(AppError { code: AppErrorCode::NotImplemented, detail: None })
    }
} // end of impl AbstractRpcClient for AmqpRpcHandler 


fn try_get_binding<'a,'b>(src:&'a Vec<AppAmqpBindingCfg>, route_key:&'b str)
    -> DefaultResult<(usize, &'a AppAmqpBindingCfg), AppError>
{
    let result = src.iter().position(
        |bind_cfg| bind_cfg.routing_key.as_str() == route_key
    );
    if let Some(idx) = result {
        let bind_cfg = src.get(idx).unwrap();
        Ok((idx, bind_cfg))
    } else {
        let d = format!("binding-cfg-not-found, {route_key}");
        Err(AppError { code: AppErrorCode::InvalidInput, detail: Some(d) })
    }
}


impl InnerServer {
    async fn init(
        bindings:Arc<Vec<AppAmqpBindingCfg>>, channel:Channel,
        shr_state:AppSharedState, route_hdlr: AppRpcRouteHdlrFn
    )  -> DefaultResult<(), AppError>
    {
        for bind_cfg in bindings.iter() {
            if bind_cfg.ensure_declare {
                Self::ensure_send_queue(&channel, bind_cfg).await?;
            }
            if let Some(r_cfg) = &bind_cfg.reply {
                Self::ensure_reply_queue(&channel, r_cfg).await?;
            }
            if bind_cfg.subscribe {
            }
        } // end of loop
        Ok(())
    }

    async fn ensure_send_queue(channel:&Channel, cfg:&AppAmqpBindingCfg)
        -> DefaultResult<(), AppError>
    {
        let ttl_millis = cfg.ttl_secs as i32  * 1000;
        let max_num_msgs = cfg.max_length as i32;
        let mut properties = FieldTable::new();
        properties.insert("x-message-ttl".try_into().unwrap(), FieldValue::I(ttl_millis));
        properties.insert("x-max-length".try_into().unwrap(), FieldValue::I(max_num_msgs));
        // note the flag `passive` only checks whether the queue exists,
        // the broker reports `OK` if exists or ambigious error if not.
        let args = QueueDeclareArguments::new(cfg.queue.as_str()).durable(cfg.durable)
            .passive(!cfg.ensure_declare).auto_delete(false).no_wait(false)
            .arguments(properties).finish();
        let result = channel.queue_declare(args).await?;
        if let Some((_q_name, _msg_cnt, _consumer_cnt)) = result {
            // TODO, logging debug message
            //println!("[debug] queue-declare-ok: {_q_name}, {_msg_cnt}, {_consumer_cnt}");
        }
        let args = QueueBindArguments::new(cfg.queue.as_str(), cfg.exchange.as_str(),
                   cfg.routing_key.as_str()).no_wait(false).finish() ;
        channel.queue_bind(args).await?;
        Ok(())
    } // end of fn ensure_send_queue

    async fn ensure_reply_queue(channel:&Channel, cfg:&AppAmqpBindingReplyCfg)
        -> DefaultResult<(), AppError>
    {
        let (queue, ttl_secs) = (cfg.queue.as_str(), cfg.ttl_secs);
        let ttl_millis = ttl_secs as i32  * 1000;
        let max_num_msgs = cfg.max_length as i32;
        let mut properties = FieldTable::new();
        properties.insert("x-message-ttl".try_into().unwrap(), FieldValue::I(ttl_millis));
        properties.insert("x-max-length".try_into().unwrap(), FieldValue::I(max_num_msgs));
        let args = QueueDeclareArguments::new(queue).durable(cfg.durable)
            .passive(false).auto_delete(false).no_wait(false)
            .arguments(properties).finish();
        let _result = channel.queue_declare(args).await?;
        Ok(())
    }
} // end of impl InnerServer

