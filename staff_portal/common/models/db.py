from time import sleep
import logging

from requests.status_codes import codes as requests_codes
from MySQLdb._exceptions import OperationalError as MySqlOperationalError
from MySQLdb.constants   import ER as MySqlErrorCode
from django.conf       import settings as django_settings
from django.http       import HttpResponse
from django.db.utils   import OperationalError

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


def _get_mysql_error_response(e, headers, raise_if_not_handled):
    code = e.args[0]
    msg  = e.args[1]
    log_args = ['db_backend', 'mysql', 'code', code, 'msg', msg]
    if code == MySqlErrorCode.USER_LIMIT_REACHED:
        # tell client to delay 1 sec before making follow-up request
        status = requests_codes['too_many_requests']
        headers['Retry-After'] = 1
    else:
        _logger.error(None, *log_args)
        if raise_if_not_handled:
            raise
        else:
            status = requests_codes['internal_server_error']
        # TODO, handle error 1062 : duplicate entry `<PK_VALUE>` for key `<PK_FIELD_NAME>`
    _logger.info(None, *log_args)
    return status



def get_db_error_response(e, headers:dict, raise_if_not_handled=True):
    status = requests_codes['internal_server_error']
    cause = e.__cause__
    if cause:
        handler = _err_resp_map[type(cause)]
        status = handler(e=cause, headers=headers, raise_if_not_handled=raise_if_not_handled)
    return status


def db_middleware_exception_handler(func):
    def inner(self, *arg, **kwargs):
        try:
            response = func(self, *arg, **kwargs)
        except OperationalError as e:
            headers = {}
            status = get_db_error_response(e=e, headers=headers)
            # do NOT use DRF response since the request is being short-circuited by directly returning
            # custom response at here and it won't invoke subsequent (including view) middlewares.
            # Instead I use HttpResponse simply containing error response status without extra message
            # in the response body.
            response = HttpResponse(status=status)
            for k,v in headers.items():
                response[k] = v
            err_msg = ' '.join(list(map(lambda x: str(x), e.args)))
            log_msg = ['status', status, 'msg', err_msg]
            _logger.warning(None, *log_msg)
        return response
    return inner


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

