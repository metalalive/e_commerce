from .common import *
DATABASES['default'] = DATABASES['product_dev_service']
render_logging_handler_localfs('tmp/log/dev')

