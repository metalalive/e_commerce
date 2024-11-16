from typing import Optional, Dict
from blacksheep.contents import JSONContent
from blacksheep.testing import TestClient
import pytest

from product.api.dto import TagCreateReqDto, TagUpdateReqDto


async def read_one_tag(
    client: TestClient,
    node_id: int,
    acs: Optional[int],
    desc_lvl: Optional[int],
    expect_status: int,
) -> Dict:
    query = {}
    if acs:
        query["acs"] = acs
    if desc_lvl:
        query["desc_lvl"] = desc_lvl
    resp = await client.get(
        path="/tag/%s" % (node_id),
        headers=None,
        query=query,
        cookies=None,
    )
    assert resp.status == expect_status
    respbody = await resp.json()
    assert respbody["curr_node"]["id_"] == node_id
    return respbody


class TestCreateTag:
    async def create_one(
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

    async def collect_node_id(
        client: TestClient, name: str, parent_id: Optional[int]
    ) -> Optional[int]:
        reqbody = TagCreateReqDto(name=name, parent=parent_id)
        respbody = await TestCreateTag.create_one(client, reqbody, 201)
        return respbody["node"]["id_"]

    @pytest.mark.asyncio
    async def test_one_ok(self, mock_client):
        cls = type(self)
        reqbody = TagCreateReqDto(name="footwear", parent=None)
        respbody = await cls.create_one(mock_client, reqbody, 201)
        assert respbody["node"].get("id_")

    @pytest.mark.asyncio
    async def test_multi_nodes_ok(self, mock_client):
        cls = type(self)
        rootnode_id = await cls.collect_node_id(
            mock_client, name="home building tool", parent_id=None
        )
        data = ["saw", "hammer"]
        layer1_ids = [
            await cls.collect_node_id(mock_client, nm, rootnode_id) for nm in data
        ]
        data = ["circular saw", "chainsaw", "jigsaw"]
        layer2_0_ids = [  # noqa: F841
            await cls.collect_node_id(mock_client, nm, layer1_ids[0]) for nm in data
        ]
        data = ["claw hammer", "sledge hammer", "tack hammer"]
        layer2_1_ids = [
            await cls.collect_node_id(mock_client, nm, layer1_ids[1]) for nm in data
        ]

        rootnode_id = await cls.collect_node_id(
            mock_client, "electronic project kit", None
        )
        data = ["multimeter", "electronic component", "sensor"]
        layer1_ids = [
            await cls.collect_node_id(mock_client, nm, rootnode_id) for nm in data
        ]
        data = ["transistor", "resistor", "capacitor", "diode"]
        layer2_1_ids = [
            await cls.collect_node_id(mock_client, nm, layer1_ids[1]) for nm in data
        ]
        data = ["ESP32", "NPK soil tester"]
        layer2_1_ids = [  # noqa: F841
            await cls.collect_node_id(mock_client, nm, layer1_ids[2]) for nm in data
        ]

        rd_tag = await read_one_tag(
            mock_client, layer1_ids[2], acs=1, desc_lvl=1, expect_status=200
        )
        assert rd_tag["curr_node"]["name"] is not None
        assert rd_tag["ancestors"] is not None
        assert rd_tag["descendants"] is not None
        # TODO, read verification


class TestUpdateTag:
    async def update_one(
        client: TestClient, node_id: int, body: TagUpdateReqDto
    ) -> Dict:
        expect_label = body.name
        expect_parent = body.parent
        resp = await client.patch(
            path="/tag/%s" % (node_id),
            headers=None,
            query=None,
            content=JSONContent(body),
            cookies=None,
        )
        assert resp.status == 200
        respbody = await resp.json()
        assert respbody["node"]["id_"] == node_id
        assert respbody["node"]["name"] == expect_label
        assert respbody.get("parent", None) == expect_parent
        return respbody

    @pytest.mark.asyncio
    async def test_ok(self, mock_client):
        rootnode_id = await TestCreateTag.collect_node_id(
            mock_client, name="household", parent_id=None
        )
        data = ["misc", "mop"]
        layer1_ids = [
            await TestCreateTag.collect_node_id(mock_client, nm, rootnode_id)
            for nm in data
        ]
        data = ["toilet paper", "towel"]
        layer2_0_ids = [
            await TestCreateTag.collect_node_id(mock_client, nm, layer1_ids[0])
            for nm in data
        ]
        data = ["sponge mop", "string mop"]
        layer2_1_ids = [  # noqa: F841
            await TestCreateTag.collect_node_id(mock_client, nm, layer1_ids[1])
            for nm in data
        ]
        cls = type(self)
        reqbody = TagUpdateReqDto(name="toilet paper", parent=rootnode_id)
        respbody = await cls.update_one(  # noqa: F841
            mock_client, node_id=layer2_0_ids[0], body=reqbody
        )


class TestDeleteTag:
    async def delete_one(client: TestClient, node_id: int, expect_status: int):
        resp = await client.delete(
            path="/tag/%s" % (node_id),
            headers=None,
            query=None,
            content=None,
            cookies=None,
        )
        assert resp.status == expect_status

    @pytest.mark.asyncio
    async def test_ok(self, mock_client):
        rootnode_id = await TestCreateTag.collect_node_id(
            mock_client, name="household II", parent_id=None
        )
        data = ["bucket", "shovel"]
        layer1_ids = [  # noqa: F841
            await TestCreateTag.collect_node_id(mock_client, nm, rootnode_id)
            for nm in data
        ]
        cls = type(self)
        await cls.delete_one(mock_client, rootnode_id, 204)
