import os
import logging
from importlib import import_module
from typing import Optional

from mariadb.constants.CLIENT import MULTI_STATEMENTS
from fastapi import FastAPI

from common.models.db  import sqlalchemy_init_engine
from common.util.python.messaging.rpc import RPCproxy

_logger = logging.getLogger(__name__)

_settings_module_path = os.getenv('APP_SETTINGS', 'store.settings.common')
settings = import_module(_settings_module_path)
shared_ctx = {'settings':settings}

def _init_db_engine(conn_args:Optional[dict]=None):
    """ TODO
      - for development and production environment, use configurable parameter
        to optionally set multi_statement for the API endpoints that require to run
        multiple SQL statements in one go.
    """ 
    kwargs = {
        'secrets_file_path':settings.SECRETS_FILE_PATH, 'base_folder':settings.SYS_BASE_PATH,
        'secret_map':(settings.DB_USER_ALIAS, 'backend_apps.databases.%s' % settings.DB_USER_ALIAS),
        'driver_label':settings.DRIVER_LABEL, 'db_name':settings.DB_NAME,
    }
    if conn_args:
        kwargs['conn_args'] = conn_args
    return sqlalchemy_init_engine(**kwargs)


async def app_shared_context_start(_app:FastAPI):
    from common.auth.keystore import create_keystore_helper
    from common.util.python import import_module_string
    shared_ctx['auth_app_rpc'] = RPCproxy(dst_app_name='user_management', src_app_name='store')
    shared_ctx['product_app_rpc'] = RPCproxy(dst_app_name='product', src_app_name='store')
    shared_ctx['order_app_rpc'] = RPCproxy(dst_app_name='order', src_app_name='store')
    shared_ctx['auth_keystore'] = create_keystore_helper(cfg=settings.KEYSTORE, import_fn=import_module_string)
    # the engine is the most efficient when created at module-level of application
    # , not per function or per request, modify the implementation in this app.
    shared_ctx['db_engine'] = _init_db_engine(conn_args={'client_flag':MULTI_STATEMENTS})
    _logger.debug(None, 'action', 'init-shared-ctx-done')
    return shared_ctx

async def app_shared_context_destroy(_app:FastAPI):
    try:
        _db_engine = shared_ctx.pop('db_engine')
        _db_engine.dispose()
    except Exception as e:
        log_args = ['action', 'deinit-db-error-caught', 'detail', ','.join(e.args)]
        _logger.error(None, *log_args)
    rpcobj = shared_ctx.pop('order_app_rpc')
    del rpcobj
    rpcobj = shared_ctx.pop('auth_app_rpc')
    del rpcobj
    rpcobj = shared_ctx.pop('product_app_rpc')
    del rpcobj
    # note intepreter might not invoke `__del__()` for some cases
    # e.g. dependency cycle


