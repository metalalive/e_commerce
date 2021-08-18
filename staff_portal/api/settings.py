import os
from datetime import time
from pathlib import Path
from common.logging.logger import ExtendedLogger

BASE_DIR = Path(__file__).resolve(strict=True).parent.parent

DEBUG = True

ALLOWED_HOSTS = ['localhost', '127.0.0.1']

# Application definition
INSTALLED_APPS = [
    'django.contrib.auth',
    'django.contrib.contenttypes',
    'django.contrib.sessions',
    'rest_framework',
    'api.apps.APIgatewayConfig',
]

MIDDLEWARE = [
    'common.cors.middleware.CorsHeaderMiddleware',
    'django.middleware.security.SecurityMiddleware',
    'django.middleware.common.CommonMiddleware',
    'common.csrf.middleware.ExtendedCsrfViewMiddleware',
    'common.auth.django.middleware.JWTbaseMiddleware',
    'common.sessions.middleware.SessionSetupMiddleware',
    'django.middleware.clickjacking.XFrameOptionsMiddleware',
]

ROOT_URLCONF = 'api.urls'

TEMPLATES = []

FIXTURE_DIRS = ['my_fixtures',]

# referenced only by development server (`runserver` command)
WSGI_APPLICATION = 'common.util.python.django.wsgi.application'

# Database
# https://docs.djangoproject.com/en/dev/ref/settings/#databases
DATABASES = { # will be update with secrets at the bottom of file
    'default': { # only give minimal privilege to start django app server
        'ENGINE': 'django.db.backends.mysql',
        'CONN_MAX_AGE': 0, # set 0 only for debugging purpose
        'TEST': {'NAME': 'test_Restaurant__api_gateway'},
    },
    'usermgt_service': {
        'ENGINE': 'django.db.backends.mysql',
        'CONN_MAX_AGE': 0,
        'reversed_app_label': ['auth']
    },
} # end of database settings

DATABASE_ROUTERS = ['common.models.db.ServiceModelRouter']

AUTH_PASSWORD_VALIDATORS = []

# jwt, session setup requires auth backend
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
CSRF_COOKIE_AGE  = 12 * 3600 ## 43

# this project stores 2 JWTs to cookie in client's browser,
# one of them is access token , the other one for renewing the access token
JWT_NAME_ACCESS_TOKEN  = 'jwt_access_token'
JWT_NAME_REFRESH_TOKEN = 'jwt_refresh_token'
# Note:
# * the valid period is estimated in seconds
# * the period for refresh token is not configurable, it has to be the same as
#   the period for session (used in web server)
# * the period for access token has to be divisible by the period for refresh token
JWT_ACCESS_TOKEN_VALID_PERIOD = 120

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

AUTH_KEYSTORE = {
    'keystore': 'common.auth.keystore.BaseAuthKeyStore',
    'persist_secret_handler': {
        'module_path': 'common.auth.keystore.JWKSFilePersistHandler',
        'init_kwargs': {
            'filepath': './tmp/cache/production/jwks/privkey/current.json',
            'name':'secret', 'expired_after_days': 7,
        },
    },
    'persist_pubkey_handler': {
        'module_path': 'common.auth.keystore.JWKSFilePersistHandler',
        'init_kwargs': {
            'filepath': './tmp/cache/production/jwks/pubkey/current.json',
            'name':'pubkey', 'expired_after_days': 21,
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


PASSWORD_HASHERS = [
    'django.contrib.auth.hashers.BCryptSHA256PasswordHasher',
    'django.contrib.auth.hashers.PBKDF2PasswordHasher',
    'django.contrib.auth.hashers.PBKDF2SHA1PasswordHasher',
    'django.contrib.auth.hashers.Argon2PasswordHasher',
]

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
        }, # end of handler section
        'loggers': {
            'common.views.api': {
                'level': 'INFO',
                'handlers': ['dbg_views_logstash'],
            },
            'common.auth': {
                'level': 'INFO',
                'handlers': ['dbg_base_logstash'],
            },
            'common.auth.middleware': {
                'level': 'INFO',
                'handlers': ['dbg_base_logstash'],
            },
            'common.auth.keystore': {
                'level': 'INFO',
                'handlers': ['dbg_base_logstash'],
            },
            'api.views.security': {
                'level': 'INFO',
                'handlers': ['dbg_views_logstash'],
            },
        }, # end of loggers section
        'root': {
            'level': 'ERROR',
            'handlers': ['default_file'],
        },
} # end of LOGGING section

# Django RESTful API framework
REST_FRAMEWORK = {
    'DEFAULT_PAGINATION_CLASS': 'rest_framework.pagination.PageNumberPagination',
}

EMAIL_HOST_PASSWORD = "__NOT_USED__"

from common.util.python.django.setup  import setup_secrets

setup_secrets(
    secrets_path='./common/data/secrets.json',
    module_path=__name__, portal_type='staff', interface_type='api'
)


