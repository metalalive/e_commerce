import os
from pathlib import Path

# set ExtendedLogger as default logger
from ecommerce_common.logging.logger import ExtendedLogger  # noqa: F401

APP_BASE_PATH = Path(__file__).resolve(strict=True).parent.parent
SYS_BASE_PATH = APP_BASE_PATH.parent

os.environ["APP_BASE_PATH"] = str(APP_BASE_PATH)
os.environ["SYS_BASE_PATH"] = str(SYS_BASE_PATH)

from ecommerce_common.cors import config as cors_config  # noqa: E402

SECRETS_FILE_PATH = SYS_BASE_PATH.joinpath("common/data/secrets.json")

DB_HOST = os.environ["DB_HOST"]
DB_PORT = int(os.environ["DB_PORT"])
DB_NAME = os.environ["DB_NAME"]
DB_USER_ALIAS = None

ORM_BASE_CLASSES = ["store.models.Base"]

DRIVER_LABEL = "mysql+asyncmy"

APP_HOST = cors_config.ALLOWED_ORIGIN["store"]

AUTH_APP_HOST = cors_config.ALLOWED_ORIGIN["user_management"]

REFRESH_ACCESS_TOKEN_API_URL = "%s/refresh_access_token" % AUTH_APP_HOST

INIT_SHARED_CONTEXT_FN = "store.shared.app_shared_context_start"
DEINIT_SHARED_CONTEXT_FN = "store.shared.app_shared_context_destroy"

ROUTERS = ["store.api.web.router"]
EXCEPTION_HANDLERS = [
    ("store.api.web.request_error_handler", "store.dto.StoreDtoError"),
    ("store.api.web.rpc_error_handler", "store.shared.AppRpcError"),
]

KEYSTORE = {
    "keystore": "ecommerce_common.auth.keystore.BaseAuthKeyStore",
    "persist_pubkey_handler": {
        "module_path": "ecommerce_common.auth.jwt.RemoteJWKSPersistHandler",
        "init_kwargs": {
            "url": f"{AUTH_APP_HOST}/jwks",
            "name": "remote_pubkey",
            "lifespan_hrs": 12,
        },
    },
}

NUM_RETRY_RPC_RESPONSE = 5
