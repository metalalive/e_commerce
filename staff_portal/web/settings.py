import os
from datetime import time
from pathlib import Path
from common.logging.logger import ExtendedLogger

BASE_DIR = Path(__file__).resolve(strict=True).parent.parent

DEBUG = True

# TODO, add new domain hosts from config file
ALLOWED_HOSTS = ['localhost', '127.0.0.1']

INSTALLED_APPS = [
    'django.contrib.auth',
    'django.contrib.contenttypes',
    'django.contrib.sessions',
    'django.contrib.messages',
    'django.contrib.staticfiles',
    'web.apps.WebInterfaceConfig',
]

MIDDLEWARE = [
    'django.middleware.security.SecurityMiddleware',
    'common.sessions.middleware.SessionVerifyMiddleware',
    'django.middleware.common.CommonMiddleware',
    'common.csrf.middleware.ExtendedCsrfViewMiddleware',
    'django.contrib.auth.middleware.AuthenticationMiddleware',
    'common.sessions.middleware.OneSessionPerAccountMiddleware',
    'django.contrib.messages.middleware.MessageMiddleware',
    'django.middleware.clickjacking.XFrameOptionsMiddleware',
]

ROOT_URLCONF = 'web.urls'

TEMPLATES = [
    {
        'BACKEND': 'django.template.backends.django.DjangoTemplates',
        'DIRS': ['my_templates'],
        'APP_DIRS': True,
        'OPTIONS': {
            'context_processors': [
                'django.template.context_processors.debug',
                'django.template.context_processors.request',
                'django.contrib.auth.context_processors.auth',
                'django.contrib.messages.context_processors.messages',
            ],
        },
    },
]

FIXTURE_DIRS = []

# referenced only by development server (`runserver` command)
WSGI_APPLICATION = 'common.util.python.django.wsgi.application'

# this application only provide web interface for frontend client
# , it doesn't have models to migrate
DATABASES = {
    'default': { # only give minimal privilege to start django app server
        'ENGINE': 'django.db.backends.mysql',
        'CONN_MAX_AGE': 0, # set 0 only for debugging purpose
    },
    'usermgt_service': {
        'ENGINE': 'django.db.backends.mysql',
        'CONN_MAX_AGE': 0,
        'reversed_app_label': ['auth']
    },
}

DATABASE_ROUTERS = ['common.models.db.ServiceModelRouter']

# session middleware wil need to invoke this auth backend
AUTHENTICATION_BACKENDS = ['common.auth.backends.ExtendedModelBackend']

SESSION_EXPIRE_AT_BROWSER_CLOSE = True

# expire time may vary based on user groups or roles,
# will need to configure this programmatically
SESSION_COOKIE_AGE = 600

SESSION_ENGINE = 'common.sessions.backends.file'

SESSION_SERIALIZER = 'common.sessions.serializers.ExtendedJSONSerializer'


# the name of request header used for CSRF authentication,
# e.g. according to setting below, frontend may send request "anti-csrf-tok" in the header
CSRF_HEADER_NAME = 'HTTP_X_ANTI_CSRF_TOK'

CSRF_COOKIE_NAME = 'anticsrftok'

# the CSRF token is stored at client side (browser cookie) and should expire as soon as
# the session expires (for logged-in users) , or each valid token should last 12 hours for
# unauthentication accesses.
CSRF_COOKIE_AGE  = 12 * 3600

CACHES = {
        'default': {
            'TIMEOUT': 3600,
            'OPTIONS': {
                'MAX_ENTRIES': 512,
                # TODO, figure out how to use KEY_PREFIX and KEY_FUNCTION
                },
            },
        'user_session': {
            'TIMEOUT': 86400,
            'OPTIONS': {
                'MAX_ENTRIES': 512,
                },
            },
        'log_level_change': {
            'TIMEOUT': None,
            'OPTIONS': {
                'MAX_ENTRIES': 1024,
                },
            },
}

# Internationalization
# https://docs.djangoproject.com/en/dev/topics/i18n/

LANGUAGE_CODE = 'en-us'

TIME_ZONE = 'Asia/Taipei'

USE_I18N = True

USE_L10N = True

USE_TZ = True


# Static files (CSS, JavaScript, Images)
# https://docs.djangoproject.com/en/dev/howto/static-files/
STATIC_ROOT = str(BASE_DIR.parent) + "/static"

# it means the URL http://your_domain_name/static/
STATIC_URL = '/static/'

# besides static files for specific application, there are static files that
# are commonly applied to multiple applications of a project. Here are paths
# to the common static files
COMMON_STATIC_PATH = os.path.join(BASE_DIR ,'common/static')
STATICFILES_DIRS = [str(COMMON_STATIC_PATH),]

DATA_UPLOAD_MAX_NUMBER_FIELDS = 432

PASSWORD_HASHERS = []

# logging
_LOG_BASE_DIR = os.path.join(BASE_DIR ,'tmp/log/staffsite')
_LOG_FMT_DBG_BASE = ["{asctime}", "{levelname}", "{process:d}", "{thread:d}", "{pathname}", "{lineno:d}", "{message}"]
_LOG_FMT_DBG_VIEW = ["{req_ip}", "{req_mthd}", "{uri}"] + _LOG_FMT_DBG_BASE


LOGGING = {
        'version': 1,
        'disable_existing_loggers': False,
        'formatters': {
            'shortened_fmt': {
                'format': "%(asctime)s %(levelname)s %(name)s %(lineno)d %(message)s",
            },
            'dbg_base_fmt': {
                'format': ' '.join(_LOG_FMT_DBG_BASE),
                'style': '{',
            },
            'dbg_view_fmt': {
                'format': ' '.join(_LOG_FMT_DBG_VIEW),
                'style': '{',
            },
        },
        # pre-defined handler classes applied to this project
        'handlers': {
            "default_file": {
                'level': 'WARNING',
                'formatter': 'shortened_fmt',
                'class': 'logging.handlers.TimedRotatingFileHandler',
                'filename': str(os.path.join(_LOG_BASE_DIR, 'default.log')),
                # daily log, keep all log files for one year
                'backupCount': 366,
                # new file is created every 0 am (local time)
                'atTime': time(hour=0, minute=0, second=0),
                'encoding': 'utf-8',
                'delay': True, # lazy creation
            },
            "dbg_views_file": {
                'level': 'INFO',
                'formatter': 'dbg_view_fmt',
                'class': 'logging.handlers.TimedRotatingFileHandler',
                'filename': str(os.path.join(_LOG_BASE_DIR, 'views.log')),
                'backupCount': 150,
                'atTime': time(hour=0, minute=0, second=0),
                'encoding': 'utf-8',
                'delay': True, # lazy creation
            },
            "dbg_views_logstash": {
                'level': 'DEBUG',
                'formatter': 'dbg_view_fmt',
                'class':    'logstash_async.handler.AsynchronousLogstashHandler',
                'transport':'logstash_async.transport.TcpTransport',
                'host': 'localhost',
                'port': 5959,
                'database_path': None,
                # In this project logstash input server and django server are hosted in the
                # same machine, therefore it's not necessary to enable secure connection.
                'ssl_enable': False,
            },
            "dbg_base_logstash": {
                'level': 'DEBUG',
                'formatter': 'dbg_base_fmt',
                'class':    'logstash_async.handler.AsynchronousLogstashHandler',
                'transport':'logstash_async.transport.TcpTransport',
                'host': 'localhost',
                'port': 5959,
                'database_path': None,
                'ssl_enable': False,
            },
        }, # end of handlers section
        'loggers': {
            'common.views.web': {
                'level': 'INFO',
                'handlers': ['dbg_views_logstash'],
            },
            'common.validators': {
                'level': 'WARNING',
                'handlers': ['dbg_base_logstash'],
            },
            'common.auth.backends': {
                'level': 'WARNING',
                'handlers': ['dbg_base_logstash'],
            },
            'common.sessions.middleware': {
                'level': 'WARNING',
                'handlers': ['dbg_base_logstash'],
            },
            'common.sessions.serializers': {
                'level': 'ERROR',
                'handlers': ['dbg_base_logstash'],
            },
            'common.util.python.async_tasks': {
                'level': 'INFO',
                'handlers': ['dbg_base_logstash'],
            },
        }, # end of loggers section
        'root': {
            'level': 'ERROR',
            'handlers': ['default_file'],
        },
} # end of LOGGING

EMAIL_HOST_PASSWORD = "__NOT_USED__"

from common.util.python.django.setup  import setup_secrets

setup_secrets(
    secrets_path='./common/data/secrets.json',
    module_path=__name__, portal_type='staff', interface_type='web'
)


