import os
from pathlib import Path
from celery.schedules import crontab

from . import _get_amqp_url

# data transfer between clients (producers) and workers (consumers)
# needs to be serialized.
task_serializer = "json"
result_serializer = "json"

timezone = "Asia/Taipei"

srv_basepath = Path(os.environ["SYS_BASE_PATH"]).resolve(strict=True)
secrets_path = os.path.join(srv_basepath, "./common/data/secrets.json")
broker_url = _get_amqp_url(secrets_path=secrets_path)

# periodic task setup
beat_schedule = {
    "mail-notification-cleanup": {
        "task": "celery.backend_cleanup",
        "options": {"queue": "periodic_default"},
        "schedule": 7200,
        "args": (),
    },
    "old-log-localdisk-cleanup": {
        "task": "ecommerce_common.util.periodic_tasks.clean_old_log_localhost",
        "options": {"queue": "periodic_default"},
        "schedule": crontab(
            hour=2, minute=0, day_of_week="tue,thu,sat"
        ),  # every Thursday and Thursday, 2 am
        #'schedule':20,
        "args": (),
        "kwargs": {"max_days_keep": 30},
    },
    "expired-rst-req-cleanup": {
        "task": "user_management.async_tasks.clean_expired_reset_requests",
        "options": {"queue": "usermgt_default"},
        "schedule": crontab(hour=3, minute=00),  ## daily 3:00 am
        #'schedule':30,
        "kwargs": {"days": 6},
    },
    "rotate-auth-keystores": {
        "task": "user_management.async_tasks.rotate_keystores",
        "options": {"queue": "usermgt_default"},
        "schedule": crontab(
            hour=4, minute=15, day_of_week="wed"
        ),  ## 3:30 am on Wednesdays
        #'schedule':60, # for testing / debugging purpose
        "kwargs": {
            "modules_setup": [
                {
                    "keystore": "ecommerce_common.auth.keystore.BaseAuthKeyStore",
                    "num_keys": 3,
                    "key_size_in_bits": 2048,  # PyJWT doesn't allow 1024-bit key pair
                    #'date_limit': '2020-03-04', # for testing / debugging purpose
                    "persist_secret_handler": {
                        "module_path": "ecommerce_common.auth.keystore.JWKSFilePersistHandler",
                        "init_kwargs": {
                            "filepath": "./tmp/cache/dev/jwks/privkey/current.json",
                            "name": "secret",
                            "expired_after_days": 10,
                        },
                    },
                    "persist_pubkey_handler": {
                        "module_path": "ecommerce_common.auth.keystore.JWKSFilePersistHandler",
                        "init_kwargs": {
                            "filepath": "./tmp/cache/dev/jwks/pubkey/current.json",
                            "name": "pubkey",
                            "expired_after_days": 21,
                        },
                    },
                    "keygen_handler": {
                        "module_path": "ecommerce_common.auth.jwt.JwkRsaKeygenHandler",
                        "init_kwargs": {},
                    },
                }
            ]
        },
    },  ## end of periodic task rotate-auth-keystores
}  # end of beat_schedule
