import os
from datetime import time
from pathlib import Path
from common.logging.logger import ExtendedLogger

BASE_DIR = Path(__file__).resolve(strict=True).parent.parent

DEBUG = False

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
    'common.auth.middleware.JWTbaseMiddleware',
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
        'TEST': {},
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

JWT_COOKIE_NAME = 'jwt'

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
        'jwt_secret': {
            'TIMEOUT': 69283,
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


PASSWORD_HASHERS = [
    'django.contrib.auth.hashers.BCryptSHA256PasswordHasher',
    'django.contrib.auth.hashers.PBKDF2PasswordHasher',
    'django.contrib.auth.hashers.PBKDF2SHA1PasswordHasher',
    'django.contrib.auth.hashers.Argon2PasswordHasher',
]

# logging , TODO

REST_FRAMEWORK = {
    'DEFAULT_PAGINATION_CLASS': 'rest_framework.pagination.PageNumberPagination',
}

EMAIL_HOST_PASSWORD = "__NOT_USED__"

from common.util.python.django.setup  import setup_secrets

setup_secrets(
    secrets_path='./common/data/secrets.json',
    module_path=__name__, portal_type='staff', interface_type='api'
)


