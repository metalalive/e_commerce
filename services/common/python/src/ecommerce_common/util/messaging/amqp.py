import socket
import logging
from functools import partial
from contextlib import contextmanager
from typing import Optional

import kombu
from kombu import Producer as KombuProducer
from kombu.exceptions import ChannelError
from kombu.pools import (
    connections as KombuConnectionPool,
    producers as KombuProducerPool,
)
from kombu.mixins import ConsumerMixin as KombuConsumerMixin
from amqp import ConsumerCancelled

from ecommerce_common.logging.util import log_fn_wrapper
from .constants import (
    AMQP_SSL_CONFIG_KEY,
    AMQP_DEFAULT_CONSUMER_ACCEPT_TYPES,
    AMQP_DEFAULT_HEARTBEAT,
    AMQP_HEARTBEAT_CONFIG_KEY,
    AMQP_TRANSPORT_OPTIONS_CONFIG_KEY,
    AMQP_DEFAULT_TRANSPORT_OPTIONS,
    AMQP_DEFAULT_RETRY_POLICY,
    amqp_delivery_mode,
)

_logger = logging.getLogger(__name__)


@contextmanager
def get_connection(
    amqp_uri: Optional[str] = None,
    ssl=None,
    block: bool = False,
    timeout=None,
    transport_options: Optional[dict] = None,
    conn=None,
):
    if conn is None:
        assert amqp_uri, "invalid URL for AMQP broker"
        applied_tx_opts = AMQP_DEFAULT_TRANSPORT_OPTIONS.copy()
        if transport_options:
            applied_tx_opts.update(transport_options)
        conn = kombu.Connection(amqp_uri, transport_options=applied_tx_opts, ssl=ssl)
    else:
        if transport_options:
            conn.transport_options.update(transport_options)
    target_pool = KombuConnectionPool[conn]
    if (
        conn is target_pool.connection
        or getattr(conn, "_acquired_from_pool", None) is True
    ):
        yield conn  # the given connection already comes from the pool, no need to acquire
    else:
        with target_pool.acquire(block=block, timeout=timeout) as connection:
            yield connection


@contextmanager
def get_producer(conn, block=False, on_return=None):
    # conn has to come from kombu connection pool
    # TODO, how to check this property ?
    with KombuProducerPool[conn].acquire(block=block) as producer:
        producer.on_return = on_return
        yield producer
    # producer = kombu.Producer(channel=conn_from_pool.default_channel)


class AMQPPublisher:
    """
    Utility helper for publishing messages to RabbitMQ.
    """

    transport_options = AMQP_DEFAULT_TRANSPORT_OPTIONS.copy()
    """
    A dict of additional connection arguments to pass to alternate kombu
    channel implementations. Consult the transport documentation for
    available options.
    """
    delivery_mode = amqp_delivery_mode.PERSISTENT.value
    """
    Default delivery mode for messages published by this Publisher.
    """
    mandatory = False
    immediate = False
    """
    Require `mandatory <https://www.rabbitmq.com/amqp-0-9-1-reference.html
    #basic.publish.mandatory>`_ delivery for published messages.
    """
    priority = 0
    """
    Priority value for published messages, to be used in conjunction with
    `consumer priorities <https://www.rabbitmq.com/priority.html>_`.
    """
    expiration = None
    """
    `Per-message TTL <https://www.rabbitmq.com/ttl.html>`_, in milliseconds.
    """
    serializer = "json"
    """ Name of the serializer to use when publishing messages.
    Must be registered as a
    `kombu serializer <http://bit.do/kombu_serialization>`_.
    """
    compression = None
    """ Name of the compression to use when publishing messages.
    Must be registered as a
    `kombu compression utility <http://bit.do/kombu-compression>`_.
    """
    retry = True
    """
    Enable automatic retries when publishing a message that fails due
    to a connection error.
    Retries according to :attr:`self.retry_policy`.
    """
    retry_policy = AMQP_DEFAULT_RETRY_POLICY
    """
    Policy to apply when retrying message publishes, if requested.
    See :attr:`self.retry`.
    """
    declare = []
    """
    Kombu :class:`~kombu.messaging.Queue` or :class:`~kombu.messaging.Exchange`
    objects to (re)declare before publishing a message.
    """

    def __init__(
        self,
        amqp_uri,
        serializer=None,
        compression=None,
        delivery_mode=None,
        mandatory=None,
        priority=None,
        expiration=None,
        declare=None,
        retry=None,
        retry_policy=None,
        ssl=None,
        **publish_kwargs
    ):
        self.amqp_uri = amqp_uri
        self.ssl = ssl
        if delivery_mode is not None:  # delivery options
            self.delivery_mode = delivery_mode
        if mandatory is not None:
            self.mandatory = mandatory
        if priority is not None:
            self.priority = priority
        if expiration is not None:
            self.expiration = expiration
        if serializer is not None:  # message options
            self.serializer = serializer
        if compression is not None:
            self.compression = compression
        if retry is not None:  # retry policy
            self.retry = retry
        if retry_policy is not None:
            self.retry_policy = retry_policy
        if declare is not None:  # declarations
            self.declare = declare
        # other publish arguments
        self.publish_kwargs = publish_kwargs

    @log_fn_wrapper(logger=_logger, loglevel=logging.INFO)
    def publish(self, payload, exchange, routing_key, conn=None, **kwargs):
        """
        Note :
        RabbitMQ doesn't seem reliable on mandatory flag, so Kombu producer will
        receive basic.return payload ONLY in every other publish operation, which
        is problematic.
        By giving return callback on publish, kombu producer can receive basic.return
        payload in almost every publish operation except the first publish since the
        system started ... (still not perfect solution for old version of RabbitMQ)
        """
        publish_kwargs = self.publish_kwargs.copy()
        # merge headers from when the publisher was instantiated
        # with any provided now; "extra" headers always win
        headers = publish_kwargs.pop("headers", {}).copy()
        headers.update(kwargs.pop("headers", {}))
        headers.update(kwargs.pop("extra_headers", {}))
        transport_options = kwargs.pop("transport_options", self.transport_options)
        delivery_mode = kwargs.pop("delivery_mode", self.delivery_mode)
        mandatory = kwargs.pop("mandatory", self.mandatory)
        # immediate = kwargs.pop('immediate', self.immediate)
        priority = kwargs.pop("priority", self.priority)
        expiration = kwargs.pop("expiration", self.expiration)
        serializer = kwargs.pop("serializer", self.serializer)
        compression = kwargs.pop("compression", self.compression)
        retry = kwargs.pop("retry", self.retry)
        retry_policy = kwargs.pop("retry_policy", self.retry_policy)

        declare = self.declare[:]
        declare.extend(kwargs.pop("declare", ()))
        publish_kwargs.update(kwargs)  # remaining publish-time kwargs win

        result = None
        try:
            _get_conn_kwargs = {
                "conn": conn,
                "block": False,
                "timeout": 2.0,
                "transport_options": transport_options,
            }
            if conn is None:
                _get_conn_kwargs.update({"amqp_uri": self.amqp_uri, "ssl": self.ssl})
            with get_connection(**_get_conn_kwargs) as conn_from_pool:
                with get_producer(conn=conn_from_pool, block=False) as producer:
                    result = producer.publish(
                        body=payload,
                        exchange=exchange,
                        routing_key=routing_key,
                        headers=headers,
                        delivery_mode=delivery_mode,
                        mandatory=mandatory,
                        # immediate=immediate, # RabbitMQ <= 3.2.4 doesn't support this
                        priority=priority,
                        expiration=expiration,
                        compression=compression,
                        declare=declare,
                        retry=retry,
                        retry_policy=retry_policy,
                        serializer=serializer,
                        **publish_kwargs  # properties
                    )
        except ChannelError as exc:
            if "NO_ROUTE" in str(exc):
                raise UndeliverableMessage(
                    exchange=exchange.name, routing_key=routing_key
                )
            raise
        return result

    # def on_return(self, *args, **kwargs):
    #    err = args[0]


## end of class AMQPPublisher


class ProviderCollector(object):
    def __init__(self, **kwargs):
        self._providers = set()  # should it be class variable ?
        self._unreg_providers = set()
        self._providers_registered = False
        super(ProviderCollector, self).__init__(**kwargs)

    def register_provider(self, provider):
        self._providers_registered = True
        self._providers.add(provider)

    def unregister_provider(self, provider):
        if not hasattr(self, "_providers"):
            return
        if provider not in self._providers:
            return
        self._providers.remove(provider)
        self._unreg_providers.add(provider)  # TODO, should it be atomic ?
        if len(self._providers) == 0:
            self._providers_registered = False

    def _find_provider(self, target, label: str):
        try:
            filt = filter(lambda x: x.identity == label, target)
            provider = next(filt)
        except StopIteration as e:
            provider = None
        return provider

    def declare(self, conn, label: str) -> bool:
        provider = self._find_provider(self._providers, label)
        log_args = [
            "action",
            "declare rpc queue",
            "label",
            label,
            "default_channel",
            conn.default_channel,
        ]
        _logger.debug(None, *log_args)
        result = None
        if provider:
            # TODO, it seems that `kombu` internal automatically declares all registered
            # queues at one API call to remote broker, not just one specific queue
            bound_q = provider.queue(conn.default_channel)
            result = bound_q.declare()
        return result is not None

    def undeclare(self, conn, label: str):
        if not hasattr(self, "_unreg_providers"):
            return
        provider = self._find_provider(self._unreg_providers, label)
        if provider:  # delete the queues of the unregistered providers
            try:  # sync with remote broker
                bound_q = provider.queue(conn.default_channel)
                bound_q.delete()
            except ConsumerCancelled as e:
                log_args = [
                    "action",
                    "undeclare rpc queue",
                    "label",
                    label,
                    "msg",
                    str(e.args),
                ]
                _logger.error(None, *log_args)
            self._unreg_providers.remove(provider)


class AMQPQueueConsumer(ProviderCollector, KombuConsumerMixin):
    def __init__(self, amqp_uri, config=None, **kwargs):
        # this project embeds user passwd in the url, TODO, more secure design option
        self._amqp_uri = amqp_uri
        self._config = config or {}
        self._consumers = {}
        self._accept = AMQP_DEFAULT_CONSUMER_ACCEPT_TYPES.copy()
        extra_accept = self._config.pop("accept", None)
        if extra_accept:
            self._accept.extend(extra_accept)
        self.connection = self._init_default_conn()
        super(AMQPQueueConsumer, self).__init__(**kwargs)

    def _init_default_conn(self) -> kombu.Connection:
        heartbeat = self._config.get(AMQP_HEARTBEAT_CONFIG_KEY, AMQP_DEFAULT_HEARTBEAT)
        transport_options = self._config.get(
            AMQP_TRANSPORT_OPTIONS_CONFIG_KEY, AMQP_DEFAULT_TRANSPORT_OPTIONS
        )
        ssl = self._config.get(AMQP_SSL_CONFIG_KEY, None)
        return kombu.Connection(
            self._amqp_uri,
            heartbeat=heartbeat,
            ssl=ssl,
            transport_options=transport_options,
        )
        # connection established lazily whenever needed

    @contextmanager
    def create_connection(self):
        with get_connection(conn=self.connection) as conn_from_pool:
            yield conn_from_pool

    def undeclare(self, label: str):
        ProviderCollector.undeclare(self, conn=self.connection, label=label)

    def get_consumers(self, consumer_cls, channel):
        """
        implement  kombu.mixins.ConsumerMixin.get_consumers() , will be
        invoked by ConsumerMixin.consume() . This function creates consumers for
        all registered providers, which means to consume all the queues at a time.
        """
        for provider in self._providers:
            if self._consumers.get(provider, None):
                continue
            consumer = consumer_cls(
                queues=[provider.queue],
                callbacks=[provider.handle_message],
                accept=self._accept,
            )
            consumer.qos(prefetch_count=1)
            self._consumers[provider] = consumer
        return self._consumers.values()

    def ack_message(self, message):
        # only attempt to ack if the message connection is alive;
        # otherwise the message will already have been reclaimed by the broker
        if message.channel.connection:
            try:
                message.ack()
            except ConnectionError as e:
                log_args = [
                    "action",
                    "ack-msg-conn-err",
                    "msg",
                    str(e.args),
                    "channel",
                    str(message.channel),
                ]
                _logger.warning(None, *log_args)
                # ignore connection closing inside conditional


## end of class AMQPQueueConsumer


class UndeliverableMessage(Exception):
    def __init__(self, exchange, routing_key):
        self.exchange = exchange
        self.routing_key = routing_key
        self.args = (type(self).__name__, str(self.exchange), str(self.routing_key))

    def __str__(self):
        return "undeliverable message, exchange:%s , routing_key:%s" % (
            self.exchange,
            self.routing_key,
        )
