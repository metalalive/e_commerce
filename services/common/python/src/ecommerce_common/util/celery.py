from ecommerce_common.logging.logger import ExtendedLogger
from celery import Celery

app = Celery("pos_async_tasks_app")
# load centralized configuration module
##app.config_from_object(celeryconfig)
