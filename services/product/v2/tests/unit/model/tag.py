from dataclasses import asdict
from typing import Tuple, Optional

import pytest
from product.model import TagModel, TagTreeModel, TagErrorModel, TagErrorReason
from product.api.dto import TagCreateReqDto


class TestCreate:
    def test_decode_req_id_ok(self):
        mock_parent_id = "taigg-2330"
        (tree_id, node_id) = TagModel.decode_req_id(mock_parent_id)
        assert tree_id == "taigg"
        assert node_id == 2330

    def test_dto_convert_ok(self):
        tree_id = "h2"
        parent_node_id = 9246
        tag_parent_id = "%s-%d" % (tree_id, parent_node_id)
        mock_req = TagCreateReqDto(name="bloom123", parent=tag_parent_id)
        tag_m = TagModel.from_req(mock_req)
        fieldmap = asdict(tag_m)
        assert fieldmap["_label"] == "bloom123"
        assert fieldmap["_id"] == 0
        mock_resp = tag_m.to_resp(tree_id, parent_node_id)
        assert mock_resp.node.name == "bloom123"
        assert mock_resp.node.id_ == "h2-0"
        assert mock_resp.parent == tag_parent_id
        mock_node_dto = tag_m.to_node_dto(tree_id)
        assert mock_node_dto.id_ == "h2-0"
        assert mock_node_dto.name == "bloom123"

    def test_insert_unknown_tree(self):
        mock_req = TagCreateReqDto(name="tag124", parent=None)
        tag_m0 = TagModel.from_req(mock_req)
        mock_req = TagCreateReqDto(name="tag125", parent=None)
        tag_m1 = TagModel.from_req(mock_req)
        mock_tree = TagTreeModel(_id="s5o0", nodes=[tag_m0])
        with pytest.raises(TagErrorModel) as e:
            mock_tree.try_insert(tag_m1, parent_node_id=None)
        assert e.value.reason == TagErrorReason.UnknownTree

    def test_insert_missing_tree(self):
        mock_parent_id = 9246
        mock_req = TagCreateReqDto(name="tag124", parent=None)
        tag_m = TagModel.from_req(mock_req)
        mock_tree = TagTreeModel(_id="s5o0")
        with pytest.raises(TagErrorModel) as e:
            mock_tree.try_insert(tag_m, parent_node_id=mock_parent_id)
        assert e.value.reason == TagErrorReason.MissingTree

    def test_insert_missing_parent(self):
        mock_parent_id = pow(2, 32) - 1
        mock_req = TagCreateReqDto(name="tag124", parent=None)
        tag_m0 = TagModel.from_req(mock_req)
        mock_req = TagCreateReqDto(name="tag125", parent=None)
        tag_m1 = TagModel.from_req(mock_req)
        mock_tree = TagTreeModel(_id="s5o0", nodes=[tag_m0])
        with pytest.raises(TagErrorModel) as e:
            mock_tree.try_insert(tag_m1, parent_node_id=mock_parent_id)
        assert e.value.reason == TagErrorReason.MissingParent

    @staticmethod
    def setup_new_node(name):
        mock_req = TagCreateReqDto(name=name, parent=None)
        return TagModel.from_req(mock_req)

    @staticmethod
    def verify_node_ends(node: TagModel, expect=Tuple[int, int]):
        actual = (node._limit_left, node._limit_right)
        assert actual == expect

    def test_insert_nodes_ok_1(self):
        cls = type(self)
        mock_tree = TagTreeModel(_id="s5g9q")
        mock_tags = list(
            map(
                cls.setup_new_node,
                ["t1", "t2", "t3", "t4", "t5", "t6", "t7", "t8", "t9", "t10", "t11"],
            )
        )
        cls.insert_then_verify(mock_tree, parent_node=None, new_node=mock_tags[0])
        cls.verify_node_ends(mock_tags[0], expect=(1, 2))
        cls.insert_then_verify(
            mock_tree, parent_node=mock_tags[0], new_node=mock_tags[1]
        )
        cls.insert_then_verify(mock_tree, mock_tags[0], new_node=mock_tags[2])
        cls.verify_node_ends(mock_tags[0], expect=(1, 6))
        cls.verify_node_ends(mock_tags[2], expect=(4, 5))
        cls.insert_then_verify(mock_tree, mock_tags[1], new_node=mock_tags[3])
        cls.verify_node_ends(mock_tags[1], expect=(2, 5))
        cls.verify_node_ends(mock_tags[2], expect=(6, 7))
        cls.insert_then_verify(mock_tree, mock_tags[2], new_node=mock_tags[4])
        cls.verify_node_ends(mock_tags[1], expect=(2, 5))
        cls.verify_node_ends(mock_tags[2], expect=(6, 9))
        cls.insert_then_verify(mock_tree, mock_tags[1], new_node=mock_tags[5])
        cls.insert_then_verify(mock_tree, mock_tags[2], new_node=mock_tags[6])
        cls.verify_node_ends(mock_tags[1], expect=(2, 7))
        cls.verify_node_ends(mock_tags[2], expect=(8, 13))
        cls.insert_then_verify(mock_tree, mock_tags[0], new_node=mock_tags[7])
        cls.verify_node_ends(mock_tags[7], expect=(14, 15))
        cls.insert_then_verify(mock_tree, mock_tags[2], new_node=mock_tags[8])
        cls.verify_node_ends(mock_tags[1], expect=(2, 7))
        cls.verify_node_ends(mock_tags[2], expect=(8, 15))
        cls.verify_node_ends(mock_tags[7], expect=(16, 17))
        cls.insert_then_verify(mock_tree, mock_tags[8], new_node=mock_tags[9])
        cls.verify_node_ends(mock_tags[0], expect=(1, 20))
        cls.verify_node_ends(mock_tags[1], expect=(2, 7))
        cls.verify_node_ends(mock_tags[2], expect=(8, 17))
        cls.verify_node_ends(mock_tags[7], expect=(18, 19))
        cls.verify_node_ends(mock_tags[3], expect=(3, 4))
        cls.verify_node_ends(mock_tags[4], expect=(9, 10))
        cls.verify_node_ends(mock_tags[5], expect=(5, 6))
        cls.verify_node_ends(mock_tags[6], expect=(11, 12))
        cls.verify_node_ends(mock_tags[8], expect=(13, 16))
        cls.verify_node_ends(mock_tags[9], expect=(14, 15))
        cls.insert_then_verify(mock_tree, mock_tags[6], new_node=mock_tags[10])
        assert len(mock_tree.nodes) == 11
        cls.verify_node_ends(mock_tags[1], expect=(2, 7))
        cls.verify_node_ends(mock_tags[2], expect=(8, 19))
        cls.verify_node_ends(mock_tags[7], expect=(20, 21))
        cls.verify_node_ends(mock_tags[6], expect=(11, 14))
        cls.verify_node_ends(mock_tags[8], expect=(15, 18))
        cls.verify_node_ends(mock_tags[9], expect=(16, 17))
        cls.verify_node_ends(mock_tags[10], expect=(12, 13))

    def test_insert_nodes_ok_2(self):
        cls = type(self)
        mock_tree = TagTreeModel(_id="99")
        mock_tags = list(
            map(cls.setup_new_node, ["t1", "t2", "t3", "t4", "t5", "t6", "t7"])
        )
        cls.insert_then_verify(mock_tree, parent_node=None, new_node=mock_tags[0])
        for i in range(6):
            cls.insert_then_verify(mock_tree, mock_tags[i], new_node=mock_tags[i + 1])
        assert len(mock_tree.nodes) == 7
        cls.verify_node_ends(mock_tags[0], expect=(1, 14))
        cls.verify_node_ends(mock_tags[1], expect=(2, 13))
        cls.verify_node_ends(mock_tags[3], expect=(4, 11))
        cls.verify_node_ends(mock_tags[5], expect=(6, 9))

    def test_insert_nodes_ok_3(self):
        cls = type(self)
        mock_tree = TagTreeModel(_id="1yu6")
        mock_tags = list(
            map(cls.setup_new_node, ["t1", "t2", "t3", "t4", "t5", "t6", "t7", "t8"])
        )
        cls.insert_then_verify(mock_tree, parent_node=None, new_node=mock_tags[0])
        cls.insert_then_verify(mock_tree, mock_tags[0], new_node=mock_tags[1])
        cls.insert_then_verify(mock_tree, mock_tags[1], new_node=mock_tags[2])
        cls.insert_then_verify(mock_tree, mock_tags[2], new_node=mock_tags[3])
        cls.insert_then_verify(mock_tree, mock_tags[2], new_node=mock_tags[4])
        cls.insert_then_verify(mock_tree, mock_tags[4], new_node=mock_tags[5])
        cls.insert_then_verify(mock_tree, mock_tags[5], new_node=mock_tags[6])
        cls.insert_then_verify(mock_tree, mock_tags[6], new_node=mock_tags[7])
        assert len(mock_tree.nodes) == 8
        cls.verify_node_ends(mock_tags[0], expect=(1, 16))
        cls.verify_node_ends(mock_tags[2], expect=(3, 14))
        cls.verify_node_ends(mock_tags[3], expect=(4, 5))
        cls.verify_node_ends(mock_tags[4], expect=(6, 13))
        cls.verify_node_ends(mock_tags[7], expect=(9, 10))

    def test_insert_nodes_ok_4(self):
        cls = type(self)
        mock_tree = TagTreeModel(_id="av5y")
        # fmt: off
        tag_labels = ["t1", "t2", "t3", "t4", "t5", "t6", "t7", "t8", "t9", "t10", "t11"]
        # fmt: on
        mock_tags = list(map(cls.setup_new_node, tag_labels))
        cls.insert_then_verify(mock_tree, parent_node=None, new_node=mock_tags[0])
        cls.insert_then_verify(mock_tree, mock_tags[0], new_node=mock_tags[1])
        cls.insert_then_verify(mock_tree, mock_tags[0], new_node=mock_tags[2])
        cls.insert_then_verify(mock_tree, mock_tags[1], new_node=mock_tags[3])
        cls.insert_then_verify(mock_tree, mock_tags[3], new_node=mock_tags[4])
        cls.insert_then_verify(mock_tree, mock_tags[4], new_node=mock_tags[5])
        cls.insert_then_verify(mock_tree, mock_tags[5], new_node=mock_tags[6])
        cls.verify_node_ends(mock_tags[1], expect=(2, 11))
        cls.verify_node_ends(mock_tags[2], expect=(12, 13))
        cls.verify_node_ends(mock_tags[3], expect=(3, 10))
        cls.verify_node_ends(mock_tags[6], expect=(6, 7))
        cls.insert_then_verify(mock_tree, mock_tags[6], new_node=mock_tags[7])
        cls.verify_node_ends(mock_tags[6], expect=(6, 9))
        cls.verify_node_ends(mock_tags[7], expect=(7, 8))
        cls.insert_then_verify(mock_tree, mock_tags[4], new_node=mock_tags[8])
        cls.insert_then_verify(mock_tree, mock_tags[5], new_node=mock_tags[9])
        cls.insert_then_verify(mock_tree, mock_tags[6], new_node=mock_tags[10])
        cls.verify_node_ends(mock_tags[0], expect=(1, 22))
        cls.verify_node_ends(mock_tags[1], expect=(2, 19))
        cls.verify_node_ends(mock_tags[2], expect=(20, 21))
        cls.verify_node_ends(mock_tags[3], expect=(3, 18))
        cls.verify_node_ends(mock_tags[4], expect=(4, 17))
        cls.verify_node_ends(mock_tags[5], expect=(5, 14))
        cls.verify_node_ends(mock_tags[6], expect=(6, 11))
        cls.verify_node_ends(mock_tags[7], expect=(7, 8))

    @staticmethod
    def insert_then_verify(
        tree: TagTreeModel, parent_node: Optional[TagModel], new_node: TagModel
    ):
        parent_node_id = parent_node._id if parent_node else None
        tree.try_insert(new_node, parent_node_id)

        # Ensure the new node exists in the tree
        assert new_node._id > 0
        inserted_node = next(
            (node for node in tree.nodes if node._id == new_node._id), None
        )
        assert inserted_node is not None, f"New node with ID {new_node._id} not found."
        assert new_node._label == inserted_node._label

        for node in tree.nodes:  # each node should still be valid
            assert (
                node._limit_left < node._limit_right
            ), f"Node {node._id} has invalid limits."

        # Verify overlapping
        for i, node in enumerate(tree.nodes):
            # Ensure no overlapping among siblings
            for j in range(i + 1, len(tree.nodes)):
                other_node = tree.nodes[j]
                # Check for proper nested structure or disjointed ranges
                left_overlap = (
                    (node._limit_left >= other_node._limit_left)
                    and (node._limit_left <= other_node._limit_right)
                    and (node._limit_right >= other_node._limit_right)
                )
                assert (
                    not left_overlap
                ), f"Node {node._id} has left overlop on another node {other_node._id}"
                right_overlap = (
                    (node._limit_left <= other_node._limit_left)
                    and (node._limit_right >= other_node._limit_left)
                    and (node._limit_right <= other_node._limit_right)
                )
                assert (
                    not right_overlap
                ), f"Node {node._id} has right overlop on another node {other_node._id}"

        if parent_node:
            # Verify parent node boundaries have expanded correctly
            assert (
                parent_node._limit_left
                < inserted_node._limit_left
                < inserted_node._limit_right
                < parent_node._limit_right
            ), "Inserted node's limits are not within the parent's boundaries."
            # Verify sibling nodes and their limits
            for node in tree.nodes:
                if (
                    node._limit_left > parent_node._limit_left
                    and node._limit_right < parent_node._limit_right
                ):
                    # Check if the sibling limits are consistent and do not overlap
                    if node._id != inserted_node._id:
                        assert not (
                            inserted_node._limit_left
                            < node._limit_right
                            < inserted_node._limit_right
                        ), f"Sibling node {node._id} overlaps with the inserted node."

        # Ensure all limits are unique
        limits = [(node._limit_left, node._limit_right) for node in tree.nodes]
        flat_limits = [limit for pair in limits for limit in pair]
        assert len(flat_limits) == len(
            set(flat_limits)
        ), "Duplicate limits found in the tree."

    def test_find_node_from_tree_ok(self):
        cls = type(self)
        mock_tree = TagTreeModel(_id="8964tank")
        tag_labels = ["t1", "t2", "t3", "t4", "t5", "t6", "t7"]
        mock_tags = list(map(cls.setup_new_node, tag_labels))
        cls.insert_then_verify(mock_tree, parent_node=None, new_node=mock_tags[0])
        cls.insert_then_verify(mock_tree, mock_tags[0], new_node=mock_tags[1])
        cls.insert_then_verify(mock_tree, mock_tags[0], new_node=mock_tags[2])
        cls.insert_then_verify(mock_tree, mock_tags[1], new_node=mock_tags[3])
        cls.insert_then_verify(mock_tree, mock_tags[1], new_node=mock_tags[4])
        cls.insert_then_verify(mock_tree, mock_tags[2], new_node=mock_tags[5])
        cls.insert_then_verify(mock_tree, mock_tags[2], new_node=mock_tags[6])
        assert mock_tree.find_node(node_id=1) is mock_tags[0]
        assert mock_tree.find_node(node_id=3) is mock_tags[2]
        assert mock_tree.find_node(node_id=7) is mock_tags[6]
        assert mock_tree.find_node(node_id=9999) is None

    def test_find_ancestors_descendants_ok(self):
        cls = type(self)
        mock_tree = TagTreeModel(_id="winnnieTheFlu")

        def gen_tag_labels(nitems):
            for idx in range(nitems):
                yield "halo%d" % (idx + 1)

        mock_tags = list(map(cls.setup_new_node, gen_tag_labels(35)))
        cls.insert_then_verify(mock_tree, parent_node=None, new_node=mock_tags[0])
        for idx_p in range(15):
            c_left = idx_p * 2 + 1
            c_right = idx_p * 2 + 2
            cls.insert_then_verify(
                mock_tree, mock_tags[idx_p], new_node=mock_tags[c_left]
            )
            cls.insert_then_verify(
                mock_tree, mock_tags[idx_p], new_node=mock_tags[c_right]
            )

        cls.insert_then_verify(mock_tree, mock_tags[26], new_node=mock_tags[31])
        cls.insert_then_verify(mock_tree, mock_tags[28], new_node=mock_tags[32])
        cls.insert_then_verify(mock_tree, mock_tags[29], new_node=mock_tags[33])
        cls.insert_then_verify(mock_tree, mock_tags[30], new_node=mock_tags[34])

        def verify_labels(nodes, expect_labels):
            actual_labels = [a._label for a in nodes]
            assert set(expect_labels) == set(actual_labels)

        ancestors = mock_tree.find_ancestors(mock_tags[0])
        assert len(ancestors) == 0
        ancestors = mock_tree.find_ancestors(mock_tags[11])
        verify_labels(ancestors, ["halo1", "halo3", "halo6"])
        ancestors = mock_tree.find_ancestors(mock_tags[18])
        verify_labels(ancestors, ["halo1", "halo2", "halo4", "halo9"])
        ancestors = mock_tree.find_ancestors(mock_tags[34])
        verify_labels(ancestors, ["halo1", "halo3", "halo7", "halo15", "halo31"])

        descs = mock_tree.find_descendants(mock_tags[0], max_desc_lvl=-1)
        assert len(descs) == 0
        descs = mock_tree.find_descendants(mock_tags[0], max_desc_lvl=1)
        verify_labels(descs, ["halo2", "halo3"])
        descs = mock_tree.find_descendants(mock_tags[29], max_desc_lvl=1)
        verify_labels(descs, ["halo34"])
        descs = mock_tree.find_descendants(mock_tags[29], max_desc_lvl=99)
        verify_labels(descs, ["halo34"])
        descs = mock_tree.find_descendants(mock_tags[3], max_desc_lvl=1)
        verify_labels(descs, ["halo8", "halo9"])
        descs = mock_tree.find_descendants(mock_tags[4], max_desc_lvl=2)
        # fmt: off
        verify_labels(descs, ["halo10", "halo11", "halo20", "halo21", "halo22", "halo23"])
        # fmt: on
        descs = mock_tree.find_descendants(mock_tags[12], max_desc_lvl=2)
        verify_labels(descs, ["halo26", "halo27", "halo32"])
