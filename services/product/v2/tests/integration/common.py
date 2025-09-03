import os
from importlib import import_module
from typing import Dict, List, Tuple, Optional
from unittest.mock import patch

import pytest
import pytest_asyncio
from blacksheep import Response
from blacksheep.testing import TestClient
from blacksheep.testing.helpers import HeadersType
from blacksheep.contents import JSONContent

from ecommerce_common.models.constants import ROLE_ID_STAFF
from ecommerce_common.tests.common import KeystoreMixin
from product.entry.web import app
from product.api.dto import TagCreateReqDto, AttrDataTypeDto, AttrCreateReqDto
from product.util import QuotaMaterialCode

app_setting_path = os.environ["APP_SETTINGS"]
app_setting = import_module(app_setting_path)

from ecommerce_common.models.enums.base import AppCodeOptions  # noqa: E402

app_setting.KEYSTORE["persist_secret_handler"] = {
    "module_path": "ecommerce_common.auth.keystore.JWKSFilePersistHandler",
    "init_kwargs": {
        "filepath": app_setting.SYS_BASE_PATH.joinpath(  # noqa: F405
            "tmp/cache/dev/jwks/privkey/current.json"
        ),
        "name": "secret4itest",
        "expired_after_days": 10,
        "flush_threshold": 8,
    },
}

app_setting.KEYSTORE.pop("persist_pubkey_handler")

app_setting.KEYSTORE["persist_pubkey_handler"] = {  # noqa: F405
    "module_path": "ecommerce_common.auth.keystore.JWKSFilePersistHandler",
    "init_kwargs": {
        "filepath": app_setting.SYS_BASE_PATH.joinpath(  # noqa: F405
            "tmp/cache/dev/jwks/pubkey/current.json"
        ),
        "name": "pubkey4itest",
        "expired_after_days": 13,
        "flush_threshold": 10,
    },
}


class ITestKeystore(KeystoreMixin):
    _keystore_init_config = {
        "keystore": app_setting.KEYSTORE["keystore"],
        "persist_secret_handler": app_setting.KEYSTORE["persist_secret_handler"],
        "persist_pubkey_handler": app_setting.KEYSTORE["persist_pubkey_handler"],
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


def add_auth_header(
    client: ITestClient,
    headers: HeadersType,
    usr_id: int,
    perms: List[str],
    quotas: Optional[List[Dict]] = None,
):
    app_code = AppCodeOptions.product.value[0]
    quotas = quotas or []
    default_quota = {
        "app_code": app_code,
        "mat_code": QuotaMaterialCode.NumAttributesPerItem.value,
        "maxnum": 93,
    }
    quotas.append(default_quota)
    auth_data = {
        "id": usr_id,
        "privilege_status": ROLE_ID_STAFF,
        "quotas": quotas,
        "roles": [{"app_code": app_code, "codename": p} for p in perms],
    }
    encoded_token = client.keystore.gen_access_token(
        auth_data, audience=["product"], issuer=app_setting.JWT_ISSUER
    )
    headers["Authorization"] = f"Bearer {encoded_token}"


@pytest_asyncio.fixture(scope="session", loop_scope="session")
async def mock_client(itest_keystore) -> ITestClient:
    await app.start()
    return ITestClient(app, itest_keystore)


async def create_one_tag(
    client: ITestClient, usr_id: int, body: TagCreateReqDto, expect_status: int
) -> Dict:
    headers: Dict[str, str] = {}
    add_auth_header(client, headers, usr_id, ["add_producttag"])
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
    usr_id: int,
    data: List[Tuple[str, AttrDataTypeDto]],
    expect_status: int,
) -> Response:
    headers: Dict[str, str] = {}
    add_auth_header(client, headers, usr_id, ["add_productattributetype"])

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
