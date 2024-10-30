from time import sleep
from pathlib import Path
from typing import Optional, Tuple
import functools
import logging

from ecommerce_common.util import (
    import_module_string,
    format_sqlalchemy_url,
    get_credential_from_secrets,
)

_logger = logging.getLogger(__name__)


def db_conn_retry_wrapper(func):
    """
    decorator for instance method that handles API call and requires database connection
    , define max_retry_db_conn and wait_intvl_sec as object variables in advance
    """
    # TODO, rename to django_db_conn_retry_wrapper
    from django.db.utils import OperationalError

    def inner(self, *arg, **kwargs):
        out = None
        max_retry_db_conn = getattr(self, "max_retry_db_conn", 3)
        wait_intvl_sec = getattr(self, "wait_intvl_sec", 0.01)
        log_args = [
            "wait_intvl_sec",
            wait_intvl_sec,
            "max_retry_db_conn",
            max_retry_db_conn,
        ]
        while max_retry_db_conn > 0:
            try:
                out = func(self, *arg, **kwargs)
                break
            except OperationalError as e:
                max_retry_db_conn -= 1
                if max_retry_db_conn < 1:
                    log_args.extend(["excpt_msg", e])
                    _logger.warning(None, *log_args)
                    raise  # throw the same excpetion & let upper layer application handle it
                else:
                    sleep(wait_intvl_sec)
        return out  #### end of inner()

    return inner  #### end of db_conn_retry_wrapper()


def sqlalchemy_init_engine(
    secrets_file_path,
    secret_map: Tuple[str, str],
    base_folder: Path,
    driver_label: str,
    db_name: str = "",
    conn_args: Optional[dict] = None,
):
    from sqlalchemy.ext.asyncio import create_async_engine

    conn_args = conn_args or {}
    db_credentials = get_credential_from_secrets(
        base_path=base_folder,
        secret_path=secrets_file_path,
        secret_map=dict([secret_map]),
    )
    chosen_db_credential = db_credentials[secret_map[0]]
    if db_name:
        chosen_db_credential["NAME"] = db_name
    url = format_sqlalchemy_url(driver=driver_label, db_credential=chosen_db_credential)
    # reminder: use engine.dispose() to free up all connections in its pool
    return create_async_engine(url, connect_args=conn_args)


def sqlalchemy_db_conn(engine, enable_orm=False):
    """
    decorator for starting a database connection (either establishing new
    one or grab from connection pool) in SQLAlchemy
    """
    assert engine, (
        "argument `engine` has to be SQLAlchemy engine instance \
            , but receive invalid value %s"
        % (engine)
    )
    if enable_orm:
        from sqlalchemy import orm as sa_orm

    def inner(func):
        @functools.wraps(func)
        def wrapped(*args, **kwargs):
            result = None
            if enable_orm:
                with sa_orm.Session(engine) as session:
                    kwargs["session"] = session
                    result = func(*args, **kwargs)
            else:
                with engine.connect() as conn:
                    kwargs["conn"] = conn
                    result = func(*args, **kwargs)
            return result

        return wrapped

    return inner


def sqlalchemy_insert(model_cls_path: str, data: list, conn):
    """
    SQLAlchemy helper function for inserting new record to database
    """
    result = None
    model_cls = import_module_string(model_cls_path)
    ins = model_cls.__table__.insert()
    if len(data) == 1:
        ins = ins.values(**data[0])
        result = conn.execute(ins)
    elif len(data) > 1:
        result = conn.execute(ins, data)
    return result


def _get_mysql_error_response(e, headers, raise_if_not_handled):
    from requests.status_codes import codes as requests_codes
    from MySQLdb.constants import ER as MySqlErrorCode

    code = e.args[0]
    msg = e.args[1]
    log_args = ["db_backend", "mysql", "code", code, "msg", msg]
    if code == MySqlErrorCode.USER_LIMIT_REACHED:
        # tell client to delay 1 sec before making follow-up request
        status = requests_codes["too_many_requests"]
        headers["Retry-After"] = 1
    else:
        _logger.error(None, *log_args)
        if raise_if_not_handled:
            raise
        else:
            status = requests_codes["internal_server_error"]
        # TODO, handle error 1062 : duplicate entry `<PK_VALUE>` for key `<PK_FIELD_NAME>`
    _logger.info(None, *log_args)
    return status


def get_db_error_response(e, headers: dict, raise_if_not_handled=True):
    from MySQLdb._exceptions import OperationalError as MySqlOperationalError

    status = 500  # defaults to internal_server_error
    cause = e.__cause__
    if cause:
        _err_resp_map = {MySqlOperationalError: _get_mysql_error_response}
        handler = _err_resp_map.get(type(cause))
        if handler and callable(handler):
            status = handler(
                e=cause, headers=headers, raise_if_not_handled=raise_if_not_handled
            )
    return status


def get_sql_table_pk_gap_ranges(db_table: str, pk_db_column: str, max_value: int):
    raw_sql_subquery = "SELECT m1.{pk_db_column} as lowerbound, MIN(m2.{pk_db_column}) as upperbound \
            FROM {your_table} m1 INNER JOIN {your_table} AS m2 ON m1.{pk_db_column} < m2.{pk_db_column} \
            GROUP BY m1.{pk_db_column} ORDER BY m1.{pk_db_column} ASC".format(
        your_table=db_table, pk_db_column=pk_db_column
    )
    raw_sql_queries = []
    raw_sql_queries.append(
        "SELECT 1 AS gap_from, {pk_db_column} - 1 AS gap_to FROM {your_table} WHERE {pk_db_column} \
            > 1 ORDER BY {pk_db_column} ASC LIMIT 1".format(
            your_table=db_table, pk_db_column=pk_db_column
        )
    )
    raw_sql_queries.append(
        "SELECT m3.lowerbound + 1 AS gap_from, m3.upperbound - 1 AS gap_to FROM (%s) m3 WHERE \
             m3.lowerbound < m3.upperbound - 1 LIMIT 8"
        % raw_sql_subquery
    )
    raw_sql_queries.append(
        "SELECT {pk_db_column} + 1 AS gap_from, {max_value} AS gap_to FROM {your_table} \
            ORDER BY {pk_db_column} DESC LIMIT 1".format(
            your_table=db_table, pk_db_column=pk_db_column, max_value=max_value
        )
    )
    return raw_sql_queries


class ServiceModelRouter:
    # commonly used apps for all services
    _common_app_labels = [
        "contenttypes",
    ]

    def __init__(self, *args, **kwargs):
        from django.conf import settings as django_settings

        self._app_db_map = {
            app_label: k
            for k, v in django_settings.DATABASES.items()
            if v.get("reversed_app_label", [])
            for app_label in v["reversed_app_label"]
        }

    def db_for_read(self, model, **hints):
        chosen_db_tag = self._app_db_map.get(model._meta.app_label, None)
        log_args = [
            "model",
            model._meta.app_label,
            "hints",
            hints,
            "chosen_db_tag",
            chosen_db_tag,
        ]
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
        log_args = [
            "obj1",
            obj1,
            "hints",
            hints,
            "obj2",
            obj2,
            "app1",
            app1,
            "app2",
            app2,
            "db_tag_1",
            db_tag_1,
            "db_tag_2",
            db_tag_2,
        ]
        _logger.debug(None, *log_args)
        if app1 in self._common_app_labels or app2 in self._common_app_labels:
            return True  ## output None will raise access-denied error, how ?
        else:
            return db_tag_1 == db_tag_2

    def allow_migrate(self, db, app_label, model_name=None, **hints):
        # log_args = ['db', db, 'hints', hints, 'app_label', app_label]
        # _logger.debug(None, *log_args)
        return None


class BaseDatabaseError(Exception):
    pass


class EmptyDataRowError(BaseDatabaseError):
    pass
