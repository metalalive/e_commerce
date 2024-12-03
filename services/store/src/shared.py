import os
import logging
from importlib import import_module
from typing import Optional, Dict

from asyncmy.constants.CLIENT import MULTI_STATEMENTS
from fastapi import FastAPI

from ecommerce_common.auth.keystore import create_keystore_helper
from ecommerce_common.util import import_module_string
from ecommerce_common.util.messaging.rpc import RPCproxy

from .db import sqlalchemy_init_engine

_logger = logging.getLogger(__name__)

FASTAPI_SETUP_VAR = "APP_SETTINGS"

# the env var `CELERY_CONFIG_MODULE` is actually undocumented , this might be
# non-standard way of getting configuration module hierarchy
CELERY_SETUP_VAR = "CELERY_CONFIG_MODULE"

cfg_mod_path = os.getenv(
    FASTAPI_SETUP_VAR, os.getenv(CELERY_SETUP_VAR, "settings.common")
)

_settings = import_module(cfg_mod_path)
shared_ctx = {"settings": _settings}


def _init_db_engine(conn_args: Optional[dict] = None):
    """TODO
    - for development and production environment, use configurable parameter
      to optionally set multi_statement for the API endpoints that require to run
      multiple SQL statements in one go.
    """
    kwargs = {
        "secrets_file_path": _settings.SECRETS_FILE_PATH,
        "base_folder": _settings.SYS_BASE_PATH,
        "secret_map": (
            _settings.DB_USER_ALIAS,
            "backend_apps.databases.%s" % _settings.DB_USER_ALIAS,
        ),
        "driver_label": _settings.DRIVER_LABEL,
        "db_name": _settings.DB_NAME,
    }
    if conn_args:
        kwargs["conn_args"] = conn_args
    return sqlalchemy_init_engine(**kwargs)


def init_shared_context() -> Dict:
    data = {
        "auth_app_rpc": RPCproxy(
            dst_app_name="user_management",
            src_app_name="store",
            srv_basepath=str(_settings.SYS_BASE_PATH),
        ),
        "product_app_rpc": RPCproxy(
            dst_app_name="product",
            src_app_name="store",
            srv_basepath=str(_settings.SYS_BASE_PATH),
        ),
        "order_app_rpc": RPCproxy(
            dst_app_name="order",
            src_app_name="store",
            srv_basepath=str(_settings.SYS_BASE_PATH),
        ),
        "auth_keystore": create_keystore_helper(
            cfg=_settings.KEYSTORE, import_fn=import_module_string
        ),
        # the engine is the most efficient when created at module-level of application
        # , not per function or per request, modify the implementation in this app.
        "db_engine": _init_db_engine(conn_args={"client_flag": MULTI_STATEMENTS}),
    }
    shared_ctx.update(data)
    return shared_ctx


async def app_shared_context_start(_app: FastAPI):
    shr_ctx = init_shared_context()
    _logger.debug(None, "action", "init-shared-ctx-done")
    return shr_ctx


async def app_shared_context_destroy(_app: FastAPI):
    try:
        _db_engine = shared_ctx.pop("db_engine")
        await _db_engine.dispose()
    except Exception as e:
        log_args = ["action", "deinit-db-error-caught", "detail", ",".join(e.args)]
        _logger.error(None, *log_args)
    rpcobj = shared_ctx.pop("order_app_rpc")
    del rpcobj
    rpcobj = shared_ctx.pop("auth_app_rpc")
    del rpcobj
    rpcobj = shared_ctx.pop("product_app_rpc")
    del rpcobj
    # note intepreter might not invoke `__del__()` for some cases
    # e.g. dependency cycle
