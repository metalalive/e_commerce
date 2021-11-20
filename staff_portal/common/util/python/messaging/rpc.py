import uuid
import socket
from datetime import datetime, timedelta
from amqp.exceptions import ConsumerCancelled
from kombu import Exchange as KombuExchange, Queue as KombuQueue
from kombu.exceptions import OperationalError as KombuOperationalError

from common.util.python import _get_amqp_url
from .amqp import AMQPPublisher, AMQPQueueConsumer, get_connection, UndeliverableMessage
from .constants import MSG_PAYLOAD_DEFAULT_CONTENT_TYPE, AMQP_SSL_CONFIG_KEY, SERIALIZER_CONFIG_KEY, DEFAULT_SERIALIZER, AMQP_EXCHANGE_NAME_CONFIG_KEY, AMQP_EXCHANGE_TYPE_CONFIG_KEY, RPC_EXCHANGE_DEFAULT_NAME,  RPC_EXCHANGE_DEFAULT_TYPE, RPC_ROUTE_KEY_PATTERN_SEND

ROUTE_KEY_PATTERN_REPLYTO = 'rpc.reply.%s'
RPC_REPLY_QUEUE_TTL = 30000  # ms (30 seconds)
RPC_DEFAULT_TASK_PATH_PATTERN = '%s.async_tasks.%s'

_default_msg_broker_url = _get_amqp_url(secrets_path="./common/data/secrets.json")


def get_rpc_exchange(config):
    ex_name = config.get(AMQP_EXCHANGE_NAME_CONFIG_KEY, RPC_EXCHANGE_DEFAULT_NAME)
    ex_type = config.get(AMQP_EXCHANGE_TYPE_CONFIG_KEY, RPC_EXCHANGE_DEFAULT_TYPE)
    exchange = KombuExchange(ex_name, durable=False, type=ex_type)
    return exchange


class ReplyListener:
    _default_config = {}
    queue_consumer = AMQPQueueConsumer(amqp_uri=_default_msg_broker_url)

    def __init__(self, config=None):
        self._reply_events = {}
        self._config = self._default_config.copy()
        if config:
            self._config.update(config)
        reply_q_uuid = uuid.uuid4()
        # here RPC server side (see celery rpc backend) defaults to anon-exchange 
        # which means :
        # * RPC server side will bypass the exchange, and reply with the result
        #    directly to the queue (whose name is given in reply_to)
        # * no need to create exchange and extra bindings for the reply queue
        # * routing key (reply_to) has to be the same as the name of the reply queue
        self.routing_key = ROUTE_KEY_PATTERN_REPLYTO % (reply_q_uuid)
        queue_name = self.routing_key
        self.queue = KombuQueue(
            queue_name,
            routing_key=self.routing_key,
            auto_delete=True,
            queue_arguments={'x-expires': RPC_REPLY_QUEUE_TTL}
        )
        self.queue_consumer.register_provider(self) # declare the reply queue

    def __del__(self):
        self.queue_consumer.unregister_provider(self) # delete the reply queue

    def get_reply_event(self, correlation_id, timeout_s=20):
        reply_event = RpcReplyEvent(listener=self, timeout_s=timeout_s)
        self._reply_events[correlation_id] = reply_event
        return reply_event

    def refresh_reply_events(self, num_of_msgs_fetch=None, timeout=0.5):
        # run consumer code, if there's no message in the queue then
        # immediately return back, let application callers decide when
        # to run this function again next time...
        # Note that `num_of_msgs_fetch`  defaults to None , which  means
        # unlimited number of messages to retrieve
        try:
            for _ in self.queue_consumer.consume(limit=num_of_msgs_fetch, timeout=timeout):
                pass
        except socket.timeout:
            print('timeout caught at ReplyListener.refresh_reply_events')
            # TODO: log warning
        except ConsumerCancelled:
            print('consumer cancelled in the middle of ReplyListener.refresh_reply_events')

    def handle_message(self, body, message):
        correlation_id = message.properties.get('correlation_id')
        reply_event = self._reply_events.get(correlation_id, None)
        if reply_event is not None:
            reply_event.send(body=body)
            if reply_event.finished:
                self._reply_events.pop(correlation_id, None)
        else:
            # TODO, log error 
            print("Unknown correlation id on consume message: %s" % correlation_id)
        self.queue_consumer.ack_message(message)



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

    def __init__(self, listener, timeout_s):
        self._listener = listener
        self._timeout_s = timedelta(seconds=timeout_s)
        self._time_deadline = datetime.utcnow() + self._timeout_s
        self.resp_body = {'status': self.status_opt.INITED, 'result': None, 'timeout':False}

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

    def refresh(self, retry=False, num_of_msgs_fetch=None, timeout=0.5):
        """
        The typical approach is to run consumer code in different thread,
        in this system, application callers take turn to determine when
        to run consumer code.
        The result might still be empty at which caller runs this function
        if the listener hasn't received any message associated with this event.
        """
        _timeout = self.timeout
        if retry is True and _timeout is True:
            self._time_deadline = datetime.utcnow() + self._timeout_s
            _timeout = False
        if _timeout is False and self.finished is False:
            self._listener.refresh_reply_events(num_of_msgs_fetch=num_of_msgs_fetch, \
                    timeout=timeout)
            _timeout = self.timeout
            # TODO, report timeout error
        self.resp_body['timeout'] = _timeout

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
    # TODO, the listener shoud be created for each http connection,
    # not each amqp connection, not system initialization
    _rpc_reply_listener = ReplyListener()

    def __init__(self, dst_app_name, src_app_name, **options):
        self._dst_app_name = dst_app_name
        self._src_app_name = src_app_name
        self._options = options

    def __getattr__(self, name):
        return MethodProxy(
                dst_app_name=self._dst_app_name,
                src_app_name=self._src_app_name,
                method_name=name,
                reply_listener=self._rpc_reply_listener,
                **self._options
            )

class MethodProxy:
    publisher_cls = AMQPPublisher

    def __init__(self, dst_app_name, src_app_name, method_name, reply_listener, **options):
        self._dst_app_name = dst_app_name
        self._src_app_name = src_app_name
        self._method_name = method_name
        self._reply_listener = reply_listener
        self._config = options.pop('config', {})
        serializer = options.pop('serializer', self.serializer)
        self._options = options
        self._publisher = self.publisher_cls(
            amqp_uri=_default_msg_broker_url,
            serializer=serializer, ssl=self.ssl, **options
        )

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
        # async call only
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
        reply_event = self._reply_listener.get_reply_event(correlation_id)
        deliver_err_body = {'result': {'exchange': exchange.name, 'routing_key': routing_key, }}
        try:
            with get_connection(amqp_uri=_default_msg_broker_url, ssl=self.ssl) as conn:
                self._reply_listener.queue_consumer.declare(conn=conn)
                self._publisher.publish(
                    payload=payload,
                    exchange=exchange,
                    routing_key=routing_key,
                    mandatory=True,
                    #immediate=True,
                    reply_to=reply_to,
                    correlation_id=correlation_id,
                    extra_headers=context,
                    conn=conn
                )
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
        out = {}
        out['id'] = id
        out['content_type'] = content_type
        out['task'] = RPC_DEFAULT_TASK_PATH_PATTERN % (self._dst_app_name, self._method_name)
        out['headers'] = {'src_app':src_app} # AMQP message headers
        return out
## end of class MethodProxy

