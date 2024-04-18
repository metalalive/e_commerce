import uuid
import socket
import logging
from datetime import datetime, timedelta
from typing import Dict, Optional
from amqp.exceptions import ConsumerCancelled, NotFound as AmqpNotFound
from kombu import Exchange as KombuExchange, Queue as KombuQueue
from kombu.exceptions import OperationalError as KombuOperationalError

from ecommerce_common.util  import _get_amqp_url
from .amqp import AMQPPublisher, AMQPQueueConsumer, get_connection, UndeliverableMessage
from .constants import MSG_PAYLOAD_DEFAULT_CONTENT_TYPE, AMQP_SSL_CONFIG_KEY, SERIALIZER_CONFIG_KEY, DEFAULT_SERIALIZER, AMQP_EXCHANGE_NAME_CONFIG_KEY, AMQP_EXCHANGE_TYPE_CONFIG_KEY, RPC_EXCHANGE_DEFAULT_NAME,  RPC_EXCHANGE_DEFAULT_TYPE, RPC_ROUTE_KEY_PATTERN_SEND

ROUTE_KEY_PATTERN_REPLYTO = 'rpc.reply.%s.%s'
RPC_REPLY_MSG_TTL = 25000  # ms (25 seconds)
RPC_DEFAULT_TASK_PATH_PATTERN = '%s.async_tasks.%s'

_logger = logging.getLogger(__name__)


def get_rpc_exchange(config:Dict):
    ex_name = config.get(AMQP_EXCHANGE_NAME_CONFIG_KEY, RPC_EXCHANGE_DEFAULT_NAME)
    ex_type = config.get(AMQP_EXCHANGE_TYPE_CONFIG_KEY, RPC_EXCHANGE_DEFAULT_TYPE)
    exchange = KombuExchange(ex_name, durable=False, type=ex_type)
    return exchange


class ReplyListener:
    _num_objs_created = 0 
    # Currently there is only one consumer per application, if the application
    # needs to scale, different consumers with different broker setups will
    # be required (TODO)
    queue_consumer: Optional[AMQPQueueConsumer] = None

    def __init__(self, broker_url:str, dst_app_label:str, src_app_label:str,
            msg_ttl:int = RPC_REPLY_MSG_TTL ):
        cls = type(self)
        if cls._num_objs_created == 0:
            cls.queue_consumer = AMQPQueueConsumer(amqp_uri=broker_url)
        cls._num_objs_created += 1 # TODO, lock required in async tasks
        self._reply_events = {}
        self._id = '{0}:{1}'.format(dst_app_label, src_app_label)
        self._q_created = False
        reply_q_uuid = uuid.uuid4()
        # here RPC server side (see celery rpc backend) defaults to anon-exchange 
        # which means :
        # * RPC server side will bypass the exchange, and reply with the result
        #    directly to the queue (whose name is given in reply_to)
        # * no need to create exchange and extra bindings for the reply queue
        # * routing key (reply_to) has to be the same as the name of the reply queue
        self.routing_key = ROUTE_KEY_PATTERN_REPLYTO % (src_app_label, reply_q_uuid)
        queue_name = self.routing_key
        log_args = ['type', 'python.messaging.rpc.ReplyListener',
                'reply_queue', queue_name, 'msg_ttl_secs', str(msg_ttl)]
        _logger.debug(None, *log_args)
        # `auto-delete` property ensure unused queue is removed after last
        # consumer unsubscribed, In this project I consider the internet
        # might be unstable in both sides of producer / consumer, so always
        # set this to false.
        self.queue = KombuQueue( name=queue_name, routing_key=self.routing_key,
            queue_arguments={'x-message-ttl': msg_ttl},  auto_delete=False )
        self.queue_consumer.register_provider(self)

    def destroy(self):
        self.queue_consumer.unregister_provider(self)
        self.queue_consumer.undeclare(label=self._id)

    @property
    def identity(self):
        return self._id
 
    def declare_queue(self, conn):
        if not self._q_created:
            self._q_created = self.queue_consumer.declare(conn=conn, label=self._id)

    def get_reply_event(self, correlation_id, timeout_s=10):
        reply_event = RpcReplyEvent(listener=self, timeout_s=timeout_s,
                corr_id=correlation_id )
        self._reply_events[correlation_id] = reply_event
        return reply_event

    def refresh_reply_events(self, num_of_msgs_fetch=None, timeout:float=0.5):
        # run consumer code, if there's no message in the queue then
        # immediately return back, let application callers decide when
        # to run this function again next time...
        # Note that `num_of_msgs_fetch`  defaults to None , which  means
        # unlimited number of messages to retrieve
        err = None
        try:
            for _ in self.queue_consumer.consume(limit=num_of_msgs_fetch,
                    timeout=timeout):
                pass
        except socket.timeout as e:
            log_args = ['action','listener-update-events','msg', e.args[0],
                    'timeout', str(timeout), 'num_of_msgs_fetch', num_of_msgs_fetch]
            _logger.warning(None, *log_args)
            err = e
        except AmqpNotFound as e:
            # this happenes when queue was deleted and time-to-live of the
            # queue (TTL, a.k.a `x-expires` in RabbitMQ) was enabled, 
            log_args = ['method_name', e.method_name, 'msg', e.message
                    , 'rpc_reply_status', str(e.reply_code)]
            _logger.error(None, *log_args)
            err = e
        except ConsumerCancelled as e:
            log_args = ['msg', str(e.args)]
            _logger.error(None, *log_args)
            err = e
        return err

    def handle_message(self, body, message):
        correlation_id:str = message.properties.get('correlation_id')
        reply_event = self._reply_events.get(correlation_id, None)
        if reply_event is not None:
            reply_event.send(body=body)
            if reply_event.finished:
                self._reply_events.pop(correlation_id, None)
        else:
            log_args = ['msg', 'Unknown correlation id', 'correlation_id', correlation_id]
            _logger.warning(None, *log_args)
        self.queue_consumer.ack_message(message)
## end of class ReplyListener



class RpcReplyEvent:
    class status_opt:
        INITED  = 'INITED'
        STARTED = 'STARTED'
        SUCCESS = 'SUCCESS'
        FAIL_CONN = 'FAIL_CONN'
        FAIL_PUBLISH = 'FAIL_PUBLISH'
        REMOTE_ERROR = 'FAILURE' # focus on errors from remote execution
        INVALID_STATUS_TRANSITION = 'INVALID_STATUS_TRANSITION'

    valid_status_transitions = [
        # published successfully and remote server already received the RPC request
        (status_opt.INITED , status_opt.STARTED),
        # failed to publish due to connectivity issues to AMQP broker
        (status_opt.INITED , status_opt.FAIL_CONN),
        # failed to publish due to errors received from RPC server
        (status_opt.INITED , status_opt.FAIL_PUBLISH),
        # remote server completed the task and return result back successfully
        (status_opt.STARTED , status_opt.SUCCESS),
        # remote server reported error in the middle of processing the RPC request
        (status_opt.STARTED , status_opt.REMOTE_ERROR),
    ]

    valid_finish_status = [status_opt.FAIL_CONN, status_opt.FAIL_PUBLISH,
            status_opt.REMOTE_ERROR, status_opt.SUCCESS]

    def __init__(self, listener, timeout_s, corr_id:str=''):
        self._listener = listener
        self._timeout_s = timedelta(seconds=timeout_s)
        self._time_deadline = datetime.utcnow() + self._timeout_s
        self.resp_body = {'status': self.status_opt.INITED, 'result': None,
                'timeout':False, 'corr_id':corr_id}

    def send(self, body):
        """ validate state transition and update result """
        old_status = self.resp_body['status']
        new_status = body.get('status', None)
        if (old_status, new_status) in self.valid_status_transitions:
            self.resp_body['status'] = new_status
            self.resp_body['result'] = body.get('result', None)
        else:
            self.resp_body['status'] = self.status_opt.INVALID_STATUS_TRANSITION
            old_result = self.resp_body['result']
            self.resp_body['result'] =  {'old_result': old_result,'new_message':body}
        extra_err = body.get('error', None)
        if extra_err:
            self.resp_body['error'] = extra_err

    def refresh(self, retry=False, num_of_msgs_fetch=None, timeout=0.5):
        """
        The typical approach is to run consumer code in different thread,
        in this system, application callers take turn to determine when
        to run consumer code.
        The result might still be empty at which caller runs this function
        if the listener hasn't received any message associated with this event.
        """
        err = None
        _timeout = self.timeout
        if retry is True and _timeout is True:
            self._time_deadline = datetime.utcnow() + self._timeout_s
            _timeout = False
        if _timeout is False and self.finished is False:
            err = self._listener.refresh_reply_events(num_of_msgs_fetch=num_of_msgs_fetch, \
                    timeout=timeout)
            _timeout = self.timeout
            # TODO, report timeout error
        self.resp_body['timeout'] = _timeout
        return err

    @property
    def result(self):
        return self.resp_body.copy()

    @property
    def finished(self):
        return  self.resp_body['status'] in self.valid_finish_status

    @property
    def timeout(self):
        time_now = datetime.utcnow()
        return self._time_deadline < time_now
# end of class RpcReplyEvent


class RPCproxy:
    """
    Each RPC proxy object has independent listener in case several message brokers
    are applied to one single application.
    """
    def __init__(self, dst_app_name:str, src_app_name:str, **options):
        # TODO, parameterize
        _default_msg_broker_url = _get_amqp_url(secrets_path="./common/data/secrets.json")
        self._dst_app_name = dst_app_name
        self._src_app_name = src_app_name
        self._rpc_reply_listener = ReplyListener( broker_url=_default_msg_broker_url,
                dst_app_label=dst_app_name, src_app_label=src_app_name )
        self._options = options
        self._options.update({'broker_url':_default_msg_broker_url})
    
    def __del__(self):
        listener = self._rpc_reply_listener
        self._rpc_reply_listener = None
        del self._rpc_reply_listener
        listener.destroy()

    def __getattr__(self, name):
        return MethodProxy(
                dst_app_name=self._dst_app_name,
                src_app_name=self._src_app_name,
                method_name=name,
                reply_listener=self._rpc_reply_listener,
                **self._options )

class MethodProxy:
    publisher_cls = AMQPPublisher

    def __init__(self, dst_app_name:str, src_app_name:str, method_name:str,
            broker_url:str, reply_listener, enable_confirm:Optional[bool]=None, **options):
        self._dst_app_name = dst_app_name
        self._src_app_name = src_app_name
        self._method_name = method_name
        self._broker_url = broker_url
        self._reply_listener = reply_listener
        self.enable_confirm = enable_confirm # each published message has its own setup
        self._config = options.pop('config', {})
        serializer = options.pop('serializer', self.serializer)
        self._options = options
        self._publisher = self.publisher_cls( amqp_uri=broker_url,
                serializer=serializer, ssl=self.ssl, **options )

    @property
    def ssl(self):
        return self._config.get(AMQP_SSL_CONFIG_KEY, None)

    @property
    def serializer(self):
        """ Default serializer to use when publishing message payloads.
        Must be registered as a
        `kombu serializer <http://bit.do/kombu_serialization>`_.
        """
        return self._config.get(SERIALIZER_CONFIG_KEY, DEFAULT_SERIALIZER)

    def __call__(self, *args, **kwargs):
        reply = self._call(*args, **kwargs)
        return reply

    def _call(self, *args, **kwargs):
        # TODO, figure out how Celery uses the metadata section
        payld_metadata = {'callbacks': None, 'errbacks': None, 'chain': None, 'chord': None}
        payload = [args, kwargs, payld_metadata]
        exchange = get_rpc_exchange(self._config)
        routing_key = RPC_ROUTE_KEY_PATTERN_SEND % (self._dst_app_name, self._method_name)
        reply_to =  self._reply_listener.routing_key
        correlation_id = str(uuid.uuid4())
        context = self.get_message_context(id=correlation_id, src_app=self._src_app_name)
        reply_event = self._reply_listener.get_reply_event(correlation_id, \
                timeout_s=self._options.get('reply_timeout_sec', 5) )
        deliver_err_body = {'result': {'exchange': exchange.name, 'routing_key': routing_key, }}
        try:
            extra_transport_opts = {}
            if self.enable_confirm is not None:
                extra_transport_opts['confirm_publish'] = self.enable_confirm
            with get_connection(amqp_uri=self._broker_url, ssl=self.ssl,
                    transport_options=extra_transport_opts ) as conn:
                self._reply_listener.declare_queue(conn=conn)
                result = self._publisher.publish(
                    payload=payload,
                    exchange=exchange,
                    routing_key=routing_key,
                    mandatory=True,
                    #immediate=True,
                    reply_to=reply_to,
                    correlation_id=correlation_id,
                    extra_headers=context,
                    conn=conn )
            if self.enable_confirm is True and result.ready is False:
                raise UndeliverableMessage(exchange=exchange, routing_key=routing_key)
        except UndeliverableMessage as ume:
            deliver_err_body['status'] = reply_event.status_opt.FAIL_PUBLISH
            deliver_err_body['error']  = ', '.join(ume.args)
            reply_event.send(body=deliver_err_body)
        except KombuOperationalError as k_op_e:
            entire_err_msg = ', '.join(k_op_e.args)
            if 'Errno 111' in entire_err_msg: # rabbitmq AMQP broker goes down
                deliver_err_body['status'] = reply_event.status_opt.FAIL_CONN
                deliver_err_body['error']  = entire_err_msg
                reply_event.send(body=deliver_err_body)
            else:
                raise
        return reply_event

    def get_message_context(self, id, src_app, content_type=MSG_PAYLOAD_DEFAULT_CONTENT_TYPE):
        # in this project , the RPC services are managed by Celery, which reference
        # extra headers :
        # * id : uuid4  string sequence
        # * task : python hierarchical path to Celery task function 
        # https://docs.celeryq.dev/en/master/internals/protocol.html
        out = {}
        out['id'] = id
        out['content_type'] = content_type
        out['task'] = RPC_DEFAULT_TASK_PATH_PATTERN % (self._dst_app_name, self._method_name)
        out['headers'] = {'src_app':src_app} # AMQP message headers
        return out
## end of class MethodProxy

