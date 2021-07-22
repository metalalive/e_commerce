from time import sleep
import functools
import logging

from common.util.python import import_module_string,  format_sqlalchemy_url, get_credential_from_secrets

_logger = logging.getLogger(__name__)

def db_conn_retry_wrapper(func):
    """
    decorator for instance method that handles API call and requires database connection
    , define max_retry_db_conn and wait_intvl_sec as object variables in advance
    """
    # TODO, rename to django_db_conn_retry_wrapper
    from django.db.utils   import OperationalError
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


def sqlalchemy_init_engine(secrets_file_path, secret_map, base_folder, driver_label, conn_args=None):
    import sqlalchemy as sa
    conn_args = conn_args or {}
    db_credentials = get_credential_from_secrets(base_folder=base_folder,
            secret_path=secrets_file_path,  secret_map=dict([secret_map]))
    url = format_sqlalchemy_url(driver=driver_label, db_credential=db_credentials[secret_map[0]])
    # reminder: use engine.dispose() to free up all connections in its pool
    return sa.create_engine(url, connect_args=conn_args)


def sqlalchemy_db_conn(engine, enable_orm=False):
    """
    decorator for starting a database connection (either establishing new
    one or grab from connection pool) in SQLAlchemy
    """
    assert engine, 'argument `engine` has to be SQLAlchemy engine instance \
            , but receive invalid value %s' % (engine)
    if enable_orm:
        from sqlalchemy import orm  as sa_orm
    def inner(func):
        @functools.wraps(func)
        def wrapped(*args, **kwargs):
            result = None
            if enable_orm:
                with sa_orm.Session(engine) as session:
                    kwargs['session'] = session
                    result = func(*args, **kwargs)
            else:
                with engine.connect() as conn:
                    kwargs['conn'] = conn
                    result = func(*args, **kwargs)
            return result
        return wrapped
    return inner


def sqlalchemy_insert(model_cls_path:str, data:list, conn):
    """
    SQLAlchemy helper function for inserting new record to database
    """
    result = None
    model_cls =  import_module_string(model_cls_path)
    ins = model_cls.__table__.insert()
    if len(data) == 1:
        ins = ins.values(**data[0])
        result = conn.execute(ins)
    elif len(data) > 1:
        result = conn.execute(ins, data)
    return result


def _get_mysql_error_response(e, headers, raise_if_not_handled):
    from requests.status_codes import codes as requests_codes
    from MySQLdb.constants   import ER as MySqlErrorCode
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
    from MySQLdb._exceptions import OperationalError as MySqlOperationalError
    status = 500 # defaults to internal_server_error
    cause = e.__cause__
    if cause:
        _err_resp_map = {
            MySqlOperationalError: _get_mysql_error_response
        }
        handler = _err_resp_map[type(cause)]
        status = handler(e=cause, headers=headers, raise_if_not_handled=raise_if_not_handled)
    return status


def db_middleware_exception_handler(func):
    from django.http  import HttpResponse
    from django.db.utils   import OperationalError
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


def monkeypatch_django_db_mysql_schema():
    from django.db.backends.mysql.schema import DatabaseSchemaEditor as MysqlDBschemaEditor
    origin_delete_unique_sql = MysqlDBschemaEditor._delete_unique_sql

    class PatchedDatabaseSchemaEditor:
        def _delete_unique_sql(self, model, name, condition=None, deferrable=None):
            """
            workaround for error when  dropping unique index that contains referential-key
            columns , that will cuase following error in mysql / mariadb:

                1553,  Cannot drop index '<UNIQUE_INDEX_NAME>': needed in a foreign key constraint
            """
            _cond = condition
            _statement = origin_delete_unique_sql(self=self, model=model, name=name,
                    condition=condition, deferrable=deferrable)
            if getattr(self, '_unique_constraint_contains_fk', False) is True:
                # wrap up extra SQL statement to temporarily disable all keys of the given table,
                SQL_ENABLE_KEYS  = 'ALTER TABLE %(table)s ENABLE KEYS'
                SQL_DISABLE_KEYS = 'ALTER TABLE %(table)s DISABLE KEYS'
                expanded_templates = [SQL_DISABLE_KEYS, _statement.template, SQL_ENABLE_KEYS]
                _statement.template = ';'.join(expanded_templates)
            return _statement

    if not hasattr(MysqlDBschemaEditor._delete_unique_sql , '_patched'):
        MysqlDBschemaEditor._delete_unique_sql = PatchedDatabaseSchemaEditor._delete_unique_sql
        setattr(MysqlDBschemaEditor._delete_unique_sql , '_patched', None)


def monkeypatch_django_db_base_schema():
    from django.db.backends.base.schema import BaseDatabaseSchemaEditor
    class PatchedDatabaseSchemaEditor:
        def table_sql(self, model):
            """Take a model and return its table definition."""
            # Add any unique_togethers (always deferred, as some fields might be
            # created afterwards, like geometry fields with some backends).
            for fields in model._meta.unique_together:
                columns = [model._meta.get_field(field).column for field in fields]
                self.deferred_sql.append(self._create_unique_sql(model, columns))
            # Create column SQL, add FK deferreds if needed.
            column_sqls = []
            params = []
            for field in model._meta.local_fields:
                # SQL.
                definition, extra_params = self.column_sql(model, field)
                if definition is None:
                    continue
                # Check constraints can go on the column SQL here.
                db_params = field.db_parameters(connection=self.connection)
                if db_params['check']:
                    definition.append(self.sql_check_constraint % db_params)
                # Autoincrement SQL (for backends with inline variant).
                col_type_suffix = field.db_type_suffix(connection=self.connection)
                if col_type_suffix:
                    definition.append(col_type_suffix)
                params.extend(extra_params)
                # FK.
                if field.remote_field and field.db_constraint:
                    to_table = field.remote_field.model._meta.db_table
                    to_column = field.remote_field.model._meta.get_field(field.remote_field.field_name).column
                    if self.sql_create_inline_fk:
                        inline_fk = self.sql_create_inline_fk % {
                            'to_table': self.quote_name(to_table),
                            'to_column': self.quote_name(to_column),
                        }
                        definition.append(inline_fk)
                    elif self.connection.features.supports_foreign_keys:
                        self.deferred_sql.append(self._create_fk_sql(model, field, '_fk_%(to_table)s_%(to_column)s'))
                # determine where to put the column name(s) in the sql
                self._table_sql_colnames_syntax(field=field, definition=definition)
                definition = ' '.join(definition)
                # Add the SQL to our big list.
                column_sqls.append(definition)
                ##if type(field).__name__ == 'CompoundPrimaryKeyField':
                ##    import pdb
                ##    pdb.set_trace()
                # Autoincrement SQL (for backends with post table definition
                # variant).
                if field.get_internal_type() in ('AutoField', 'BigAutoField', 'SmallAutoField'):
                    autoinc_sql = self.connection.ops.autoinc_sql(model._meta.db_table, field.column)
                    if autoinc_sql:
                        self.deferred_sql.extend(autoinc_sql)
            constraints = [constraint.constraint_sql(model, self) for constraint in model._meta.constraints]
            sql = self.sql_create_table % {
                'table': self.quote_name(model._meta.db_table),
                'definition': ', '.join(constraint for constraint in (*column_sqls, *constraints) if constraint),
            }
            if model._meta.db_tablespace:
                tablespace_sql = self.connection.ops.tablespace_sql(model._meta.db_tablespace)
                if tablespace_sql:
                    sql += ' ' + tablespace_sql
            return sql, params


        def _table_sql_colnames_syntax(self, field, definition):
            if field.get_internal_type() == 'CompoundPrimaryKeyField':
                idx = definition.index('PRIMARY KEY') + 1
                cols = field.db_columns
                cols = list(map(self.quote_name, cols))
                cols = '(%s)' % ','.join(cols)
            else:
                idx  = 0
                cols = self.quote_name(field.column)
            definition.insert(idx, cols)

        def add_field(self, model, field):
            """
            Create a field on a model. Usually involves adding a column, but may
            involve adding a table instead (for M2M fields).
            """
            # Special-case implicit M2M tables
            if field.many_to_many and field.remote_field.through._meta.auto_created:
                return self.create_model(field.remote_field.through)
            # Get the column's definition
            definition, params = self.column_sql(model, field, include_default=True)
            # It might not actually have a column behind it
            if definition is None:
                return
            # Check constraints can go on the column SQL here
            db_params = field.db_parameters(connection=self.connection)
            if db_params['check']:
                definition.append(self.sql_check_constraint % db_params)
            if field.remote_field and self.connection.features.supports_foreign_keys and field.db_constraint:
                constraint_suffix = '_fk_%(to_table)s_%(to_column)s'
                # Add FK constraint inline, if supported.
                if self.sql_create_column_inline_fk:
                    to_table = field.remote_field.model._meta.db_table
                    to_column = field.remote_field.model._meta.get_field(field.remote_field.field_name).column
                    inline_fk = self.sql_create_column_inline_fk % {
                        'name': self._fk_constraint_name(model, field, constraint_suffix),
                        'column': self.quote_name(field.column),
                        'to_table': self.quote_name(to_table),
                        'to_column': self.quote_name(to_column),
                        'deferrable': self.connection.ops.deferrable_sql()
                    }
                    definition.append(inline_fk)
                # Otherwise, add FK constraints later.
                else:
                    self.deferred_sql.append(self._create_fk_sql(model, field, constraint_suffix))
            # Build the SQL and run it
            sql = self.sql_create_column % {
                "table": self.quote_name(model._meta.db_table),
                "column": self.quote_name(field.column),
                "definition": ' '.join(definition),
            }
            self.execute(sql, params)
            # Drop the default if we need to
            # (Django usually does not use in-database defaults)
            if not self.skip_default(field) and self.effective_default(field) is not None:
                changes_sql, params = self._alter_column_default_sql(model, None, field, drop=True)
                sql = self.sql_alter_column % {
                    "table": self.quote_name(model._meta.db_table),
                    "changes": changes_sql,
                }
                self.execute(sql, params)
            # Add an index, if required
            self.deferred_sql.extend(self._field_indexes_sql(model, field))
            # Reset connection if required
            if self.connection.features.connection_persists_old_columns:
                self.connection.close()


        def column_sql(self, model, field, include_default=False):
            """
            Take a field and return its column definition.
            The field must already have had set_attributes_from_name() called.
            """
            # Get the column's type and use that as the basis of the SQL
            db_params = field.db_parameters(connection=self.connection)
            # Check for fields that aren't actually columns (e.g. M2M)
            if db_params['type'] is None:
                return None, None
            sql = [db_params['type']]
            params = []
            # If we were told to include a default value, do so
            include_default = include_default and not self.skip_default(field)
            if include_default:
                default_value = self.effective_default(field)
                column_default = 'DEFAULT ' + self._column_default_sql(field)
                if default_value is not None:
                    if self.connection.features.requires_literal_defaults:
                        # Some databases can't take defaults as a parameter (oracle)
                        # If this is the case, the individual schema backend should
                        # implement prepare_default
                        column_default = column_default % self.prepare_default(default_value)
                    else:
                        params += [default_value]
                    sql.append(column_default)
            sql.append(self._column_sql_null_syntax(field=field))
            # Primary key/unique outputs
            if field.primary_key:
                sql.append('PRIMARY KEY')
            elif field.unique:
                sql.append('UNIQUE')
            # Optionally add the tablespace if it's an implicitly indexed column
            tablespace = field.db_tablespace or model._meta.db_tablespace
            if tablespace and self.connection.features.supports_tablespaces and field.unique:
                tbsp_syntax = " %s" % self.connection.ops.tablespace_sql(tablespace, inline=True)
                sql.append(tbsp_syntax)
            # Return the sql
            return sql, params


        def _column_sql_null_syntax(self, field):
            out = ''
            if self._column_sql_enable_null_syntax(field=field) :
                # Work out nullability
                null = field.null
                # Oracle treats the empty string ('') as null, so coerce the null
                # option whenever '' is a possible value.
                if (field.empty_strings_allowed and not field.primary_key and
                        self.connection.features.interprets_empty_strings_as_nulls):
                    null = True
                if null and not self.connection.features.implied_column_null:
                    out = "NULL"
                elif not null:
                    out = "NOT NULL"
            return out

        def _column_sql_enable_null_syntax(self, field):
            """
            subclasses can override this function, or developers are
            free to monkey-patch this smaller function
            """
            return field.get_internal_type() != 'CompoundPrimaryKeyField'

        def skip_default(self, field):
            """
            Some backends don't accept default values for certain columns types
            (i.e. MySQL longtext and longblob).
            """
            # composite primary key does not allow default value
            return  field.get_internal_type() == 'CompoundPrimaryKeyField'
    ## end of class PatchedDatabaseSchemaEditor

    if not hasattr(BaseDatabaseSchemaEditor.table_sql , '_patched'):
        BaseDatabaseSchemaEditor.table_sql = PatchedDatabaseSchemaEditor.table_sql
        BaseDatabaseSchemaEditor._table_sql_colnames_syntax = PatchedDatabaseSchemaEditor._table_sql_colnames_syntax
        setattr(BaseDatabaseSchemaEditor.table_sql , '_patched', None)

    if not hasattr(BaseDatabaseSchemaEditor.column_sql , '_patched'):
        BaseDatabaseSchemaEditor.column_sql = PatchedDatabaseSchemaEditor.column_sql
        BaseDatabaseSchemaEditor._column_sql_null_syntax = PatchedDatabaseSchemaEditor._column_sql_null_syntax
        BaseDatabaseSchemaEditor._column_sql_enable_null_syntax  = PatchedDatabaseSchemaEditor._column_sql_enable_null_syntax
        setattr(BaseDatabaseSchemaEditor.column_sql , '_patched', None)

    if not hasattr(BaseDatabaseSchemaEditor.skip_default , '_patched'):
        BaseDatabaseSchemaEditor.skip_default = PatchedDatabaseSchemaEditor.skip_default
        setattr(BaseDatabaseSchemaEditor.skip_default , '_patched', None)

    if not hasattr(BaseDatabaseSchemaEditor.add_field , '_patched'):
        BaseDatabaseSchemaEditor.add_field = PatchedDatabaseSchemaEditor.add_field
        setattr(BaseDatabaseSchemaEditor.add_field , '_patched', None)
## end of monkeypatch_django_db_base_schema


def monkeypatch_django_model_unik_constraint():
    from django.db.models.constraints import UniqueConstraint
    origin_remove_sql = UniqueConstraint.remove_sql
    def patched_remove_sql(self, model, schema_editor):
        # check whether referential-key column is included in the unique constraint
        for fd in model._meta.local_fields:
            if not fd.name in self.fields:
                continue
            if fd.get_internal_type() == 'ForeignKey':
                # currently only mysql schema editor will reference this attribute value
                setattr(schema_editor, '_unique_constraint_contains_fk', True)
                break
        #import pdb
        #pdb.set_trace()
        sql = origin_remove_sql(self=self, model=model, schema_editor=schema_editor)
        return sql

    if not hasattr(UniqueConstraint.remove_sql , '_patched'):
        UniqueConstraint.remove_sql = patched_remove_sql
        setattr(UniqueConstraint.remove_sql , '_patched', None)



def monkeypatch_django_db():
    monkeypatch_django_db_base_schema()
    monkeypatch_django_db_mysql_schema()
    monkeypatch_django_model_unik_constraint()


class ServiceModelRouter:
    # commonly used apps for all services
    _common_app_labels = ['contenttypes',]

    def __init__(self, *args, **kwargs):
        from django.conf  import settings as django_settings
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


class BaseDatabaseError(Exception):
    pass

class EmptyDataRowError(BaseDatabaseError):
    pass

