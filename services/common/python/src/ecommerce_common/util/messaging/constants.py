from enum import Enum

# config keys
AMQP_SSL_CONFIG_KEY = 'AMQP_SSL'
SERIALIZER_CONFIG_KEY = 'serializer'
AMQP_EXCHANGE_NAME_CONFIG_KEY = 'exchange_name'
AMQP_EXCHANGE_TYPE_CONFIG_KEY = 'exchange_type'
AMQP_HEARTBEAT_CONFIG_KEY = 'HEARTBEAT'
AMQP_TRANSPORT_OPTIONS_CONFIG_KEY = 'TRANSPORT_OPTIONS'

# default value
DEFAULT_SERIALIZER = 'json'
MSG_PAYLOAD_DEFAULT_CONTENT_TYPE = 'application/json'
AMQP_DEFAULT_CONSUMER_ACCEPT_TYPES = ['application/json']
AMQP_DEFAULT_HEARTBEAT = 60
AMQP_DEFAULT_TRANSPORT_OPTIONS = {
    'max_retries': 3,
    'interval_start': 2,
    'interval_step': 1,
    'interval_max': 5
}
AMQP_DEFAULT_RETRY_POLICY = {'max_retries': 3}


class amqp_exchange_type(Enum):
    FANOUT = 'fanout'
    TOPIC = 'topic'
    DIRECT = 'direct'

RPC_EXCHANGE_DEFAULT_TYPE = amqp_exchange_type.DIRECT.value
RPC_EXCHANGE_DEFAULT_NAME       = 'rpc-default-allapps'

RPC_ROUTE_KEY_PATTERN_SEND    = 'rpc.%s.%s'

class amqp_delivery_mode(Enum):
    NON_PERSISTENT = 1
    PERSISTENT = 2



