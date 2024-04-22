import json

# re-import the parameters from common.py , it is acceptable if some variables
# are unused at here
from .common import *  # noqa: F403

secrets_path = BASE_DIR.joinpath("common/data/secrets.json")  # noqa: F405
secrets = None

AUTH_KEYSTORE["persist_secret_handler_test"] = {  # noqa: F405
    "module_path": "ecommerce_common.auth.keystore.JWKSFilePersistHandler",
    "init_kwargs": {
        "filepath": BASE_DIR.joinpath(  # noqa: F405
            "tmp/cache/test/jwks/privkey/current.json"
        ),
        "name": "secret",
        "expired_after_days": 7,
        "flush_threshold": 4,
    },
}

AUTH_KEYSTORE["persist_pubkey_handler_test"] = {  # noqa: F405
    "module_path": "ecommerce_common.auth.keystore.JWKSFilePersistHandler",
    "init_kwargs": {
        "filepath": BASE_DIR.joinpath(  # noqa: F405
            "tmp/cache/test/jwks/pubkey/current.json"
        ),
        "name": "pubkey",
        "expired_after_days": 9,
        "flush_threshold": 4,
    },
}

with open(secrets_path, "r") as f:
    secrets = json.load(f)
    secrets = secrets["backend_apps"]["databases"]["test_site_dba"]

# Django test only uses `default` alias , which does NOT allow users to switch
# between different database credentials
DATABASES["default"].update(secrets)  # noqa: F405
DATABASES["default"]["NAME"] = DATABASES["default"]["TEST"]["NAME"]  # noqa: F405

DATABASE_ROUTERS.clear()  # noqa: F405
render_logging_handler_localfs(BASE_DIR.joinpath("tmp/log/test"))  # noqa: F405
