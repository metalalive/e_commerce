from .common import *  # noqa: F403
from .celeryconfig import *  # noqa: F403

DB_USER_ALIAS = "test_site2_dba"

KEYSTORE["persist_secret_handler_test"] = {  # noqa: F405
    "module_path": "ecommerce_common.auth.keystore.JWKSFilePersistHandler",
    "init_kwargs": {
        "filepath": SYS_BASE_PATH.joinpath(  # noqa: F405
            "tmp/cache/test/jwks/privkey/current.json"
        ),
        "name": "secret",
        "expired_after_days": 7,
        "flush_threshold": 4,
    },
}

KEYSTORE["persist_pubkey_handler_test"] = {  # noqa: F405
    "module_path": "ecommerce_common.auth.keystore.JWKSFilePersistHandler",
    "init_kwargs": {
        "filepath": SYS_BASE_PATH.joinpath(  # noqa: F405
            "tmp/cache/test/jwks/pubkey/current.json"
        ),
        "name": "pubkey",
        "expired_after_days": 9,
        "flush_threshold": 4,
    },
}
