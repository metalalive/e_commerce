import enum

SECRETS_FILE_PATH = 'common/data/secrets.json'
DB_NAME = 'ecommerce_order'


class _MatCodeOptions(enum.Enum):
    MAX_NUM_ORDER_INVOICES = 1
    MAX_NUM_ORDER_RECEIPTS = 2

