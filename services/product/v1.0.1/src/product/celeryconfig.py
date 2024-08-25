import os
from pathlib import Path
from ecommerce_common.util import _get_amqp_url

imports = ["product.async_tasks"]
# data transfer between clients (producers) and workers (consumers)
# needs to be serialized.
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
result_expires = 120

task_queues = {}
task_routes = {}
task_track_started = True
# note the default is 24 hours
result_expires = 12


def init_rpc(app):
    import kombu
    from ecommerce_common.util.messaging.constants import (
        RPC_EXCHANGE_DEFAULT_NAME,
        RPC_EXCHANGE_DEFAULT_TYPE,
    )

    exchange = kombu.Exchange(
        name=RPC_EXCHANGE_DEFAULT_NAME, type=RPC_EXCHANGE_DEFAULT_TYPE
    )
    # determine a list of task queues used at here, you don't need to
    # give option -Q at Celery command line
    app.conf.task_queues = [
        kombu.Queue(
            "rpc_productmgt_get_product",
            exchange=exchange,
            routing_key="rpc.product.get_product",
        ),
    ]
