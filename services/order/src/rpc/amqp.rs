use std::result::Result as DefaultResult;
use std::boxed::Box;
use std::sync::Arc;
use async_trait::async_trait;

use serde::Deserialize;
use amqprs::{BasicProperties, FieldTable, FieldValue};
use amqprs::connection::{OpenConnectionArguments, Connection as AmqpConnection};
use amqprs::channel::{
    Channel, QueueDeclareArguments, QueueBindArguments, ConfirmSelectArguments,
    BasicPublishArguments
};
use amqprs::callbacks::{DefaultConnectionCallback, DefaultChannelCallback};
use amqprs::error::Error as AmqpError;
use tokio::sync::Mutex;

use crate::generate_custom_uid;
use crate::confidentiality::AbstractConfidentiality;
use crate::config::{AppRpcAmqpCfg, AppAmqpBindingCfg, AppAmqpBindingReplyCfg};
use crate::constant::{HTTP_CONTENT_TYPE_JSON, app_meta};
use crate::error::{AppError, AppErrorCode};
use super::{
    AbsRpcClientCtx, AbstractRpcContext, AbstractRpcClient, AppRpcClientReqProperty,
    AppRpcReply, AbsRpcServerCtx, AbstractRpcServer
};

#[derive(Deserialize)]
struct BrokerSecret {
    host    : String,
    port    : u16,
    username: String,
    password: String
}
        
// TODO, configurable parameters
const MAX_NUM_MSGS : i32 = 1300;
const MSG_TTL_MILLISEC : i32 = 33000;

pub(super) struct AmqpRpcContext {
    // currently use single connection, will swtich to
    // existing pool like the crate `deadpool` (TODO)
    inner_conn: Mutex<Option<AmqpConnection>>,
    conn_opts: OpenConnectionArguments,
	bindings: Arc<Vec<AppAmqpBindingCfg>>
}
pub(super) struct AmqpRpcHandler {
	bindings: Arc<Vec<AppAmqpBindingCfg>>,
    chosen_bind_idx: Option<usize>,
    channel: Channel,
    reconnected: bool,
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
        AppError { code, detail: Some(detail) }
    }
}


#[async_trait]
impl AbsRpcClientCtx for AmqpRpcContext {
    async fn acquire (&self, num_retry:u8)
        -> DefaultResult<Box<dyn AbstractRpcClient>, AppError>
    {
        let mut result = Err(AppError { code: AppErrorCode::Unknown,
                         detail: Some(format!("AbsRpcClientCtx::acquire, AmqpRpcContext")) });
        for _ in 0..num_retry {
            result = self.get_channel().await;
            if result.is_ok() { break; }
        }
        let (channel, reconnected) = result?;
        let obj = AmqpRpcHandler { channel, reconnected, chosen_bind_idx:None,
                  bindings:self.bindings.clone() };
        Ok(Box::new(obj))
    }
}
#[async_trait]
impl AbsRpcServerCtx for AmqpRpcContext {
    async fn acquire (&self, _num_retry:u8)
        -> DefaultResult<Box<dyn AbstractRpcServer>, AppError>
    {
        Err(AppError { code: AppErrorCode::NotImplemented, detail: None })
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
                         inner_conn: Mutex::new(None) };
        Ok(Box::new(obj))
    }

    async fn _create_conn(&self) -> DefaultResult<AmqpConnection, AppError>
    { // TODO, distinguish low-level network error and auth failure
        let c = AmqpConnection::open(&self.conn_opts).await?;
        c.register_callback(DefaultConnectionCallback).await?;
        assert!(c.is_open());
        Ok(c)
    }
    async fn _create_channel(conn:&AmqpConnection) -> DefaultResult<Channel, AppError>
    { // give null channel-ID, let broker randomly generate channel ID
        let channel = conn.open_channel(None).await?;
        channel.register_callback(DefaultChannelCallback).await?;
        assert!(channel.is_connection_open());
        assert!(channel.is_open());
        Ok(channel)
    }
    async fn get_channel(&self) -> DefaultResult<(Channel, bool), AppError>
    {
        let mut reconnected = false;
        let mut guard = self.inner_conn.lock().await;
        if guard.as_ref().is_none() {
            let c = self._create_conn().await?;
            (*guard, reconnected) = (Some(c), true);
        }
        if let Some(conn) = guard.as_ref() {
            match Self::_create_channel(&conn).await {
                Ok(chn) => Ok((chn, reconnected)),
                Err(e) =>
                    if conn.is_open() { Err(e.into()) }
                    else {
                        let conn = guard.take().unwrap();
                        let _result = conn.close().await; // drop and ignore any error
                        let conn = self._create_conn().await?;
                        (*guard, reconnected) = (Some(conn), true);
                        let conn = guard.as_ref().unwrap();
                        let chn = Self::_create_channel(&conn).await ?;
                        Ok((chn, reconnected))
                    }
            }
        } else {
            let d = "amqp-conn-new-fail".to_string();
            Err(AppError { code: AppErrorCode::DataCorruption, detail: Some(d) })
        }
    } // end of fn get_channel
} // end of impl AmqpRpcContext


#[async_trait]
impl AbstractRpcClient for AmqpRpcHandler {
    async fn send_request(mut self:Box<Self>, req:AppRpcClientReqProperty)
        -> DefaultResult<Box<dyn AbstractRpcClient>, AppError>
    {
        let (route, content, retry, t_start) = (req.route, req.msgbody,
                                                req.retry, req.start_time);
        let (idx, bind_cfg) = self.try_get_binding(route.as_str())?;
        let (reply_q_name, corr_id_prefix, reply_q_ttl) = if let AppAmqpBindingReplyCfg::client {
            queue, correlation_id_prefix, ttl_sec } = &bind_cfg.reply
        {
            (queue.as_str(), correlation_id_prefix.as_str(), ttl_sec.clone())
        } else {
            let d = format!("amqp-client-send-req, bind-reply-cfg");
            return Err(AppError { code:AppErrorCode::InvalidInput, detail: Some(d) });
        };
        if self.reconnected {
            if bind_cfg.ensure_declare {
                self.ensure_send_queue(bind_cfg).await?;
            }
            self.ensure_reply_queue(reply_q_name, bind_cfg.durable, reply_q_ttl).await?;
            // only RPC client needs to turn on confirm mode in the broker
            self.channel.confirm_select(ConfirmSelectArguments::new(false)).await?;
        }
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

#[async_trait]
impl AbstractRpcServer for AmqpRpcHandler {
    async fn send_response(mut self:Box<Self>, _props:AppRpcReply)
        -> DefaultResult<(), AppError>
    {
        Err(AppError { code: AppErrorCode::NotImplemented, detail: None })
    }

    async fn receive_request(&mut self)
        -> DefaultResult<AppRpcClientReqProperty, AppError>
    {
        Err(AppError { code: AppErrorCode::NotImplemented, detail: None })
    }
}

impl AmqpRpcHandler {
    fn try_get_binding(&self, route_key:&str) -> DefaultResult<(usize, &AppAmqpBindingCfg), AppError>
    {
        let result = self.bindings.iter().position(
            |bind_cfg| bind_cfg.routing_key.as_str() == route_key
        );
        if let Some(idx) = result {
            let bind_cfg = self.bindings.get(idx).unwrap();
            Ok((idx, bind_cfg))
        } else {
            let d = format!("binding-cfg-not-found, {route_key}");
            Err(AppError { code: AppErrorCode::InvalidInput, detail: Some(d) })
        }
    }
    async fn ensure_send_queue(&self, cfg:&AppAmqpBindingCfg)
        -> DefaultResult<(), AppError>
    {
        let mut properties = FieldTable::new();
        properties.insert("x-message-ttl".try_into().unwrap(), FieldValue::I(MSG_TTL_MILLISEC));
        properties.insert("x-max-length".try_into().unwrap(), FieldValue::I(MAX_NUM_MSGS));
        // note the flag `passive` only checks whether the queue exists,
        // the broker reports `OK` if exists or ambigious error if not.
        let args = QueueDeclareArguments::new(cfg.queue.as_str()).durable(cfg.durable)
            .passive(!cfg.ensure_declare).auto_delete(false).no_wait(false)
            .arguments(properties).finish();
        let result  = self.channel.queue_declare(args).await?;
        if let Some((_q_name, _msg_cnt, _consumer_cnt)) = result {
            // TODO, logging debug message
            //println!("[debug] queue-declare-ok: {_q_name}, {_msg_cnt}, {_consumer_cnt}");
        }
        let args = QueueBindArguments::new(cfg.queue.as_str(), cfg.exchange.as_str(),
                   cfg.routing_key.as_str()).no_wait(false).finish() ;
        self.channel.queue_bind(args).await?;
        Ok(())
    }
    async fn ensure_reply_queue(&self, queue:&str, durable:bool, ttl_sec:u16)
        -> DefaultResult<(), AppError>
    {
        let ttl_millis = ttl_sec as i32  * 1000;
        let mut properties = FieldTable::new();
        properties.insert("x-message-ttl".try_into().unwrap(), FieldValue::I(ttl_millis));
        properties.insert("x-max-length".try_into().unwrap(), FieldValue::I(MAX_NUM_MSGS));
        let args = QueueDeclareArguments::new(queue).durable(durable)
            .passive(false).auto_delete(false).no_wait(false)
            .arguments(properties).finish();
        let _result  = self.channel.queue_declare(args).await?;
        Ok(())
    }
} // end of impl AmqpRpcHandler 
