"""
Django settings for restaurant project.

Generated by 'django-admin startproject' using Django 3.1.

For more information on this file, see
https://docs.djangoproject.com/en/dev/topics/settings/

For the full list of settings and their values, see
https://docs.djangoproject.com/en/dev/ref/settings/
"""

import os
from pathlib import Path
from datetime import time

# set ExtendedLogger as default logger
from ecommerce_common.logging.logger import ExtendedLogger
from ecommerce_common.util.django.setup import setup_secrets

# Build paths inside the project like this: BASE_DIR / 'subdir'.
BASE_DIR = Path(__file__).resolve(strict=True).parent.parent.parent
os.environ["SYS_BASE_PATH"] = str(BASE_DIR)


# SECURITY WARNING: don't run with debug turned on in production!
DEBUG = True

ALLOWED_HOSTS = ["localhost", "127.0.0.1"]


# Application definition

INSTALLED_APPS = [
    "django.contrib.auth",
    "django.contrib.contenttypes",
    # configure each application by subclassing AppConfig in apps.py of
    # each application folder, give dotted path of the subclass at here
    # to complete application registry.
    "rest_framework",
    "user_management.apps.UserManagementConfig",
]

# TODO, apply domain name filter at IP layer, only accept requests from trusted proxy server
MIDDLEWARE = [
    "ecommerce_common.cors.middleware.CorsHeaderMiddleware",
    "django.middleware.security.SecurityMiddleware",
    "django.middleware.common.CommonMiddleware",
    "ecommerce_common.csrf.middleware.ExtendedCsrfViewMiddleware",
    #'django.middleware.clickjacking.XFrameOptionsMiddleware', # enable this when frontend webapp needs iframe
]

ROOT_URLCONF = "user_management.urls"

TEMPLATES = [
    {  # will be used when rendering email content
        "BACKEND": "django.template.backends.django.DjangoTemplates",
    }
]

FIXTURE_DIRS = [
    "migrations/django/user_management",
]

# referenced only by development server (`runserver` command)
WSGI_APPLICATION = "ecommerce_common.util.django.wsgi.application"


# Database
# https://docs.djangoproject.com/en/dev/ref/settings/#databases

DATABASES = {  # will be update with secrets at the bottom of file
    "default": {  # only give minimal privilege to start django app server
        "ENGINE": "django.db.backends.mysql",
        "NAME": "ecommerce_usermgt_v2",
        "CONN_MAX_AGE": 0,  # set 0 only for debugging purpose
        "TEST": {"NAME": "test_ecommerce_usermgt_v2"},
    },
    "site_dba": {  # apply this setup only when you run management commands at backend server
        "ENGINE": "django.db.backends.mysql",
        "NAME": "ecommerce_usermgt_v2",
        "CONN_MAX_AGE": 0,
    },
    "usermgt_service": {
        "ENGINE": "django.db.backends.mysql",
        "NAME": "ecommerce_usermgt_v2",
        "CONN_MAX_AGE": 0,
        "reversed_app_label": [
            "user_management",
        ],  # 'auth',
    },
}  # end of database settings

DATABASE_ROUTERS = ["ecommerce_common.models.db.ServiceModelRouter"]


# Password validation
# https://docs.djangoproject.com/en/dev/ref/settings/#auth-password-validators

AUTH_PASSWORD_VALIDATORS = [
    {
        "NAME": "django.contrib.auth.password_validation.UserAttributeSimilarityValidator",
    },
    {
        "NAME": "django.contrib.auth.password_validation.CommonPasswordValidator",
    },
    {
        "NAME": "django.contrib.auth.password_validation.NumericPasswordValidator",
    },
]

AUTH_USER_MODEL = "user_management.LoginAccount"

AUTHENTICATION_BACKENDS = ["user_management.backends.ExtendedModelBackend"]


CACHES = {
    "default": {
        "TIMEOUT": 3600,
        "OPTIONS": {
            "MAX_ENTRIES": 512,
            # TODO, figure out how to use KEY_PREFIX and KEY_FUNCTION
        },
    },
    "log_level_change": {
        "TIMEOUT": None,
        "OPTIONS": {
            "MAX_ENTRIES": 1024,
        },
    },
}

AUTH_KEYSTORE = {
    "keystore": "ecommerce_common.auth.keystore.BaseAuthKeyStore",
    "persist_secret_handler": {
        "module_path": "ecommerce_common.auth.keystore.JWKSFilePersistHandler",
        "init_kwargs": {
            "name": "secret",
            "expired_after_days": 7,
        },
    },
    "persist_pubkey_handler": {
        "module_path": "ecommerce_common.auth.keystore.JWKSFilePersistHandler",
        "init_kwargs": {
            "name": "pubkey",
            "expired_after_days": 21,
        },
    },
}

# in this backend app, session will NOT be used, the parameter below is only for
# synchronization of refresh token and CSRF token for authenticated users
SESSION_COOKIE_AGE = 600
# this project stores refresh JWT to cookie (with httponly flag) in frontend client,
# while access token can be requested by another API endpoint with valid refresh token
JWT_NAME_REFRESH_TOKEN = "jwt_refresh_token"
# Note:
# * the valid period is estimated in seconds
# * the period for refresh token is not configurable, it has to be the
#   same as the period for session (SESSION_COOKIE_AGE, used in web app)
# * the period for access token has to be divisible by the period for refresh token
JWT_ACCESS_TOKEN_VALID_PERIOD = 120
JWT_REFRESH_TOKEN_VALID_PERIOD = SESSION_COOKIE_AGE

# the header name used for CSRF authentication,
# e.g. according to setting below, frontend may send request "anti-csrf-tok" in the header
CSRF_HEADER_NAME = "HTTP_X_ANTI_CSRF_TOK"
# In this project web app sends response with the header CSRF_COOKIE_NAME, which stores CSRF
# token to client, the client would send other unsafe request with the same CSRF token at a later time.
CSRF_COOKIE_NAME = "anticsrftok"
# the parameter below is only referrenced for unauthenticated accesses.
# for authenticated accesses , the token expiry is bound to session expiry
# (used by web app) and JWT token (used by all other backend apps)
CSRF_COOKIE_AGE = 12 * 3600
CSRF_COOKIE_AGE_AUTHED_USER = SESSION_COOKIE_AGE


# Internationalization
# https://docs.djangoproject.com/en/dev/topics/i18n/

LANGUAGE_CODE = "en-us"

TIME_ZONE = "Asia/Taipei"

USE_I18N = True

USE_L10N = True

USE_TZ = True


DATA_UPLOAD_MAX_NUMBER_FIELDS = 400


# use bcrypt + SHA256 as default password hashing function.
PASSWORD_HASHERS = [
    "django.contrib.auth.hashers.BCryptSHA256PasswordHasher",
    "django.contrib.auth.hashers.PBKDF2PasswordHasher",
    "django.contrib.auth.hashers.PBKDF2SHA1PasswordHasher",
    "django.contrib.auth.hashers.Argon2PasswordHasher",
]

# logging
_LOG_FMT_DBG_BASE = [
    "{asctime}",
    "{levelname}",
    "{process:d}",
    "{thread:d}",
    "{pathname}",
    "{lineno:d}",
    "{message}",
]
_LOG_FMT_DBG_VIEW = ["{req_ip}", "{req_mthd}", "{uri}"] + _LOG_FMT_DBG_BASE


LOGGING = {
    "version": 1,
    "disable_existing_loggers": False,
    "formatters": {
        "shortened_fmt": {
            "format": "%(asctime)s %(levelname)s %(name)s %(lineno)d %(message)s",
        },
        "dbg_base_fmt": {
            "format": " ".join(_LOG_FMT_DBG_BASE),
            "style": "{",
        },
        "dbg_view_fmt": {
            "format": " ".join(_LOG_FMT_DBG_VIEW),
            "style": "{",
        },
    },
    # pre-defined handler classes applied to this project
    "handlers": {
        #'console': {
        #    'level': 'ERROR',
        #    'formatter': 'shortened_fmt',
        #    'class': 'logging.StreamHandler',
        #    'stream': 'ext://sys.stdout',
        # },
        "dbg_views_logstash": {
            "level": "DEBUG",
            "formatter": "dbg_view_fmt",
            "class": "logstash_async.handler.AsynchronousLogstashHandler",
            "transport": "logstash_async.transport.TcpTransport",
            "host": "localhost",
            "port": 5959,
            "database_path": None,
            # In this project logstash input server and django server are hosted in the
            # same machine, therefore it's not necessary to enable secure connection.
            "ssl_enable": False,
        },
        "dbg_base_logstash": {
            "level": "DEBUG",
            "formatter": "dbg_base_fmt",
            "class": "logstash_async.handler.AsynchronousLogstashHandler",
            "transport": "logstash_async.transport.TcpTransport",
            "host": "localhost",
            "port": 5959,
            "database_path": None,
            "ssl_enable": False,
        },
    },  # end of handlers section
    "loggers": {
        "ecommerce_common.views.api": {
            "level": "INFO",
            "handlers": ["dbg_views_file", "dbg_views_logstash"],
        },
        "ecommerce_common.views.mixins": {
            "level": "INFO",
            "handlers": ["dbg_views_file", "dbg_views_logstash"],
        },
        "ecommerce_common.views.filters": {
            "level": "WARNING",
            "handlers": ["dbg_views_logstash"],
        },
        "ecommerce_common.serializers": {
            "level": "INFO",
            "handlers": ["dbg_base_logstash"],
        },
        "ecommerce_common.serializers.mixins.nested": {
            "level": "WARNING",
            "handlers": ["dbg_base_logstash"],
        },
        "ecommerce_common.serializers.mixins.quota": {
            "level": "WARNING",
            "handlers": ["dbg_base_logstash"],
        },
        "ecommerce_common.serializers.mixins.closure_table": {
            "level": "WARNING",
            "handlers": ["dbg_base_logstash"],
        },
        "ecommerce_common.validators": {
            "level": "WARNING",
            "handlers": ["dbg_base_logstash"],
        },
        "ecommerce_common.models.closure_table": {
            "level": "WARNING",
            "handlers": ["dbg_base_logstash"],
        },
        "ecommerce_common.models.db": {
            "level": "INFO",
            "handlers": ["dbg_base_logstash"],
        },
        "ecommerce_common.models.migrations": {
            "level": "INFO",
            "handlers": ["default_file", "dbg_base_logstash"],
        },
        "ecommerce_common.auth.keystore": {
            "level": "INFO",
            "handlers": ["dbg_base_logstash"],
        },
        "ecommerce_common.auth.backends": {
            "level": "WARNING",
            "handlers": ["dbg_base_logstash"],
        },
        "ecommerce_common.sessions.middleware": {
            "level": "WARNING",
            "handlers": ["dbg_base_logstash"],
        },
        "ecommerce_common.sessions.serializers": {
            "level": "ERROR",
            "handlers": ["dbg_base_logstash"],
        },
        "ecommerce_common.util.elasticsearch": {
            "level": "WARNING",
            "handlers": ["dbg_base_logstash"],
        },
        "ecommerce_common.util.async_tasks": {
            "level": "INFO",
            "handlers": ["dbg_base_logstash"],
        },
        "user_management.views.api": {
            "level": "INFO",
            "handlers": ["dbg_views_file", "dbg_views_logstash"],
        },
        "user_management.views.common": {
            "level": "WARNING",
            "handlers": ["dbg_views_logstash"],
        },
        "user_management.serializers.nested": {
            "level": "INFO",
            "handlers": ["dbg_base_logstash"],
        },
        "user_management.serializers": {
            "level": "INFO",
            "handlers": ["dbg_base_logstash"],
        },
        "user_management.models": {
            "level": "INFO",
            "handlers": ["dbg_base_logstash"],
        },
        "user_management.permissions": {
            "level": "WARNING",
            "handlers": ["dbg_views_logstash"],
        },
        "user_management.async_tasks": {
            "level": "INFO",
            "handlers": ["dbg_base_logstash"],
        },
        "user_management.queryset": {
            "level": "WARNING",
            "handlers": ["dbg_base_logstash"],
        },
        "softdelete.models": {
            "level": "WARNING",
            "handlers": ["dbg_base_logstash"],
        },
        "softdelete.views": {
            "level": "INFO",
            "handlers": ["dbg_views_logstash"],
        },
    },  # end of loggers section
    "root": {
        "level": "ERROR",
        "handlers": ["default_file"],
    },
}  # end of LOGGING


def render_logging_handler_localfs(log_dir):
    _log_base_dir = os.path.join(BASE_DIR, log_dir)
    handlers = {
        "default_file": {
            "level": "WARNING",
            "formatter": "shortened_fmt",
            "class": "logging.handlers.TimedRotatingFileHandler",
            "filename": str(os.path.join(_log_base_dir, "usermgt_default.log")),
            # daily log, keep all log files for one year
            "backupCount": 366,
            # new file is created every 0 am (local time)
            "atTime": time(hour=0, minute=0, second=0),
            "encoding": "utf-8",
            "delay": True,  # lazy creation
        },
        "dbg_views_file": {
            "level": "INFO",
            "formatter": "dbg_view_fmt",
            "class": "logging.handlers.TimedRotatingFileHandler",
            "filename": str(os.path.join(_log_base_dir, "usermgt_views.log")),
            "backupCount": 150,
            "atTime": time(hour=0, minute=0, second=0),
            "encoding": "utf-8",
            "delay": True,  # lazy creation
        },
    }
    LOGGING["handlers"].update(handlers)


REST_FRAMEWORK = {
    "DEFAULT_PAGINATION_CLASS": "rest_framework.pagination.PageNumberPagination",
    "EXCEPTION_HANDLER": "ecommerce_common.views.api.exception_handler",
    #'PAGE_SIZE' : 40
}

# mailing function setup
DEFAULT_FROM_EMAIL = "system@yourproject.io"

setup_secrets(
    secrets_path=os.path.join(BASE_DIR, "common/data/secrets.json"),
    module_path=__name__,
    portal_type="staff",
    interface_type="usermgt",
)
