from typing import Optional, Dict
from blacksheep.contents import JSONContent
from blacksheep.testing import TestClient
import pytest

from product.api.dto import TagCreateReqDto, TagUpdateReqDto
from .common import create_one_tag


async def read_one_tag(
    client: TestClient,
    tag_id: str,
    acs: Optional[int] = None,
    desc_lvl: Optional[int] = None,
    expect_status: int = -1,
) -> Dict:
    query = {}
    if acs:
        query["acs_req"] = acs
    if desc_lvl:
        query["desc_lvl"] = desc_lvl
    resp = await client.get(
        path="/tag/%s" % (tag_id),
        headers=None,
        query=query,
        cookies=None,
    )
    assert resp.status == expect_status
    respbody = await resp.json()
    assert respbody["curr_node"]["id_"] == tag_id
    return respbody


class TestCreateTag:
    @classmethod
    async def collect_node_id(
        cls, client: TestClient, name: str, parent_id: Optional[int]
    ) -> Optional[int]:
        reqbody = TagCreateReqDto(name=name, parent=parent_id)
        respbody = await create_one_tag(client, reqbody, 201)
        return respbody["node"]["id_"]

    @pytest.mark.asyncio(loop_scope="session")
    async def test_one_ok(self, mock_client):
        reqbody = TagCreateReqDto(name="footwear", parent=None)
        respbody = await create_one_tag(mock_client, reqbody, 201)
        assert respbody["node"].get("id_")

    @pytest.mark.asyncio(loop_scope="session")
    async def test_multi_nodes_ok(self, mock_client):
        cls = type(self)
        root_id = await cls.collect_node_id(
            mock_client, name="home building tool", parent_id=None
        )
        data = ["saw", "hammer"]
        layer1_ids = [
            await cls.collect_node_id(mock_client, nm, root_id) for nm in data
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
        assert rd_tag["curr_node"]["name"] == "sensor"
        assert len(rd_tag["ancestors"]) == 1
        assert len(rd_tag["descendants"]) == 2
        assert rd_tag["ancestors"][0]["name"] == "electronic project kit"
        actual_child_labels = [d["name"] for d in rd_tag["descendants"]]
        assert set(actual_child_labels) == set(["ESP32", "NPK soil tester"])


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
        assert respbody["node"]["name"] == expect_label
        assert respbody.get("parent", None) == expect_parent
        return respbody

    @pytest.mark.asyncio(loop_scope="session")
    async def test_same_tree(self, mock_client):
        fn_add_tag = TestCreateTag.collect_node_id
        root_id = await fn_add_tag(mock_client, name="household", parent_id=None)
        data = ["misc", "mop"]
        layer1_ids = [await fn_add_tag(mock_client, nm, root_id) for nm in data]
        data = ["toilet paper", "towel"]
        layer2_0_ids = [await fn_add_tag(mock_client, nm, layer1_ids[0]) for nm in data]
        data = ["sponge mop", "string mop"]
        layer2_1_ids = [  # noqa: F841
            await fn_add_tag(mock_client, nm, layer1_ids[1]) for nm in data
        ]
        cls = type(self)
        reqbody = TagUpdateReqDto(name="toilet paper", parent=root_id)
        respbody = await cls.update_one(
            mock_client, node_id=layer2_0_ids[0], body=reqbody
        )
        assert respbody["parent"] == root_id
        assert respbody["node"]["id_"] != layer2_0_ids[0]
        assert respbody["node"]["name"] == "toilet paper"
        layer2_0_ids[0] = respbody["node"]["id_"]
        respbody = await read_one_tag(
            mock_client,
            tag_id=root_id,
            acs=None,
            desc_lvl=1,
            expect_status=200,
        )
        expect_labels = ["misc", "mop", "toilet paper"]
        actual_labels = [d["name"] for d in respbody["descendants"]]
        assert set(expect_labels) == set(actual_labels)
        respbody = await read_one_tag(
            mock_client,
            tag_id=layer1_ids[0],
            desc_lvl=1,
            expect_status=200,
        )
        expect_labels = ["towel"]
        actual_labels = [d["name"] for d in respbody["descendants"]]
        assert set(expect_labels) == set(actual_labels)

    @pytest.mark.asyncio(loop_scope="session")
    async def test_move_to_new_tree(self, mock_client):
        fn_add_tag = TestCreateTag.collect_node_id
        root_id = await fn_add_tag(mock_client, name="cooking", parent_id=None)
        data = ["stove", "oven", "blender"]
        layer1_ids = [await fn_add_tag(mock_client, nm, root_id) for nm in data]
        cls = type(self)
        reqbody = TagUpdateReqDto(name="stove", parent=None)
        respbody = await cls.update_one(
            mock_client, node_id=layer1_ids[0], body=reqbody
        )
        assert respbody["node"]["id_"] != layer1_ids[0]
        assert respbody["node"]["name"] == "stove"
        layer1_ids[0] = respbody["node"]["id_"]
        respbody = await read_one_tag(
            mock_client,
            tag_id=root_id,
            desc_lvl=5,
            expect_status=200,
        )
        expect_labels = ["blender", "oven"]
        actual_labels = [d["name"] for d in respbody["descendants"]]
        assert set(expect_labels) == set(actual_labels)
        respbody = await read_one_tag(
            mock_client,
            tag_id=layer1_ids[0],
            desc_lvl=1,
            expect_status=200,
        )
        assert respbody["curr_node"]["name"] == "stove"
        assert len(respbody["descendants"]) == 0

    @pytest.mark.asyncio(loop_scope="session")
    async def test_different_tree(self, mock_client):
        fn_add_tag = TestCreateTag.collect_node_id
        root1_id = await fn_add_tag(mock_client, name="0r9anHarve5t", parent_id=None)
        root2_id = await fn_add_tag(mock_client, name="Da1aiLLama", parent_id=None)
        data = ["kidney", "liver", "lung"]
        t1L1_ids = [await fn_add_tag(mock_client, nm, root1_id) for nm in data]
        data = ["mindful", "calm"]
        for nm in data:
            await fn_add_tag(mock_client, nm, root2_id)
        cls = type(self)
        reqbody = TagUpdateReqDto(name="liver", parent=root2_id)
        respbody = await cls.update_one(mock_client, node_id=t1L1_ids[1], body=reqbody)
        assert respbody["node"]["id_"] != t1L1_ids[1]
        assert respbody["node"]["name"] == "liver"
        t1L1_ids[1] = respbody["node"]["id_"]
        respbody = await read_one_tag(
            mock_client, tag_id=root1_id, desc_lvl=1, expect_status=200
        )
        expect_labels = ["kidney", "lung"]
        actual_labels = [d["name"] for d in respbody["descendants"]]
        assert set(expect_labels) == set(actual_labels)
        respbody = await read_one_tag(
            mock_client, tag_id=root2_id, desc_lvl=1, expect_status=200
        )
        expect_labels = ["mindful", "calm", "liver"]
        actual_labels = [d["name"] for d in respbody["descendants"]]
        assert set(expect_labels) == set(actual_labels)


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

    @pytest.mark.asyncio(loop_scope="session")
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
