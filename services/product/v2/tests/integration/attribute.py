import asyncio
from typing import Tuple, List, Dict
from blacksheep import Response
from blacksheep.contents import JSONContent
from blacksheep.testing import TestClient
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

    @classmethod
    async def setup_create_many(
        cls,
        client: TestClient,
        data: List[Tuple[str, AttrDataTypeDto]],
        expect_status: int,
    ) -> Response:
        reqbody = list(map(cls.setup_create_req, data))
        resp = await client.post(
            path="/attributes",
            headers=None,
            content=JSONContent(reqbody),
            cookies=None,
        )
        assert resp.status == expect_status
        return resp

    @staticmethod
    async def search_then_verify(
        client: TestClient,
        req_keyword: str,
        expect_data: List[Tuple[str, AttrDataTypeDto]],
    ):
        resp = await client.get(
            path="/attributes",
            headers=None,
            query={"keyword": req_keyword},
            cookies=None,
        )
        assert resp.status == 200
        respbody = await resp.json()
        expect_attrs = [(d[0], d[1].value) for d in expect_data]
        actual_attrs = [(r["name"], r["dtype"]) for r in respbody]
        assert set(expect_attrs) == set(actual_attrs)

    @pytest.mark.asyncio(loop_scope="session")
    async def test_create(self, mock_client):
        cls = type(self)
        mockdata = [
            ("inner colored meat", AttrDataTypeDto.String),
            ("request per seCoNd", AttrDataTypeDto.UnsignedInteger),
            ("rotation per second", AttrDataTypeDto.UnsignedInteger),
            ("digital color spectrum", AttrDataTypeDto.Integer),
            ("num seconds to max speed", AttrDataTypeDto.UnsignedInteger),
            ("stress test qualified", AttrDataTypeDto.Boolean),
            ("glue joint the components", AttrDataTypeDto.String),
        ]
        resp = await cls.setup_create_many(mock_client, mockdata, 201)
        respbody = await resp.json()
        assert len(respbody) == len(mockdata)
        assert respbody[0].get("id_", None)
        expect_attrs = [(d[0], d[1].value) for d in mockdata]
        actual_attrs = [(r["name"], r["dtype"]) for r in respbody]
        assert set(expect_attrs) == set(actual_attrs)
        expect_readdata = [
            ("request per seCoNd", AttrDataTypeDto.UnsignedInteger),
            ("rotation per second", AttrDataTypeDto.UnsignedInteger),
            ("num seconds to max speed", AttrDataTypeDto.UnsignedInteger),
        ]
        await asyncio.sleep(2)  # wait for ElasticSearch refresh documents
        await cls.search_then_verify(
            mock_client,
            req_keyword="second",
            expect_data=expect_readdata,
        )

    @pytest.mark.asyncio(loop_scope="session")
    async def test_update(self, mock_client):
        cls = type(self)
        mockdata = [
            ("5urface cOLOr", AttrDataTypeDto.Boolean),
            ("inner diamEter", AttrDataTypeDto.Integer),
        ]
        resp = await cls.setup_create_many(mock_client, mockdata, 201)
        respbody = await resp.json()

        def _setup_update_data(d: Dict):
            if d["name"] == "5urface cOLOr":
                assert d["dtype"] == AttrDataTypeDto.Boolean.value
                d["name"] = "surface color"
                d["dtype"] = AttrDataTypeDto.String
            elif d["name"] == "inner diamEter":
                assert d["dtype"] == AttrDataTypeDto.Integer.value
                d["name"] = "inner diameter"
                d["dtype"] = AttrDataTypeDto.UnsignedInteger
            return cls.setup_update_req((d["id_"], d["name"], d["dtype"]))

        reqbody = list(map(_setup_update_data, respbody))
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
        expect_attrs = [(r.id_, r.name, r.dtype.value) for r in reqbody]
        actual_attrs = [(r["id_"], r["name"], r["dtype"]) for r in respbody]
        assert set(expect_attrs) == set(actual_attrs)

    @pytest.mark.asyncio(loop_scope="session")
    async def test_delete(self, mock_client):
        cls = type(self)
        mockdata = [
            ("unknown despair lost", AttrDataTypeDto.Boolean),
            ("fearless ice climb", AttrDataTypeDto.String),
            ("everest base camp", AttrDataTypeDto.UnsignedInteger),
            ("meshed boiled pumpkin", AttrDataTypeDto.Integer),
        ]
        resp = await cls.setup_create_many(mock_client, mockdata, 201)
        respbody = await resp.json()
        ids_to_delete = [
            d["id_"]
            for d in respbody
            if d["name"] in ("fearless ice climb", "meshed boiled pumpkin")
        ]
        query = {"ids": ",".join(ids_to_delete)}
        resp = await mock_client.delete(
            path="/attributes",
            headers=None,
            query=query,
        )
        assert resp.status == 204
        # TODO, search then verify
