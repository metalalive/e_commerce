import json
from celery.schedules import crontab

from common.util.python.celeryconfig import _get_amqp_url


# TODO, import & register tasks dynamically from different services
# explicitly indicate all tasks applied in this project
imports = ['user_management.async_tasks']

# data transfer between clients (producers) and workers (consumers)
# needs to be serialized.
task_serializer = 'json'
result_serializer = 'json'

timezone = "Asia/Taipei"

broker_url = _get_amqp_url(secrets_path="./common/data/secrets.json")


# TODO: use Redis as result backend
# store the result to file system, but file-system result backend
# does not support result_expires and does not have result clean-up function
# you have to implement your own version.
#result_backend = 'file://./tmp/celery/result'

# send result as transient message back to caller from AMQP broker,
# instead of storing it somewhere (e.g. database, file system)
result_backend = 'rpc://'
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
    'usermgt_default': {'exchange':'usermgt_default', 'routing_key':'usermgt_default'},
}

task_routes = {
    'user_management.async_tasks.update_roles_on_accounts': {
        'queue':'usermgt_default', ## celery
    },
} # end of task routes

# set rate limit, at most 10 tasks to process in a single minute.
task_annotations = {
    'user_management.async_tasks.update_roles_on_accounts': {'rate_limit': '7/m'},
}

# following 3 parameters affects async result sent from a running task
task_track_started = True
# task_ignore_result = True
# result_expires , note the default is 24 hours

