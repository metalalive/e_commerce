from logging.config import fileConfig

from sqlalchemy import engine_from_config
from sqlalchemy import pool

from alembic import context
from alembic.util import CommandError as AlembicCommandError

from common.util.python import format_sqlalchemy_url, get_credential_from_secrets
# this is the Alembic Config object, which provides
# access to the values within the .ini file in use.
config = context.config

# Interpret the config file for Python logging.
# This line sets up loggers basically.
fileConfig(config.config_file_name)

# add your model's MetaData object here
# for 'autogenerate' support
from fileupload import models
target_metadata = models.Base.metadata
#target_metadata = None

# other values from the config, defined by the needs of env.py,
# can be acquired:
# my_important_option = config.get_main_option("my_important_option")
# ... etc.

def run_migrations_offline(migration_kwargs):
    """Run migrations in 'offline' mode.

    This configures the context with just a URL
    and not an Engine, though an Engine is acceptable
    here as well.  By skipping the Engine creation
    we don't even need a DBAPI to be available.

    Calls to context.execute() here emit the given string to the
    script output.

    """
    # for models in fileupload service
    url = config.get_main_option("sqlalchemy.url")
    context.configure(
        url=url,  literal_binds=True,
        transactional_ddl=True,
        target_metadata=target_metadata,
        dialect_opts={"paramstyle": "named"},
    )
    with context.begin_transaction():
        context.run_migrations(**migration_kwargs)



def run_migrations_online(migration_kwargs):
    """Run migrations in 'online' mode.

    In this scenario we need to create an Engine
    and associate a connection with the context.

    """
    connectable = engine_from_config(
        config.get_section(config.config_ini_section),
        prefix="sqlalchemy.",
        poolclass=pool.NullPool,
    )
    with connectable.connect() as connection:
        context.configure(
            transactional_ddl=True,
            connection=connection, target_metadata=target_metadata
        )
        with context.begin_transaction():
            context.run_migrations(**migration_kwargs)


def _env_init():
    _secret_map = {
        'site_dba'           : 'backend_apps.databases.site_dba' ,
        'usermgt_service'    : 'backend_apps.databases.usermgt_service' ,
        'file_upload_service': 'backend_apps.databases.file_upload_service' ,
    }
    db_credentials = get_credential_from_secrets(base_folder='staff_portal',
            secret_path='./common/data/secrets.json', secret_map=_secret_map )
    url = format_sqlalchemy_url(driver='mysql+pymysql', db_credential=db_credentials['site_dba'])
    config.set_main_option(name='sqlalchemy.url', value=url)
    _migration_kwargs = {
        'is_offline': context.is_offline_mode(),
        'sql_exe': context.execute,
        'auth_url': format_sqlalchemy_url(driver='mysql+pymysql',
                    db_credential=db_credentials['usermgt_service']),
        'service_db': {
            'user': db_credentials['file_upload_service']['USER'],
            'name': db_credentials['file_upload_service']['NAME'],
            'host': db_credentials['file_upload_service']['HOST'],
        }
    }
    try:
        _migration_kwargs['from_revision'] = context.get_starting_revision_argument()
        _migration_kwargs['to_revision'] =  context.get_revision_argument()
    except AlembicCommandError as e:
        #if config.cmd_opts[0].__name__
        pass # raise

    if context.is_offline_mode():
        run_migrations_offline(_migration_kwargs)
    else:
        run_migrations_online(_migration_kwargs)


_env_init()

# alembic is applied for database migration, here are the frequently-used commands
# * alembic -c </PATH/TO/YOUR_CONFIG.ini>  init </PATH/TO/MIGRATION_SETUP>
#   Note that without `-c` option , alembic command only looks for `alembic.ini`
#   at current folder.
#
# * edit </PATH/TO/MIGRATION_SETUP>/env.py , add models metadata of your application
#   to the variable `target_metadata`
#
# * alembic -c </PATH/TO/YOUR_CONFIG.ini>  revision --autogenerate -m "<SHORT_DESCRIPTION>"
#   (Note that you may need to manually edit the auto-generated migration script)
#
# * check static SQL script before committing the new migration, referred to as `offline upgrade`
#   alembic upgrade <FROM_REVISION_ID>:<TO_REVISION_ID> --sql
#
# * in case you want to check all the changes from initial state to the new migration, you can
#   ignore <FROM_REVISION_ID> option in the `upgrade` command:
#   alembic upgrade <TO_REVISION_ID> --sql
#   alembic upgrade <FROM_REVISION_ID>:<TO_REVISION_ID> --sql
#
# * commit a upgrade migration (no auto-fallback for upgrade failure)
#   alembic upgrade <FROM_REVISION_ID>:<TO_REVISION_ID>
#   alembic upgrade <TO_REVISION_ID>
#
# * inspect raw SQL statements for downgrade
#   alembic downgrade  <FROM_REVISION_ID>:<TO_REVISION_ID> --sql
#
# * commit a downgrade migration (note range revision is NOT allowed)
#   alembic downgrade  <TO_REVISION_ID>
#
# * other subcommands to check revisions
#   alembic -c </PATH/TO/YOUR_CONFIG.ini> current
#   alembic -c </PATH/TO/YOUR_CONFIG.ini> history


