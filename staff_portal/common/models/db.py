from time import sleep
import logging

from MySQLdb._exceptions import OperationalError as MySqlOperationalError
from MySQLdb.constants   import ER as MySqlErrorCode
from django.conf       import settings as django_settings
from django.db.utils   import OperationalError
from rest_framework    import status as RestStatus

_logger = logging.getLogger(__name__)

def db_conn_retry_wrapper(func):
    """
    decorator for instance method that handles API call and requires database connection
    , define max_retry_db_conn and wait_intvl_sec as object variables in advance
    """
    def inner(self, *arg, **kwargs):
        out = None
        max_retry_db_conn = getattr(self, 'max_retry_db_conn', 3)
        wait_intvl_sec    = getattr(self, 'wait_intvl_sec', 0.01)
        log_args = ['wait_intvl_sec', wait_intvl_sec, 'max_retry_db_conn', max_retry_db_conn]
        while max_retry_db_conn > 0:
            try:
                out = func(self, *arg, **kwargs)
                break
            except OperationalError as e:
                max_retry_db_conn -= 1
                if max_retry_db_conn < 1:
                    log_args.extend(['excpt_msg', e])
                    _logger.warning(None, *log_args)
                    raise # throw the same excpetion & let upper layer application handle it
                else:
                    sleep(wait_intvl_sec)
        return out #### end of inner()
    return inner #### end of db_conn_retry_wrapper()


def _get_mysql_error_response(e, headers):
    code = e.args[0]
    msg  = e.args[1]
    log_args = ['db_backend', 'mysql', 'code', code, 'msg', msg]
    if code == MySqlErrorCode.USER_LIMIT_REACHED:
        # tell client to delay 1 sec before making follow-up request
        status = RestStatus.HTTP_429_TOO_MANY_REQUESTS
        headers['Retry-After'] = 1
    else:
        _logger.error(None, *log_args)
        raise
        # TODO, handle error 1062 : duplicate entry `<PK_VALUE>` for key `<PK_FIELD_NAME>`
    _logger.info(None, *log_args)
    return status



def get_db_error_response(e, headers:dict):
    cause = e.__cause__
    return _err_resp_map[type(cause)](e=cause, headers=headers)


class ServiceModelRouter:
    # commonly used apps for all services
    _common_app_labels = ['contenttypes',]

    def __init__(self, *args, **kwargs):
        self._app_db_map = {app_label: k for k,v in django_settings.DATABASES.items() if
                v.get('reversed_app_label', []) for app_label in v['reversed_app_label']}

    def db_for_read(self, model, **hints):
        chosen_db_tag = self._app_db_map.get(model._meta.app_label, None)
        log_args = ['model', model._meta.app_label, 'hints', hints, 'chosen_db_tag', chosen_db_tag]
        _logger.debug(None, *log_args)
        ##chosen_db_tag = 'site_dba'
        return chosen_db_tag

    def db_for_write(self, model, **hints):
        return self._app_db_map.get(model._meta.app_label, None)

    def allow_relation(self, obj1, obj2, **hints):
        app1 = obj1._meta.app_label
        app2 = obj2._meta.app_label
        db_tag_1 = self._app_db_map.get(app1, None)
        db_tag_2 = self._app_db_map.get(app2, None)
        log_args = ['obj1', obj1, 'hints', hints, 'obj2', obj2, 'app1', app1, 'app2', app2,
                'db_tag_1', db_tag_1, 'db_tag_2', db_tag_2]
        _logger.debug(None, *log_args)
        if app1 in self._common_app_labels  or app2 in self._common_app_labels:
            return True ## output None will raise access-denied error, how ?
        else:
            return db_tag_1 == db_tag_2

    def allow_migrate(self, db, app_label, **hints):
        #log_args = ['db', db, 'hints', hints, 'app_label', app_label]
        #_logger.debug(None, *log_args)
        return None


_err_resp_map = {
    MySqlOperationalError: _get_mysql_error_response
}

