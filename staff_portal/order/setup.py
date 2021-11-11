import sys
import os
from pathlib import Path
import shutil

from alembic import command
from alembic.util.exc import CommandError

from common.util.python import get_credential_from_secrets
from migrations.alembic.config import ExtendedConfig
from . import settings


def _setup_db_credential():
    _secret_map = {
        'site_dba'        : 'backend_apps.databases.site_dba' ,
        'usermgt_service' : 'backend_apps.databases.usermgt_service' ,
    }
    db_credentials = get_credential_from_secrets(base_folder='staff_portal',
            secret_path=settings.SECRETS_FILE_PATH, secret_map=_secret_map )
    return db_credentials


def _copy_migration_scripts(src, dst):
    for file_ in src.iterdir():
        if not file_.is_file():
            continue
        if not file_.suffix in ('.py',):
            continue
        shutil.copy(str(file_), dst)


app_base_path  = Path(__file__).resolve(strict=True).parent
migration_base_path = app_base_path.joinpath('migrations')
cfg_file_path  = app_base_path.joinpath('alembic.ini')
template_base_path = app_base_path.parent.joinpath('migrations/alembic/templates')
alembic_cfg = ExtendedConfig(cfg_file_path, template_base_path=template_base_path)
db_credentials = _setup_db_credential()
order_init_rev_id = '000001'
auth_app_rev_id = '000002'


def init_migration():
    command.init(config=alembic_cfg, directory=migration_base_path)
    alembic_cfg.set_main_option(name='app.orm_base', value='order.models.Base')
    # ------------------
    db_credentials['site_dba']['NAME'] = settings.DB_NAME
    alembic_cfg.set_url(db_credential=db_credentials['site_dba'], driver_label='mariadb+mariadbconnector')
    result = command.revision( config=alembic_cfg, message='create initial tables',
                autogenerate=True, rev_id=order_init_rev_id, depends_on=None, )
    assert result.revision == order_init_rev_id
    command.upgrade(config=alembic_cfg, revision=order_init_rev_id)
    # ------------------
    _copy_migration_scripts(src=app_base_path.parent.joinpath('migrations/alembic/order'),
            dst=migration_base_path.joinpath('versions'))
    db_credentials['site_dba']['NAME'] = 'ecommerce_usermgt'
    alembic_cfg.set_url(db_credential=db_credentials['site_dba'], driver_label='mariadb+mariadbconnector')
    command.upgrade(config=alembic_cfg, revision=auth_app_rev_id)


def deinit_migration():
    alembic_cfg.set_main_option(name='app.orm_base', value='order.models.Base')
    try:
        db_credentials['site_dba']['NAME'] = 'ecommerce_usermgt'
        alembic_cfg.set_url(db_credential=db_credentials['site_dba'], driver_label='mariadb+mariadbconnector')
        command.downgrade(config=alembic_cfg, revision='base')
        db_credentials['site_dba']['NAME'] = settings.DB_NAME
        alembic_cfg.set_url(db_credential=db_credentials['site_dba'], driver_label='mariadb+mariadbconnector')
        command.downgrade(config=alembic_cfg, revision='base')
    except CommandError as e:
        pos = e.args[0].lower().find('path doesn\'t exist')
        if pos < 0:
            raise
    if migration_base_path.exists():
        shutil.rmtree(migration_base_path)


if __name__ == '__main__':
    if sys.argv[-1] == 'reverse':
        deinit_migration()
    else:
        init_migration()

