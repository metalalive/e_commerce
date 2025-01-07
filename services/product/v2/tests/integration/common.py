import os
from importlib import import_module
from typing import Dict, List, Tuple
from unittest.mock import patch

import pytest
import pytest_asyncio
from blacksheep import Response
from blacksheep.testing import TestClient
from blacksheep.testing.helpers import HeadersType
from blacksheep.contents import JSONContent
from curator.cli import run as run_curator

from ecommerce_common.tests.common import KeystoreMixin
from ecommerce_common.models.constants import ROLE_ID_STAFF
from product.entry.web import app
from product.api.dto import TagCreateReqDto, AttrDataTypeDto, AttrCreateReqDto

app_setting_path = os.environ["APP_SETTINGS"]
app_setting = import_module(app_setting_path)


@pytest.fixture(scope="session")
def es_mapping_init():
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


class ITestKeystore(KeystoreMixin):
    _keystore_init_config = {
        "keystore": app_setting.KEYSTORE["keystore"],
        "persist_secret_handler": app_setting.KEYSTORE["persist_secret_handler_test"],
        "persist_pubkey_handler": app_setting.KEYSTORE["persist_pubkey_handler_test"],
    }


@pytest.fixture(scope="session")
def itest_keystore():
    ks = ITestKeystore()
    ks._setup_keystore()
    try:
        yield ks
    finally:
        ks._teardown_keystore()


class ITestClient(TestClient):
    def __init__(self, app, kstore: ITestKeystore, *args, **kwargs):
        super().__init__(app, *args, **kwargs)
        self._kstore = kstore

    @property
    def keystore(self) -> ITestKeystore:
        return self._kstore

    async def post(self, *args, **kwargs) -> Response:
        with patch("jwt.PyJWKClient.fetch_data", self.keystore._mocked_get_jwks):
            return await super().post(*args, **kwargs)

    async def put(self, *args, **kwargs) -> Response:
        with patch("jwt.PyJWKClient.fetch_data", self.keystore._mocked_get_jwks):
            return await super().put(*args, **kwargs)

    async def get(self, *args, **kwargs) -> Response:
        with patch("jwt.PyJWKClient.fetch_data", self.keystore._mocked_get_jwks):
            return await super().get(*args, **kwargs)

    async def delete(self, *args, **kwargs) -> Response:
        with patch("jwt.PyJWKClient.fetch_data", self.keystore._mocked_get_jwks):
            return await super().delete(*args, **kwargs)


def add_auth_header(client: ITestClient, headers: HeadersType):
    auth_data = {
        "id": 1234,
        "privilege_status": ROLE_ID_STAFF,
        "quotas": [{"app_code": 5566, "mat_code": 1, "maxnum": -1}],
        "roles": [{"app_code": 5566, "codename": "add_saleableitem"}],
    }  # TODO: complete quota and roles from different use cases
    encoded_token = client.keystore.gen_access_token(
        auth_data, audience=["product"], issuer=app_setting.JWT_ISSUER
    )
    headers["Authorization"] = f"Bearer {encoded_token}"


@pytest_asyncio.fixture(scope="session", loop_scope="session")
async def mock_client(es_mapping_init, itest_keystore) -> ITestClient:
    await app.start()
    return ITestClient(app, itest_keystore)


async def create_one_tag(
    client: ITestClient, body: TagCreateReqDto, expect_status: int
) -> Dict:
    headers: Dict[str, str] = {}
    add_auth_header(client, headers)
    expect_label = body.name
    expect_parent = body.parent
    resp = await client.post(
        path="/tag",
        headers=headers,
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
    client: ITestClient,
    data: List[Tuple[str, AttrDataTypeDto]],
    expect_status: int,
) -> Response:
    headers: Dict[str, str] = {}
    add_auth_header(client, headers)

    def setup_create_req(d: Tuple[str, AttrDataTypeDto]) -> AttrCreateReqDto:
        out = AttrCreateReqDto(name=d[0], dtype=d[1].value)
        return out

    reqbody = list(map(setup_create_req, data))
    resp = await client.post(
        path="/attributes",
        headers=headers,
        content=JSONContent(reqbody),
        cookies=None,
    )
    assert resp.status == expect_status
    return resp
