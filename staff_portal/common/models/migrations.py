import logging

from django.conf  import  settings as django_settings
from django.db    import  migrations, router, models
from django.db.utils import OperationalError, ProgrammingError

_logger = logging.getLogger(__name__)

class ExtendedRunPython(migrations.RunPython):
    """
    extended from original RunPython class by :
        * providing user-defined arguments to forward function and reverse function
    """
    def __init__(self, code, reverse_code=None, atomic=None, hints=None, elidable=False,
            code_kwargs=None, reverse_code_kwargs=None):
        if code_kwargs:
            self._code_kwargs = code_kwargs
        if reverse_code_kwargs:
            self._reverse_code_kwargs = reverse_code_kwargs
        super().__init__(code=code, reverse_code=reverse_code, atomic=atomic,
                hints=hints, elidable=elidable)

    def deconstruct(self):
        kwargs = {
            'code': self.code,
        }
        if self.reverse_code is not None:
            kwargs['reverse_code'] = self.reverse_code
        if self.atomic is not None:
            kwargs['atomic'] = self.atomic
        if self.hints:
            kwargs['hints'] = self.hints
        if hasattr(self, '_code_kwargs'):
            kwargs['code_kwargs'] = getattr(self, '_code_kwargs')
        if hasattr(self, '_reverse_code_kwargs'):
            kwargs['reverse_code_kwargs'] = getattr(self, '_reverse_code_kwargs')
        return (self.__class__.__qualname__ , [], kwargs)

    def database_forwards(self, app_label, schema_editor, from_state, to_state):
        from_state.clear_delayed_apps_cache()
        if router.allow_migrate(schema_editor.connection.alias, app_label, **self.hints):
            kwargs = getattr(self, '_code_kwargs', {})
            kwargs['app_label'] = app_label
            kwargs['to_state_apps'] = to_state.apps
            self.code(from_state.apps, schema_editor, **kwargs)

    def database_backwards(self, app_label, schema_editor, from_state, to_state):
        if self.reverse_code is None:
            raise NotImplementedError("You cannot reverse this operation")
        if router.allow_migrate(schema_editor.connection.alias, app_label, **self.hints):
            kwargs = getattr(self, '_reverse_code_kwargs', {})
            kwargs['app_label'] = app_label
            kwargs['to_state_apps'] = to_state.apps
            self.reverse_code(from_state.apps, schema_editor, **kwargs)




class AlterTablePrivilege(ExtendedRunPython):
    """
    When there are new models to create, or existing models to rename or delete,
    this operation class is  responsible for updating necessary privileges for
    given database user.
    Note that each instance of this class is supposed to work with ONLY one
    database setup, for the complex migration that includes multi-database updates,
    there should be multiple instances of ths class created & dedicated to that migration.
    """
    PRIVILEGE_MAP = {
        # to create tuple with only one item, you must add comma after the item
        # , otherwise python will NOT recognize the variable as a tuple, instead python
        # treat the variable as the data type of the only item .
        'READ_ONLY' : ('SELECT',),
        'EDIT_ONLY' : ('UPDATE',),
        'WRITE_ONLY': ('INSERT','DELETE','UPDATE'),
        'READ_WRITE': ('SELECT', 'INSERT','DELETE','UPDATE'),
    }

    ACCEPTED_OPERATIONS = (migrations.CreateModel, migrations.DeleteModel, migrations.AlterModelTable)

    def __init__(self, autogen_ops, db_setup_tag, **kwargs):
        self._db_setup_tag = db_setup_tag
        self._extract_table_names(autogen_ops)
        code_kwargs = {'operation': self}
        super().__init__(code=_forward_privilege_setup, reverse_code=_backward_privilege_setup,
                code_kwargs=code_kwargs, reverse_code_kwargs=code_kwargs )
        # TODO, figure out how the order of operations affects state change
        # always insert this operation into both ends of the operation list, in case current migration
        # includes complex operations, e.g. create, delete, rename table operations are in
        # a single migration .
        autogen_ops.insert(0, self)
        autogen_ops.append(self)
        self._first_run = True


    def _extract_table_names(self, autogen_ops):
        add_models = []
        rm_models = []
        rename_tables = []

        for op in autogen_ops:
            _priv_lvl = getattr(op, '_priv_lvl', None)
            if _priv_lvl is None:
                continue # discard
            _priv_lvl = ','.join(_priv_lvl)
            if isinstance(op, migrations.CreateModel):
                item = {'model_name': op.name, 'new_table_name': op.options.get('db_table',None), 'priv_lvl': _priv_lvl}
                add_models.append(item)
                for fd in op.fields:
                    if isinstance(fd[1], models.ManyToManyField):
                        item = {'model_name': op.name, 'new_table_name': op.options.get('db_table',None),
                                'm2m_fd': fd[0] , 'priv_lvl': _priv_lvl}
                        add_models.append(item)
            elif isinstance(op, migrations.DeleteModel):
                ## TODO, what if the migration deletes a table that contains m2m fields ?
                item = {'model_name': op.name, 'priv_lvl': _priv_lvl }
                rm_models.append(item)
            elif isinstance(op, migrations.AlterModelTable):
                item = {'new_table_name': op.table, 'model_name': op.name, 'priv_lvl': _priv_lvl}
                rename_tables.append(item)
        self._add_models = add_models
        self._rm_models  = rm_models
        self._rename_tables = rename_tables


    def _execute_raw_sql(self, cursor, sql_pattern, priv_lvl, db_name, table_name, db_user, db_host):
        sql = sql_pattern % (priv_lvl, db_name, table_name, db_user, db_host)
        log_arg = ['renderred_sql', sql]
        _logger.debug(None, *log_arg)
        cursor.execute(sql)


    def _grant_table_priv(self, model_list, app_label, cursor, reverse, db_name, db_user, db_host, log_arg):
        done_with_warnings  = False
        if reverse is self._first_run:
            sql_pattern_add = "REVOKE %s ON `%s`.`%s` FROM %s@%s" if reverse is True else "GRANT %s ON `%s`.`%s` TO %s@%s"
            for item  in model_list:
                if item['new_table_name'] is None: # db_table not specified
                    if item.get('m2m_fd', None):
                        item['new_table_name'] = '%s_%s_%s' % (app_label.lower(), item['model_name'].lower(), item['m2m_fd'].lower())
                    else:
                        item['new_table_name'] = '%s_%s' % (app_label.lower(), item['model_name'].lower())
                else:
                    if item.get('m2m_fd', None):
                        item['new_table_name'] = '%s_%s' % (item['new_table_name'].lower(), item['m2m_fd'].lower())
                try: # the pattern below works only in MySQL, TODO, refactor
                    log_arg.extend(['add_table', item['new_table_name']])
                    self._execute_raw_sql(cursor, sql_pattern_add, item['priv_lvl'], db_name, item['new_table_name'], db_user, db_host)
                except (OperationalError, ProgrammingError) as e:
                    log_arg.extend(['err_msg_grant', self._get_error_msg(e)])
                    done_with_warnings = True
        return done_with_warnings


    def _revoke_table_priv(self, model_list, apps, app_label, cursor, reverse, db_name, db_user, db_host, log_arg):
        """
        for revoke privilege operation :
        * if not reverse, get `apps` before changing the state and database schema
        * if     reverse, get `apps` after  changing the state and database schema
        """
        done_with_warnings  = False
        if reverse is not self._first_run:
            sql_pattern_del = "GRANT %s ON `%s`.`%s` TO %s@%s" if reverse is True else "REVOKE %s ON `%s`.`%s` FROM %s@%s"
            for item in model_list:
                try:
                    fakemodel = apps.get_model(app_label, item['model_name'])
                    table_name = fakemodel._meta.db_table
                    log_arg.extend(['rm_table', table_name])
                    self._execute_raw_sql(cursor, sql_pattern_del, item['priv_lvl'], db_name, table_name, db_user, db_host)
                except (OperationalError, LookupError) as e:
                    log_arg.extend(['err_msg_revoke', self._get_error_msg(e)])
                    done_with_warnings = True
        return done_with_warnings


    def _common_handler(self, apps, schema_editor, app_label, reverse=False, **kwargs):
        loglevel = logging.INFO
        log_arg = ['first_run', self._first_run, 'app_label', app_label, 'reverse', reverse,
                'apps', apps, 'to_state_apps', kwargs['to_state_apps']]
        try:
            conn = schema_editor.connection
            caller_db_setup = conn.settings_dict
            migration_caller = '%s@%s:%s' % (caller_db_setup['USER'], caller_db_setup['HOST'], caller_db_setup['PORT'])
            log_arg.extend(['migration_caller', migration_caller,  'conn', conn, 'conn.connection', type(conn.connection)])

            target_db_setup = django_settings.DATABASES[self._db_setup_tag]
            db_name = target_db_setup['NAME']
            db_user = target_db_setup['USER']
            db_host = target_db_setup['HOST']
            log_arg.extend(['db_name', db_name, 'db_user', db_user, 'db_host', db_host])

            assert caller_db_setup['HOST'] == target_db_setup['HOST'], "DB hosts mismatch"
            assert caller_db_setup['PORT'] == target_db_setup['PORT'], "DB server ports mismatch"
            assert caller_db_setup['NAME'] == target_db_setup['NAME'], "database names mismatch"

            done_with_warnings  = False
            with conn.cursor() as cursor:
                apps_for_delete = apps if reverse is False else kwargs['to_state_apps']  # choose proper apps for delete operation
                log_arg.extend(['apps_for_delete', apps_for_delete])
                done_with_warnings |= self._grant_table_priv(model_list=self._add_models, cursor=cursor, reverse=reverse,
                        app_label=app_label, db_name=db_name, db_user=db_user, db_host=db_host, log_arg=log_arg)
                done_with_warnings |= self._grant_table_priv(model_list=self._rename_tables, cursor=cursor, reverse=reverse,
                        app_label=app_label, db_name=db_name, db_user=db_user, db_host=db_host, log_arg=log_arg)
                done_with_warnings |= self._revoke_table_priv(model_list=self._rm_models, apps=apps_for_delete,
                        app_label=app_label, cursor=cursor, reverse=reverse, db_name=db_name, db_user=db_user,
                        db_host=db_host, log_arg=log_arg)
                done_with_warnings |= self._revoke_table_priv(model_list=self._rename_tables, apps=apps_for_delete,
                        app_label=app_label, cursor=cursor, reverse=reverse, db_name=db_name, db_user=db_user,
                        db_host=db_host, log_arg=log_arg)
            if done_with_warnings:
                loglevel = logging.WARNING
            self._first_run = False
        except Exception as e:
            loglevel = logging.ERROR
            log_arg.extend(['err_msg', self._get_error_msg(e)])
        _logger.log(loglevel, None, *log_arg)
    ## end of _common_handler


    def _get_error_msg(self, e):
        e_cls = type(e)
        e_cls_name = '%s.%s' % (e_cls.__module__ , e_cls.__qualname__)
        err_msg = list(map(lambda x: str(x) , e.args))
        err_msg.append(e_cls_name)
        err_msg = ', '.join(err_msg)
        return err_msg
## end of AlterTablePrivilege


# internal call method for AlterTablePrivilege class
def _forward_privilege_setup(apps, schema_editor, app_label, operation, **kwargs):
    operation._common_handler(apps=apps, schema_editor=schema_editor, app_label=app_label,
            reverse=False, **kwargs)


def _backward_privilege_setup(apps, schema_editor, app_label, operation, **kwargs):
    operation._common_handler(apps=apps, schema_editor=schema_editor, app_label=app_label,
            reverse=True, **kwargs)


