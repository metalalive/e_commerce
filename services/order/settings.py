import enum
from pathlib import Path

APP_BASE_PATH = Path(__file__).resolve(strict=True).parent

AUTH_MIGRATION_PATH = APP_BASE_PATH.parent.joinpath('migrations/alembic/order')

SECRETS_FILE_PATH = 'common/data/secrets.json'

DB_NAME = 'ecommerce_order'
AUTH_DB_NAME = 'ecommerce_usermgt'
VERSION_TABLE_AUTH_APP = 'alembic_version_order'

DRIVER_LABEL = 'mariadb+mariadbconnector'

class _MatCodeOptions(enum.Enum):
    MAX_NUM_ORDER_INVOICES = 1
    MAX_NUM_ORDER_RECEIPTS = 2

