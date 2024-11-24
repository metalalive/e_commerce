import os
from importlib import import_module

import pytest
import pytest_asyncio
from blacksheep.testing import TestClient
from curator.cli import run as run_curator

from product.entry.web import app


@pytest.fixture(scope="session")
def es_mapping_init():
    app_setting_path = os.environ["APP_SETTINGS"]
    app_setting = import_module(app_setting_path)
    base_path = app_setting.APP_BASE_PATH
    actionfile_fullpath = base_path.joinpath(
        "src/product/migrations/elastic_curator/tag/action_0001.yaml"
    )
    cfg_fullpath = base_path.joinpath("settings/elastic_curator.yaml")
    run_curator(
        config=str(cfg_fullpath), action_file=str(actionfile_fullpath), dry_run=False
    )
    yield
    actionfile_fullpath = base_path.joinpath(
        "tests/unit/adapter/repository/elasticsearch/action_teardown_test.yaml"
    )  # TODO, improve path setup
    run_curator(
        config=str(cfg_fullpath), action_file=str(actionfile_fullpath), dry_run=False
    )


@pytest_asyncio.fixture(scope="session", loop_scope="session")
async def mock_client(es_mapping_init) -> TestClient:
    await app.start()
    return TestClient(app)
