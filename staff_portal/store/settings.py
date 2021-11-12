import enum
from pathlib import Path

APP_BASE_PATH = Path(__file__).resolve(strict=True).parent

AUTH_MIGRATION_PATH = APP_BASE_PATH.parent.joinpath('migrations/alembic/store')

SECRETS_FILE_PATH = 'common/data/secrets.json'

DB_NAME = 'ecommerce_store'
AUTH_DB_NAME = 'ecommerce_usermgt'
VERSION_TABLE_AUTH_APP = 'alembic_version_store'

DRIVER_LABEL = 'mariadb+mariadbconnector'

class _MatCodeOptions(enum.Enum):
    MAX_NUM_STORES = 1
    MAX_NUM_STAFF  = 2
    MAX_NUM_EMAILS = 3
    MAX_NUM_PHONES = 4
    MAX_NUM_PRODUCTS = 5

