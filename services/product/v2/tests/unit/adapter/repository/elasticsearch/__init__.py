import os
from click.testing import CliRunner
from curator.cli import cli as run_curator
import pytest_asyncio


@pytest_asyncio.fixture(scope="session", loop_scope="session")
async def es_mapping_init(app_setting):
    runner = CliRunner()
    base_path = app_setting.APP_BASE_PATH
    cfg_path = base_path.joinpath("settings/elastic_curator.yaml")
    es_domain_name = os.environ["DB_ES_HOST"]
    es_port = os.environ["DB_ES_PORT"]
    es_hosts = f"http://{es_domain_name}:{es_port}"
    action_file_rel_paths = [
        "attri_label/action_0001.yaml",
        "tag/action_0001.yaml",
        "saleable_item/action_0001.yaml",
        "saleable_item/action_0002.yaml",
    ]

    def _run_curator(relpath):
        app_path = "src/product/migrations/elastic_curator/%s" % relpath
        actionfile_path = base_path.joinpath(app_path)
        result = runner.invoke(
            run_curator,
            [
                "--config",
                str(cfg_path),
                "--hosts",
                es_hosts,
                str(actionfile_path),  # action_file argument (positional)
            ],
        )
        print("Exit Code:", result.exit_code)
        # print(result.output)

    list(map(_run_curator, action_file_rel_paths))
    yield
    app_path = "tests/unit/adapter/repository/elasticsearch/action_teardown_test.yaml"
    actionfile_path = base_path.joinpath(app_path)
    runner.invoke(
        run_curator, ["--config", str(cfg_path), "--hosts", es_hosts, str(actionfile_path)]
    )
