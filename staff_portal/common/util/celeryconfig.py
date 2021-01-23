import json
from celery.schedules import crontab
# explicitly indicate all tasks applied in this project
imports = ['common.util.async_tasks', 'user_management.async_tasks']

# data transfer between clients (producers) and workers (consumers)
# needs to be serialized.
task_serializer = 'json'
result_serializer = 'json'

timezone = "Asia/Taipei"

# use rabbitmqctl to manage accounts
def _get_amqp_url(secrets_path):
    secrets = None
    with open(secrets_path, 'r') as f:
        secrets = json.load(f)
        secrets = secrets['amqp_broker']
    assert secrets, "failed to load secrets from file"
    protocol = secrets['protocol']
    username = secrets['username']
    passwd = secrets['password']
    host   = secrets['host']
    port   = secrets['port']
    out = '%s://%s:%s@%s:%s' % (protocol, username, passwd, host, port)
    return out

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

# seperate queues for mailing, generating report, handling orders at a high
# volume (if any), and scrapy 
task_queues = {
    'mailing'  : {'exchange':'mailing',   'routing_key':'mailing'},
    'reporting': {'exchange':'reporting', 'routing_key':'reporting'},
    'periodic_default': {'exchange':'periodic_default', 'routing_key':'periodic_default'},
}

task_routes = {
    'common.util.async_tasks.sendmail':
    {
        'queue':'mailing',
        'routing_key':'common.util.async_tasks.sendmail',
    },
    'celery.backend_cleanup':
    {
        'queue':'periodic_default',
        'routing_key':'celery.backend_cleanup',
    },
    'common.util.async_tasks.clean_expired_web_session':
    {
        'queue':'periodic_default',
        'routing_key':'common.util.async_tasks.clean_expired_web_session',
    },
    'common.util.async_tasks.clean_old_log_data':
    {
        'queue':'periodic_default',
        'routing_key':'common.util.async_tasks.clean_old_log_data',
    },
    'user_management.async_tasks.update_roles_on_accounts': {
        'queue':'celery',
        'routing_key':'user_management.async_tasks.update_roles_on_accounts',
    },
    'user_management.async_tasks.clean_expired_auth_token':
    {
        'queue':'periodic_default',
        'routing_key':'user_management.async_tasks.clean_expired_auth_token',
    },
} # end of task routes

# set rate limit, at most 10 tasks to process in a single minute.
task_annotations = {
    'common.util.async_tasks.sendmail': {'rate_limit': '10/m'},
}

# following 3 parameters affects async result sent from a running task
task_track_started = True
# task_ignore_result = True
# result_expires , note the default is 24 hours

# periodic task setup
beat_schedule = {
    'mail-notification-cleanup': {
        'task':'celery.backend_cleanup',
        'schedule':7200,
        'args':(),
    },
    'expired-web-session-cleanup': {
        'task':'common.util.async_tasks.clean_expired_web_session',
        'schedule': crontab(hour=2, minute=0, day_of_week='tue,thu'), # every Thursday and Thursday, 2 am
        'args':(),
    },
    'expired-auth-req-cleanup': {
        'task':'user_management.async_tasks.clean_expired_auth_token',
        'schedule': crontab(hour=3, minute=00), ## daily 3:00 am
        'kwargs': {'days':6},
    },
    'old-log-data-cleanup': {
        'task':'common.util.async_tasks.clean_old_log_data',
        'schedule': crontab(hour=3, minute=30), # daily 3:30 am
        'kwargs': {'days':7, 'weeks':0, 'scroll_size': 1200, 'requests_per_second':-1},
    },
}

