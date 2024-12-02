import asyncio
import os
from importlib import import_module
from logging.config import fileConfig
from pathlib import Path
from typing import Dict, Union

from sqlalchemy import pool
from sqlalchemy.engine import Connection
from sqlalchemy.ext.asyncio import async_engine_from_config

# proxied from EnvironmentContext instance
from alembic import context

from ecommerce_common.util import (
    import_module_string,
    format_sqlalchemy_url,
    get_credential_from_secrets,
)

app_settings = import_module(os.environ["APP_SETTINGS"])

# this is the Alembic Config object, which provides
# access to the values within the .ini file in use.
config = context.config

# Interpret the config file for Python logging.
# This line sets up loggers basically.
fileConfig(config.config_file_name)

# add your model's MetaData object here
# for 'autogenerate' support
# from myapp import mymodel
# target_metadata = mymodel.Base.metadata


def load_metad_class(path: str):
    cls = import_module_string(dotted_path=path)
    return cls.metadata


assert len(app_settings.ORM_BASE_CLASSES) > 0
target_metadata = list(map(load_metad_class, app_settings.ORM_BASE_CLASSES))

# other values from the config, defined by the needs of env.py,
# can be acquired:
# my_important_option = config.get_main_option("my_important_option")
# ... etc.


def _setup_db_credential() -> Dict:
    base_path: Path = app_settings.SYS_BASE_PATH
    secret_path: Union[Path, str] = app_settings.SECRETS_FILE_PATH
    db_usr_alias: str = app_settings.DB_USER_ALIAS
    _secret_map = {
        db_usr_alias: "backend_apps.databases.%s" % db_usr_alias,
    }
    s_map = get_credential_from_secrets(
        base_path=base_path, secret_path=secret_path, secret_map=_secret_map
    )
    out_map = s_map[db_usr_alias]
    out_map["NAME"] = app_settings.DB_NAME
    return out_map


db_credential = _setup_db_credential()
url = format_sqlalchemy_url(
    driver=app_settings.DRIVER_LABEL, db_credential=db_credential
)
config.set_main_option(name="sqlalchemy.url", value=url)


def run_migrations_offline():
    """Run migrations in 'offline' mode.

    This configures the context with just a URL
    and not an Engine, though an Engine is acceptable
    here as well.  By skipping the Engine creation
    we don't even need a DBAPI to be available.

    Calls to context.execute() here emit the given string to the
    script output.

    """
    url = config.get_main_option("sqlalchemy.url")
    context.configure(
        url=url,
        target_metadata=target_metadata,
        literal_binds=True,
        dialect_opts={"paramstyle": "named"},
    )

    with context.begin_transaction():
        context.run_migrations()


def run_migrations_online():
    """Run migrations in 'online' mode.

    In this scenario we need to create an Engine
    and associate a connection with the context.

    """
    asyncio.run(run_async_migrations())


async def run_async_migrations() -> None:
    connectable = async_engine_from_config(
        config.get_section(config.config_ini_section),
        prefix="sqlalchemy.",
        poolclass=pool.NullPool,
        connect_args={"connect_timeout": 300},
    )

    async with connectable.connect() as connection:
        await connection.run_sync(do_run_migrations)

    await connectable.dispose()


def do_run_migrations(connection: Connection) -> None:
    context.configure(
        connection=connection,
        target_metadata=target_metadata,
        version_table="alembic_migration_table",
    )
    with context.begin_transaction():
        context.run_migrations()


if context.is_offline_mode():
    run_migrations_offline()
else:
    run_migrations_online()
