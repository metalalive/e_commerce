from dataclasses import asdict
from typing import Tuple, Optional

import pytest
from product.model import TagModel, TagTreeModel, TagErrorModel, TagErrorReason
from product.api.dto import TagCreateReqDto


class TestCreate:
    def test_dto_convert_ok(self):
        mock_parent_id = "2-9246"
        mock_req = TagCreateReqDto(name="tag123", parent=mock_parent_id)
        tag_m = TagModel.from_req(mock_req)
        fieldmap = asdict(tag_m)
        assert fieldmap["_label"] == "tag123"
        assert fieldmap["_id"] == 0
        mock_resp = tag_m.to_resp(2, 9246)
        assert mock_resp.node.name == "tag123"
        assert mock_resp.node.id_ == "2-0"
        assert mock_resp.parent == mock_parent_id

    def test_insert_unknown_tree(self):
        mock_req = TagCreateReqDto(name="tag124", parent=None)
        tag_m0 = TagModel.from_req(mock_req)
        mock_req = TagCreateReqDto(name="tag125", parent=None)
        tag_m1 = TagModel.from_req(mock_req)
        mock_tree = TagTreeModel(_id=1, nodes=[tag_m0])
        with pytest.raises(TagErrorModel) as e:
            mock_tree.try_insert(tag_m1, parent_node_id=None)
        assert e.value.reason == TagErrorReason.UnknownTree

    def test_insert_missing_tree(self):
        mock_parent_id = 9246
        mock_req = TagCreateReqDto(name="tag124", parent=None)
        tag_m = TagModel.from_req(mock_req)
        mock_tree = TagTreeModel(_id=1)
        with pytest.raises(TagErrorModel) as e:
            mock_tree.try_insert(tag_m, parent_node_id=mock_parent_id)
        assert e.value.reason == TagErrorReason.MissingTree

    def test_insert_missing_parent(self):
        mock_parent_id = pow(2, 32) - 1
        mock_req = TagCreateReqDto(name="tag124", parent=None)
        tag_m0 = TagModel.from_req(mock_req)
        mock_req = TagCreateReqDto(name="tag125", parent=None)
        tag_m1 = TagModel.from_req(mock_req)
        mock_tree = TagTreeModel(_id=1, nodes=[tag_m0])
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
        mock_tree = TagTreeModel(_id=99)
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
        mock_tree = TagTreeModel(_id=99)
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
        mock_tree = TagTreeModel(_id=99)
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
        mock_tree = TagTreeModel(_id=99)
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
        # import pdb
        # pdb.set_trace()

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
