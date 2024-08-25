import os
from pathlib import Path
from ecommerce_common.util import _get_amqp_url

imports = ["user_management.async_tasks"]

# data transfer between clients (producers) and workers (consumers)
# needs to be serialized.
task_serializer = "json"
result_serializer = "json"

timezone = "Asia/Taipei"

srv_basepath = Path(os.environ["SYS_BASE_PATH"]).resolve(strict=True)
secrets_fullpath = os.path.join(srv_basepath, "./common/data/secrets.json")
broker_url = _get_amqp_url(secrets_path=secrets_fullpath, idx=0)


# TODO: use Redis as result backend
# store the result to file system, but file-system result backend
# does not support result_expires and does not have result clean-up function
# you have to implement your own version.
# result_backend = 'file://./tmp/celery/result'

# send result as transient message back to caller from AMQP broker,
# instead of storing it somewhere (e.g. database, file system)
result_backend = "rpc://"
# set False as transient message, if set True, then the message will NOT be
# lost after broker restarts.
# [Downsides]
# * Official documentation mentions it is only for RPC backend,
# * For Django server that includes celery app, once the server shuts down, it will lost
#   all the result/status of the previously running (and completed) tasks, so anyone
#   with correct task ID are no longer capable of checking the status of all previous tasks.
result_persistent = False

# default expiration time in seconds, should depend on different tasks
result_expires = 120

task_queues = {
    #'usermgt_default': {'exchange':'usermgt_default', 'routing_key':'usermgt_default'},
    #'rpc_usermgt_get_profile': {'exchange':'rpc-default-allapps', 'routing_key':'rpc.user_management.get_profile'},
}

task_routes = {
    "user_management.async_tasks.update_accounts_privilege": {
        "queue": "usermgt_default",  ## celery
    },
}  # end of task routes

# set rate limit, at most 10 tasks to process in a single minute.
task_annotations = {
    "user_management.async_tasks.update_accounts_privilege": {"rate_limit": "7/m"},
}

# following 3 parameters affects async result sent from a running task
task_track_started = True
# task_ignore_result = True
# result_expires , note the default is 24 hours


def init_rpc(app):
    import kombu
    from kombu.pools import set_limit as kombu_pool_set_limit
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
            "usermgt_default",
            routing_key="usermgt_default",
            exchange=kombu.Exchange(name="usermgt_default", type="direct"),
        ),
        # kombu.Queue('usermgt_rpc3_rx', routing_key='usermgt_rpc333_rx.*',
        #    exchange=kombu.Exchange(name='usermgt_rpc3_rx', type='topic')),
        kombu.Queue(
            "rpc_usermgt_get_profile",
            exchange=exchange,
            routing_key="rpc.user_management.get_profile",
        ),
        kombu.Queue(
            "rpc_usermgt_profile_descendant_validity",
            exchange=exchange,
            routing_key="rpc.user_management.profile_descendant_validity",
        ),
    ]
    kombu_pool_set_limit(limit=2)
