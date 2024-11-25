from typing import Tuple
from blacksheep.contents import JSONContent
import pytest

from product.api.dto import AttrDataTypeDto, AttrCreateReqDto, AttrUpdateReqDto


class TestAttribute:
    @staticmethod
    def setup_create_req(d: Tuple[str, AttrDataTypeDto]) -> AttrCreateReqDto:
        out = AttrCreateReqDto(name=d[0], dtype=d[1].value)
        return out

    @staticmethod
    def setup_update_req(d: Tuple[str, str, AttrDataTypeDto]) -> AttrUpdateReqDto:
        out = AttrUpdateReqDto(id_=d[0], name=d[1], dtype=d[2].value)
        return out

    @pytest.mark.asyncio(loop_scope="session")
    async def test_create(self, mock_client):
        cls = type(self)
        mockdata = [
            ("surface color", AttrDataTypeDto.String),
            ("inner diameter", AttrDataTypeDto.UnsignedInteger),
        ]
        reqbody = list(map(cls.setup_create_req, mockdata))
        resp = await mock_client.post(
            path="/attributes",
            headers=None,
            query=None,
            content=JSONContent(reqbody),
            cookies=None,
        )
        assert resp.status == 201  # expect_status
        respbody = await resp.json()
        assert len(respbody) == len(reqbody)
        assert respbody[0].get("id_", None)
        expect_attrs = [(d[0], d[1].value) for d in mockdata]
        actual_attrs = [(r["name"], r["dtype"]) for r in respbody]
        assert set(expect_attrs) == set(actual_attrs)

    @pytest.mark.asyncio(loop_scope="session")
    async def test_update(self, mock_client):
        cls = type(self)
        mockdata = [
            ("u028837", "surface color", AttrDataTypeDto.String),
            ("u297011", "inner diameter", AttrDataTypeDto.UnsignedInteger),
        ]
        reqbody = list(map(cls.setup_update_req, mockdata))
        resp = await mock_client.put(
            path="/attributes",
            headers=None,
            query=None,
            content=JSONContent(reqbody),
            cookies=None,
        )
        assert resp.status == 200  # expect_status
        respbody = await resp.json()
        assert len(respbody) == len(reqbody)
        expect_attrs = [(d[0], d[1], d[2].value) for d in mockdata]
        actual_attrs = [(r["id_"], r["name"], r["dtype"]) for r in respbody]
        assert set(expect_attrs) == set(actual_attrs)

    @pytest.mark.asyncio(loop_scope="session")
    async def test_delete(self, mock_client):
        query = {"ids": "iu736,0w237,jgu7B"}
        resp = await mock_client.delete(
            path="/attributes",
            headers=None,
            query=query,
        )
        assert resp.status == 204
