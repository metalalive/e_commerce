from celery import Celery

##from . import celeryconfig

app = Celery("pos_async_tasks_app")
# load centralized configuration module
##app.config_from_object(celeryconfig)

# import os
# set default Django settings module for Celery application
# os.environ.setdefault('DJANGO_SETTINGS_MODULE', 'user_management.settings')
