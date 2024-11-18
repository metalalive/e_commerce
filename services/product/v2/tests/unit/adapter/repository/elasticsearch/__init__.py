from curator.cli import run as run_curator
import pytest_asyncio


@pytest_asyncio.fixture(scope="session", loop_scope="session")
async def es_mapping_init(app_setting):
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
    )
    run_curator(
        config=str(cfg_fullpath), action_file=str(actionfile_fullpath), dry_run=False
    )
