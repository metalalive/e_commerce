from curator.cli import run as run_curator
import pytest_asyncio


@pytest_asyncio.fixture(scope="session", loop_scope="session")
async def es_mapping_init(app_setting):
    base_path = app_setting.APP_BASE_PATH
    cfg_path = base_path.joinpath("settings/elastic_curator.yaml")
    action_file_rel_paths = [
        "attri_label/action_0001.yaml",
        "tag/action_0001.yaml",
        "saleable_item/action_0001.yaml",
    ]

    def _run_curator(relpath):
        app_path = "src/product/migrations/elastic_curator/%s" % relpath
        actionfile_path = base_path.joinpath(app_path)
        run_curator(
            config=str(cfg_path), action_file=str(actionfile_path), dry_run=False
        )

    list(map(_run_curator, action_file_rel_paths))
    yield
    app_path = "tests/unit/adapter/repository/elasticsearch/action_teardown_test.yaml"
    actionfile_path = base_path.joinpath(app_path)
    run_curator(config=str(cfg_path), action_file=str(actionfile_path), dry_run=False)
