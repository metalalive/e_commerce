from celery.schedules import crontab

from . import _get_amqp_url

# data transfer between clients (producers) and workers (consumers)
# needs to be serialized.
task_serializer = 'json'
result_serializer = 'json'

timezone = "Asia/Taipei"

broker_url = _get_amqp_url(secrets_path="./common/data/secrets.json")

# periodic task setup
beat_schedule = {
    'mail-notification-cleanup': {
        'task':'celery.backend_cleanup',
        'options': {'queue': 'periodic_default'},
        'schedule':7200,
        'args':(),
    },
    'expired-web-session-cleanup': {
        'task':'common.util.python.periodic_tasks.clean_expired_web_session',
        'options': {'queue': 'periodic_default'},
        'schedule': crontab(hour=2, minute=0, day_of_week='tue,thu'), # every Thursday and Thursday, 2 am
        ##'schedule':30,
        'args':(),
    },
    'expired-auth-req-cleanup': {
        'task':'user_management.async_tasks.clean_expired_auth_token',
        'options': {'queue': 'usermgt_default'},
        'schedule': crontab(hour=3, minute=00), ## daily 3:00 am
        ##'schedule':30,
        'kwargs': {'days':6},
    },
    'rotate-auth-keystores': {
        'task':'user_management.async_tasks.rotate_keystores',
        'options': {'queue': 'usermgt_default'},
        'schedule': crontab(hour=4, minute=15, day_of_week='wed'), ## 3:30 am on Wednesdays
        #'schedule':60, # for testing / debugging purpose
        'kwargs': {'modules_setup': [
            {
                'keystore': 'common.auth.keystore.BaseAuthKeyStore',
                'num_keys': 3,
                'key_size_in_bits': 2048, # PyJWT doesn't allow 1024-bit key pair
                #'date_limit': '2020-03-04', # for testing / debugging purpose
                'persist_secret_handler': {
                    'module_path': 'common.auth.keystore.JWKSFilePersistHandler',
                    'init_kwargs': {
                        'filepath': './tmp/cache/production/jwks/privkey/current.json',
                        'name':'secret', 'expired_after_days': 10,
                    },
                },
                'persist_pubkey_handler': {
                    'module_path': 'common.auth.keystore.JWKSFilePersistHandler',
                    'init_kwargs': {
                        'filepath': './tmp/cache/production/jwks/pubkey/current.json',
                        'name':'pubkey', 'expired_after_days': 21,
                    },
                },
                'keygen_handler': {
                    'module_path': 'common.auth.jwt.JwkRsaKeygenHandler',
                    'init_kwargs': {},
                },
            }]
        },
    }, ## end of periodic task rotate-auth-keystores
    'old-log-data-cleanup': {
        'task':'common.util.python.periodic_tasks.clean_old_log_data',
        'options': {'queue': 'periodic_default'},
        'schedule': crontab(hour=3, minute=30), # daily 3:30 am
        'kwargs': {'days':7, 'weeks':0, 'scroll_size': 1200, 'requests_per_second':-1},
    },
} # end of beat_schedule


