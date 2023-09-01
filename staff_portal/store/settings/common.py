import enum
from pathlib import Path
# set ExtendedLogger as default logger
from common.logging.logger import ExtendedLogger

APP_BASE_PATH = Path(__file__).resolve(strict=True).parent.parent
SYS_BASE_PATH = APP_BASE_PATH.parent

AUTH_MIGRATION_PATH = APP_BASE_PATH.parent.joinpath('migrations/alembic/store')

SECRETS_FILE_PATH = 'common/data/secrets.json'

DB_NAME = 'ecommerce_store'
DB_USER_ALIAS = None

ORM_BASE_CLASSES = ['store.models.Base']

DRIVER_LABEL = 'mariadb+mariadbconnector'

class _MatCodeOptions(enum.Enum):
    MAX_NUM_STORES = 1
    MAX_NUM_STAFF  = 2
    MAX_NUM_EMAILS = 3
    MAX_NUM_PHONES = 4
    MAX_NUM_PRODUCTS = 5


from common.cors import config as cors_config

APP_HOST = cors_config.ALLOWED_ORIGIN['store']

AUTH_APP_HOST = cors_config.ALLOWED_ORIGIN['user_management']

REFRESH_ACCESS_TOKEN_API_URL = '%s/refresh_access_token' % AUTH_APP_HOST

INIT_SHARED_CONTEXT_FN = 'store.views.app_shared_context_start'
DEINIT_SHARED_CONTEXT_FN = 'store.views.app_shared_context_destroy'

ROUTERS = ['store.views.router']

KEYSTORE = {
    "keystore": "common.auth.keystore.BaseAuthKeyStore",
    "persist_pubkey_handler": {
        "module_path": "common.auth.jwt.RemoteJWKSPersistHandler",
        "init_kwargs": {"url": "http://localhost:8008/jwks",
            "name":"remote_pubkey", "lifespan_hrs":12 }
    }
}

NUM_RETRY_RPC_RESPONSE = 5

