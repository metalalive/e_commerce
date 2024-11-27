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
    cfg_fullpath = base_path.joinpath("settings/elastic_curator.yaml")
    action_file_rel_paths = ["attri_label/action_0001.yaml", "tag/action_0001.yaml"]

    def _run_curator(relpath):
        app_path = "src/product/migrations/elastic_curator/%s" % relpath
        actionfile_fullpath = base_path.joinpath(app_path)
        run_curator(
            config=str(cfg_fullpath),
            action_file=str(actionfile_fullpath),
            dry_run=False,
        )

    list(map(_run_curator, action_file_rel_paths))
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
