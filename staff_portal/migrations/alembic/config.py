import os
import shutil
from pathlib import Path

from alembic import command
from alembic.config import Config
from alembic.util.exc import CommandError

from common.util.python import format_sqlalchemy_url
from common.util.python import get_credential_from_secrets

class ExtendedConfig(Config):
    def __init__(self, *args, template_base_path:Path=None, **kwargs):
        condition = template_base_path and template_base_path.exists() and template_base_path.is_dir()
        assert condition , 'should be existing path to custom template'
        self._template_base_path = template_base_path
        super().__init__(*args, **kwargs)

    def get_template_directory(self) -> str:
        return str(self._template_base_path)

    def set_url(self, db_credential, driver_label):
        url = format_sqlalchemy_url(driver=driver_label, db_credential=db_credential)
        self.set_main_option(name='sqlalchemy.url', value=url)



def _setup_db_credential(secret_path):
    _secret_map = {
        'site_dba'        : 'backend_apps.databases.site_dba' ,
        'usermgt_service' : 'backend_apps.databases.usermgt_service' ,
    }
    out_map = get_credential_from_secrets(base_folder='staff_portal',
            secret_path=secret_path, secret_map=_secret_map )
    return out_map


def _copy_migration_scripts(src, dst):
    for file_ in src.iterdir():
        if not file_.is_file():
            continue
        if not file_.suffix in ('.py',):
            continue
        shutil.copy(str(file_), dst)


DEFAULT_VERSION_TABLE = 'alembic_version'


def _init_common_params(app_base_path, secret_path):
    migration_base_path = app_base_path.joinpath('migrations')
    cfg_file_path  = app_base_path.joinpath('alembic.ini')
    template_base_path = app_base_path.parent.joinpath('migrations/alembic/templates')
    alembic_cfg = ExtendedConfig(cfg_file_path, template_base_path=template_base_path)
    db_credentials = _setup_db_credential(secret_path)
    return migration_base_path, alembic_cfg, db_credentials


def init_migration(app_settings, orm_base_cls_path, app_init_rev_id='000001', auth_init_rev_id='000002'):
    migration_base_path, alembic_cfg, db_credentials = _init_common_params( \
            secret_path=app_settings.SECRETS_FILE_PATH, app_base_path=app_settings.APP_BASE_PATH)
    command.init(config=alembic_cfg, directory=migration_base_path)
    alembic_cfg.set_main_option(name='app.orm_base', value=orm_base_cls_path)
    # ------------------
    db_credentials['site_dba']['NAME'] = app_settings.DB_NAME
    alembic_cfg.set_url(db_credential=db_credentials['site_dba'], driver_label=app_settings.DRIVER_LABEL)
    alembic_cfg.set_main_option(name='version_table', value=DEFAULT_VERSION_TABLE)
    result = command.revision( config=alembic_cfg, message='create initial tables',
                autogenerate=True, rev_id=app_init_rev_id, depends_on=None, )
    assert result.revision == app_init_rev_id
    command.upgrade(config=alembic_cfg, revision=app_init_rev_id)
    # ------------------
    _copy_migration_scripts(src=app_settings.AUTH_MIGRATION_PATH, dst=migration_base_path.joinpath('versions'))
    db_credentials['site_dba']['NAME'] = app_settings.AUTH_DB_NAME
    alembic_cfg.set_url(db_credential=db_credentials['site_dba'], driver_label=app_settings.DRIVER_LABEL)
    alembic_cfg.set_main_option(name='version_table', value=app_settings.VERSION_TABLE_AUTH_APP)
    command.upgrade(config=alembic_cfg, revision=auth_init_rev_id)


def deinit_migration(app_settings, orm_base_cls_path):
    migration_base_path, alembic_cfg, db_credentials = _init_common_params( \
            secret_path=app_settings.SECRETS_FILE_PATH, app_base_path=app_settings.APP_BASE_PATH)
    alembic_cfg.set_main_option(name='app.orm_base', value=orm_base_cls_path)
    try:
        db_credentials['site_dba']['NAME'] = app_settings.AUTH_DB_NAME
        alembic_cfg.set_url(db_credential=db_credentials['site_dba'], driver_label=app_settings.DRIVER_LABEL)
        alembic_cfg.set_main_option(name='version_table', value=app_settings.VERSION_TABLE_AUTH_APP)
        command.downgrade(config=alembic_cfg, revision='base')
        db_credentials['site_dba']['NAME'] = app_settings.DB_NAME
        alembic_cfg.set_url(db_credential=db_credentials['site_dba'], driver_label=app_settings.DRIVER_LABEL)
        alembic_cfg.set_main_option(name='version_table', value=DEFAULT_VERSION_TABLE) # must not be NULL or empty string
        command.downgrade(config=alembic_cfg, revision='base')
    except CommandError as e:
        pos = e.args[0].lower().find('path doesn\'t exist')
        if pos < 0:
            raise
    if migration_base_path.exists():
        shutil.rmtree(migration_base_path)

