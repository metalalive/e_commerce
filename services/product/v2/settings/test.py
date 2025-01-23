from .common import *  # noqa: F403
from .celeryconfig import *  # noqa: F403

DATABASES["confidential_path"] = (  # noqa: F405
    "backend_apps.databases.product_test_service_v2"
)

KEYSTORE["persist_secret_handler_test"] = {  # noqa: F405
    "module_path": "ecommerce_common.auth.keystore.JWKSFilePersistHandler",
    "init_kwargs": {
        "filepath": SYS_BASE_PATH.joinpath(  # noqa: F405
            "tmp/cache/test/jwks/privkey/current.json"
        ),
        "name": "secret",
        "expired_after_days": 8,
        "flush_threshold": 5,
    },
}

KEYSTORE["persist_pubkey_handler_test"] = {  # noqa: F405
    "module_path": "ecommerce_common.auth.keystore.JWKSFilePersistHandler",
    "init_kwargs": {
        "filepath": SYS_BASE_PATH.joinpath(  # noqa: F405
            "tmp/cache/test/jwks/pubkey/current.json"
        ),
        "name": "pubkey",
        "expired_after_days": 10,
        "flush_threshold": 6,
    },
}
