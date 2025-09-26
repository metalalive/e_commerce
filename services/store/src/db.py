from typing import Dict, Tuple, Optional
from pathlib import Path
from sqlalchemy.ext.asyncio import create_async_engine

from ecommerce_common.util import get_credential_from_secrets


def format_sqlalchemy_url(driver: str, db_credential: Dict) -> str:
    """format URL string used in SQLalchemy"""
    url_pattern = "{db_driver}://{username}:{passwd}@{db_host}:{db_port}/{db_name}"
    return url_pattern.format(
        db_driver=driver,
        username=db_credential["USER"],
        passwd=db_credential["PASSWORD"],
        db_host=db_credential["HOST"],
        db_port=db_credential["PORT"],
        db_name=db_credential.get("NAME", ""),
    )


def sqlalchemy_init_engine(
    secrets_file_path,
    secret_map: Tuple[str, str],
    base_folder: Path,
    driver_label: str,
    db_name: str,
    db_host: str,
    db_port: int,
    conn_args: Optional[dict] = None,
):
    conn_args = conn_args or {}
    db_credentials = get_credential_from_secrets(
        base_path=base_folder,
        secret_path=secrets_file_path,
        secret_map=dict([secret_map]),
    )
    chosen_db_credential = db_credentials[secret_map[0]]
    chosen_db_credential["NAME"] = db_name
    chosen_db_credential["HOST"] = db_host
    chosen_db_credential["PORT"] = db_port
    url = format_sqlalchemy_url(driver=driver_label, db_credential=chosen_db_credential)
    # reminder: use engine.dispose() to free up all connections in its pool
    return create_async_engine(url, connect_args=conn_args)
