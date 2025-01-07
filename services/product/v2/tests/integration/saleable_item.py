from typing import List, Tuple, Dict, Optional

from blacksheep import Response
from blacksheep.contents import JSONContent
from pydantic import NonNegativeInt
import pytest

from product.api.dto import (
    TagCreateReqDto,
    AttrDataTypeDto,
    SaleItemCreateReqDto,
    SaleItemAttriReqDto,
)

from .common import (
    ITestClient,
    add_auth_header,
    create_one_tag,
    create_many_attri_labels,
)


class TestSaleableItem:
    @staticmethod
    async def create_tag_bulk(
        client: ITestClient, reqdata: List[Tuple[str, Optional[str]]]
    ) -> List[str]:
        async def setup_one_tag(label: str, parent: Optional[str]) -> str:
            reqbody = TagCreateReqDto(name=label, parent=parent)
            respbody = await create_one_tag(client, reqbody, 201)
            return respbody["node"]

        return [await setup_one_tag(label, parent) for label, parent in reqdata]

    @staticmethod
    async def setup_create_one(
        client: ITestClient,
        reqbody: SaleItemCreateReqDto,
        expect_status: int,
    ) -> Response:
        headers: Dict[str, str] = {}
        add_auth_header(client, headers)
        resp = await client.post(
            path="/item",
            headers=headers,
            content=JSONContent(reqbody),
            cookies=None,
        )
        assert resp.status == expect_status
        return await resp.json()

    @staticmethod
    async def setup_update_one(
        client: ITestClient,
        existing_saleitem_id: int,
        reqbody: SaleItemCreateReqDto,
        expect_status: int,
    ) -> Response:
        headers: Dict[str, str] = {}
        add_auth_header(client, headers)
        resp = await client.put(
            path="/item/%d" % existing_saleitem_id,
            headers=headers,
            content=JSONContent(reqbody),
            cookies=None,
        )
        assert resp.status == expect_status
        return await resp.json()

    @pytest.mark.asyncio(loop_scope="session")
    async def test_create_update_ok(self, mock_client):
        cls = type(self)
        total_tags = []
        tags = await cls.create_tag_bulk(mock_client, [("consumer electronics", None)])
        total_tags.extend(tags)

        chosen_tag = tags[0]["id_"]
        reqdata = [("watches", chosen_tag), ("smartphones", chosen_tag)]
        tags = await cls.create_tag_bulk(mock_client, reqdata)
        total_tags.extend(tags)

        chosen_tag = next(filter(lambda t: t["name"] == "smartphones", tags))
        reqdata = [
            ("Apple", chosen_tag["id_"]),
            ("Samsung", chosen_tag["id_"]),
            ("Google", chosen_tag["id_"]),
        ]
        tags = await cls.create_tag_bulk(mock_client, reqdata)
        total_tags.extend(tags)

        mockdata = [
            ("back color", AttrDataTypeDto.String),
            ("Screen width", AttrDataTypeDto.UnsignedInteger),
            ("screen height", AttrDataTypeDto.UnsignedInteger),
            ("wrap case weight", AttrDataTypeDto.Integer),
            ("Battery Capacity", AttrDataTypeDto.UnsignedInteger),
            ("flammable", AttrDataTypeDto.Boolean),
            ("CPU vendor", AttrDataTypeDto.String),
            ("Supports 5G", AttrDataTypeDto.Boolean),
        ]
        resp = await create_many_attri_labels(mock_client, mockdata, 201)
        total_attr_lablels: List[Dict] = await resp.json()

        def setup_tag_vals(data: List[Dict]) -> List[str]:
            iter0 = filter(lambda t: t["name"] == "smartphones", data)
            iter1 = map(lambda t: t["id_"], iter0)
            return list(iter1)

        def setup_attr_vals(data: List[Dict]) -> List[SaleItemAttriReqDto]:
            attrvals = []
            for a in data:
                if a["name"] == "Battery Capacity":
                    value = NonNegativeInt(479)
                elif a["name"] == "flammable":
                    value = False
                else:
                    continue
                attrval = SaleItemAttriReqDto(id_=a["id_"], value=value)
                attrvals.append(attrval)
            return attrvals

        reqdata = SaleItemCreateReqDto(
            name="Bluetooth Headphones",
            visible=True,
            tags=setup_tag_vals(total_tags),
            attributes=setup_attr_vals(total_attr_lablels),
            media_set=["resource-video-id-999", "resource-image-id-888"],
        )
        respdata = await cls.setup_create_one(mock_client, reqdata, 201)
        assert respdata["usrprof"] == 1234  # TODO, parameter
        assert respdata["name"] == "Bluetooth Headphones"
        assert respdata["visible"] is True
        assert "resource-video-id-999" in respdata["media_set"]
        assert "resource-image-id-888" in respdata["media_set"]
        assert any(filter(lambda t: t["name"] == "smartphones", respdata["tags"]))
        assert any(
            filter(lambda t: t["name"] == "consumer electronics", respdata["tags"])
        )
        assert len(respdata["attributes"]) > 0

        existing_saleitem_id = respdata["id_"]

        def setup_attr_vals(data: List[Dict]) -> List[SaleItemAttriReqDto]:
            attrvals = []
            for a in data:
                if a["name"] == "CPU vendor":
                    value = "RISC-V"
                elif a["name"] == "wrap case weight":
                    value = 76
                elif a["name"] == "Battery Capacity":
                    value = NonNegativeInt(481)
                else:
                    continue
                attrval = SaleItemAttriReqDto(id_=a["id_"], value=value)
                attrvals.append(attrval)
            return attrvals

        reqdata2 = SaleItemCreateReqDto(
            name="LoRa brain wave remote controller",
            visible=False,
            tags=setup_tag_vals(total_tags),
            attributes=setup_attr_vals(total_attr_lablels),
            media_set=["resource-video-id-9487", "resource-image-id-888"],
        )
        respdata2 = await cls.setup_update_one(
            mock_client, existing_saleitem_id, reqdata2, 200
        )
        assert respdata2["usrprof"] == 1234  # TODO, parameter
        assert respdata2["name"] == "LoRa brain wave remote controller"
        assert respdata2["visible"] is False
        assert "resource-video-id-9487" in respdata2["media_set"]
        assert "resource-image-id-888" in respdata2["media_set"]
        expect = [
            ("CPU vendor", "RISC-V"),
            ("wrap case weight", 76),
            ("Battery Capacity", 481),
        ]
        actual = [(a["label"]["name"], a["value"]) for a in respdata2["attributes"]]
        assert set(expect) == set(actual)

    @pytest.mark.asyncio(loop_scope="session")
    async def test_delete_fetch_ok(self, mock_client):
        cls = type(self)
        total_tags = await cls.create_tag_bulk(mock_client, [("home appliances", None)])
        chosen_tag = total_tags[0]["id_"]
        mockdata = [
            ("motor type", AttrDataTypeDto.String),
            ("capacity liter", AttrDataTypeDto.UnsignedInteger),
        ]
        resp = await create_many_attri_labels(mock_client, mockdata, 201)
        total_attr_lablels: List[Dict] = await resp.json()

        def setup_attr_vals(data: List[Dict]) -> List[SaleItemAttriReqDto]:
            attrvals = []
            for a in data:
                if a["name"] == "motor type":
                    value = "belt drive"
                elif a["name"] == "capacity liter":
                    value = 4
                else:
                    continue
                attrval = SaleItemAttriReqDto(id_=a["id_"], value=value)
                attrvals.append(attrval)
            return attrvals

        reqdata = SaleItemCreateReqDto(
            name="Smart Washing Machine",
            visible=True,
            tags=[chosen_tag],
            attributes=setup_attr_vals(total_attr_lablels),
            media_set=["resource-id-video", "resource-id-image"],
        )
        respdata = await cls.setup_create_one(mock_client, reqdata, 201)
        created_item_id = respdata["id_"]

        fetch_resp = await mock_client.get(f"/item/{created_item_id}")
        assert fetch_resp.status == 200
        fetched_data = await fetch_resp.json()
        assert fetched_data["id_"] == created_item_id
        assert fetched_data["name"] == "Smart Washing Machine"
        assert "resource-id-video" in fetched_data["media_set"]
        assert "resource-id-image" in fetched_data["media_set"]

        headers: Dict[str, str] = {}
        add_auth_header(mock_client, headers)
        delete_resp = await mock_client.delete(
            f"/item/{created_item_id}", headers=headers
        )
        assert delete_resp.status == 204

        fetch_deleted_resp = await mock_client.get(f"/item/{created_item_id}")
        assert fetch_deleted_resp.status == 404

    @pytest.mark.asyncio(loop_scope="session")
    async def test_create_invalid_tag(self, mock_client):
        cls = type(self)
        reqdata = SaleItemCreateReqDto(
            name="illegal drug",
            visible=False,
            tags=["nonexist-9876"],
            attributes=[],
            media_set=["resource-video-id-9487", "resource-image-id-888"],
        )
        respdata = await cls.setup_create_one(mock_client, reqdata, 400)
        assert "nonexist-9876" in respdata["tag_nonexist"]

    @pytest.mark.asyncio(loop_scope="session")
    async def test_create_invalid_attribute(self, mock_client):
        cls = type(self)
        total_tags = await cls.create_tag_bulk(mock_client, [("healthcare", None)])
        chosen_tag = total_tags[0]["id_"]
        reqdata = SaleItemCreateReqDto(
            name="no-magic mushr0om",
            visible=True,
            tags=[chosen_tag],
            attributes=[SaleItemAttriReqDto(id_="nonexist567", value="illusion")],
            media_set=["resource-video-id-9487", "resource-image-id-888"],
        )
        respdata = await cls.setup_create_one(mock_client, reqdata, 400)
        assert "nonexist567" in respdata["nonexist-attribute-labels"]
