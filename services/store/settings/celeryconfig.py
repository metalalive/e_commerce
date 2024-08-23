import os
from pathlib import Path
from kombu import Queue

from ecommerce_common.util import _get_amqp_url
from ecommerce_common.util.messaging.constants import RPC_EXCHANGE_DEFAULT_NAME

imports = ["store.api.rpc"]
# data transfer between clients (producers) and workers (consumers)
task_serializer = "json"
result_serializer = "json"

timezone = "Asia/Taipei"

srv_basepath = Path(os.environ["SYS_BASE_PATH"]).resolve(strict=True)
secrets_fullpath = os.path.join(srv_basepath, "./common/data/secrets.json")
broker_url = _get_amqp_url(secrets_path=secrets_fullpath, idx=0)

# send result as transient message back to caller from AMQP broker,
# instead of storing it somewhere (e.g. database, file system)
result_backend = "rpc://"
# set False as transient message, if set True, then the message will NOT be
# lost after broker restarts.
result_persistent = False
# default expiration time in seconds, should depend on different tasks
result_expires = 160

task_queues = [
    Queue(
        name="rpc_storefront_get_profile",
        exchange=RPC_EXCHANGE_DEFAULT_NAME,
        routing_key="rpc.storefront.get_profile",
    ),
]
task_routes = {
    "store.api.rpc.get_shop_profile": {
        "queue": "rpc_storefront_get_profile",
        "exchange": RPC_EXCHANGE_DEFAULT_NAME,
        "routing_key": "rpc.storefront.get_profile",
    }
}

task_track_started = True
