import os
from importlib import import_module
from typing import Dict, List, Tuple

import pytest
import pytest_asyncio
from blacksheep import Response
from blacksheep.testing import TestClient
from blacksheep.contents import JSONContent
from curator.cli import run as run_curator

from product.entry.web import app
from product.api.dto import TagCreateReqDto, AttrDataTypeDto, AttrCreateReqDto


@pytest.fixture(scope="session")
def es_mapping_init():
    app_setting_path = os.environ["APP_SETTINGS"]
    app_setting = import_module(app_setting_path)
    base_path = app_setting.APP_BASE_PATH
    cfg_fullpath = base_path.joinpath("settings/elastic_curator.yaml")
    action_file_rel_paths = [
        "attri_label/action_0001.yaml",
        "saleable_item/action_0001.yaml",
        "tag/action_0001.yaml",
    ]

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


async def create_one_tag(
    client: TestClient, body: TagCreateReqDto, expect_status: int
) -> Dict:
    expect_label = body.name
    expect_parent = body.parent
    resp = await client.post(
        path="/tag",
        headers=None,
        query=None,
        content=JSONContent(body),
        cookies=None,
    )
    assert resp.status == expect_status
    respbody = await resp.json()
    assert respbody["node"].get("id_", None)
    assert respbody["node"]["name"] == expect_label
    assert respbody.get("parent", None) == expect_parent
    return respbody


async def create_many_attri_labels(
    client: TestClient,
    data: List[Tuple[str, AttrDataTypeDto]],
    expect_status: int,
) -> Response:
    def setup_create_req(d: Tuple[str, AttrDataTypeDto]) -> AttrCreateReqDto:
        out = AttrCreateReqDto(name=d[0], dtype=d[1].value)
        return out

    reqbody = list(map(setup_create_req, data))
    resp = await client.post(
        path="/attributes",
        headers=None,
        content=JSONContent(reqbody),
        cookies=None,
    )
    assert resp.status == expect_status
    return resp
