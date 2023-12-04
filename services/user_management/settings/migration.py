from .common import *
DATABASES['default'] = DATABASES['usermgt_service']
render_logging_handler_localfs('tmp/log/dev')

