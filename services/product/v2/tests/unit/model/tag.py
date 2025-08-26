from dataclasses import asdict
from typing import Tuple, List, Optional

import pytest
from product.model import TagModel, TagTreeModel, TagErrorModel, TagErrorReason
from product.api.dto import TagCreateReqDto


def setup_new_node(name: str) -> TagModel:
    mock_req = TagCreateReqDto(name=name, parent=None)
    return TagModel.from_req(mock_req)


def verify_node_ends(node: TagModel, expect=Tuple[int, int]):
    actual = (node._limit_left, node._limit_right)
    assert actual == expect


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

    def test_insert_nodes_ok_1(self):
        cls = type(self)
        mock_tree = TagTreeModel(_id="s5g9q")
        mock_tags = list(
            map(
                setup_new_node,
                ["t1", "t2", "t3", "t4", "t5", "t6", "t7", "t8", "t9", "t10", "t11"],
            )
        )
        cls.insert_then_verify(mock_tree, parent_node=None, new_node=mock_tags[0])
        verify_node_ends(mock_tags[0], expect=(1, 2))
        cls.insert_then_verify(mock_tree, parent_node=mock_tags[0], new_node=mock_tags[1])
        cls.insert_then_verify(mock_tree, mock_tags[0], new_node=mock_tags[2])
        verify_node_ends(mock_tags[0], expect=(1, 6))
        verify_node_ends(mock_tags[2], expect=(4, 5))
        cls.insert_then_verify(mock_tree, mock_tags[1], new_node=mock_tags[3])
        verify_node_ends(mock_tags[1], expect=(2, 5))
        verify_node_ends(mock_tags[2], expect=(6, 7))
        cls.insert_then_verify(mock_tree, mock_tags[2], new_node=mock_tags[4])
        verify_node_ends(mock_tags[1], expect=(2, 5))
        verify_node_ends(mock_tags[2], expect=(6, 9))
        cls.insert_then_verify(mock_tree, mock_tags[1], new_node=mock_tags[5])
        cls.insert_then_verify(mock_tree, mock_tags[2], new_node=mock_tags[6])
        verify_node_ends(mock_tags[1], expect=(2, 7))
        verify_node_ends(mock_tags[2], expect=(8, 13))
        cls.insert_then_verify(mock_tree, mock_tags[0], new_node=mock_tags[7])
        verify_node_ends(mock_tags[7], expect=(14, 15))
        cls.insert_then_verify(mock_tree, mock_tags[2], new_node=mock_tags[8])
        verify_node_ends(mock_tags[1], expect=(2, 7))
        verify_node_ends(mock_tags[2], expect=(8, 15))
        verify_node_ends(mock_tags[7], expect=(16, 17))
        cls.insert_then_verify(mock_tree, mock_tags[8], new_node=mock_tags[9])
        verify_node_ends(mock_tags[0], expect=(1, 20))
        verify_node_ends(mock_tags[1], expect=(2, 7))
        verify_node_ends(mock_tags[2], expect=(8, 17))
        verify_node_ends(mock_tags[7], expect=(18, 19))
        verify_node_ends(mock_tags[3], expect=(3, 4))
        verify_node_ends(mock_tags[4], expect=(9, 10))
        verify_node_ends(mock_tags[5], expect=(5, 6))
        verify_node_ends(mock_tags[6], expect=(11, 12))
        verify_node_ends(mock_tags[8], expect=(13, 16))
        verify_node_ends(mock_tags[9], expect=(14, 15))
        cls.insert_then_verify(mock_tree, mock_tags[6], new_node=mock_tags[10])
        assert len(mock_tree.nodes) == 11
        verify_node_ends(mock_tags[1], expect=(2, 7))
        verify_node_ends(mock_tags[2], expect=(8, 19))
        verify_node_ends(mock_tags[7], expect=(20, 21))
        verify_node_ends(mock_tags[6], expect=(11, 14))
        verify_node_ends(mock_tags[8], expect=(15, 18))
        verify_node_ends(mock_tags[9], expect=(16, 17))
        verify_node_ends(mock_tags[10], expect=(12, 13))

    def test_insert_nodes_degenerate(self):
        cls = type(self)
        mock_tree = TagTreeModel(_id="99")
        mock_tags = list(map(setup_new_node, ["t1", "t2", "t3", "t4", "t5", "t6", "t7"]))
        cls.insert_then_verify(mock_tree, parent_node=None, new_node=mock_tags[0])
        for i in range(6):
            cls.insert_then_verify(mock_tree, mock_tags[i], new_node=mock_tags[i + 1])
        assert len(mock_tree.nodes) == 7
        verify_node_ends(mock_tags[0], expect=(1, 14))
        verify_node_ends(mock_tags[1], expect=(2, 13))
        verify_node_ends(mock_tags[3], expect=(4, 11))
        verify_node_ends(mock_tags[5], expect=(6, 9))

    def test_insert_nodes_ok_2(self):
        cls = type(self)
        mock_tree = TagTreeModel(_id="1yu6")
        mock_tags = list(map(setup_new_node, ["t1", "t2", "t3", "t4", "t5", "t6", "t7", "t8"]))
        cls.insert_then_verify(mock_tree, parent_node=None, new_node=mock_tags[0])
        cls.insert_then_verify(mock_tree, mock_tags[0], new_node=mock_tags[1])
        cls.insert_then_verify(mock_tree, mock_tags[1], new_node=mock_tags[2])
        cls.insert_then_verify(mock_tree, mock_tags[2], new_node=mock_tags[3])
        cls.insert_then_verify(mock_tree, mock_tags[2], new_node=mock_tags[4])
        cls.insert_then_verify(mock_tree, mock_tags[4], new_node=mock_tags[5])
        cls.insert_then_verify(mock_tree, mock_tags[5], new_node=mock_tags[6])
        cls.insert_then_verify(mock_tree, mock_tags[6], new_node=mock_tags[7])
        assert len(mock_tree.nodes) == 8
        verify_node_ends(mock_tags[0], expect=(1, 16))
        verify_node_ends(mock_tags[2], expect=(3, 14))
        verify_node_ends(mock_tags[3], expect=(4, 5))
        verify_node_ends(mock_tags[4], expect=(6, 13))
        verify_node_ends(mock_tags[7], expect=(9, 10))

    def test_insert_nodes_ok_3(self):
        cls = type(self)
        mock_tree = TagTreeModel(_id="av5y")
        # fmt: off
        tag_labels = ["t1", "t2", "t3", "t4", "t5", "t6", "t7", "t8", "t9", "t10", "t11"]
        # fmt: on
        mock_tags = list(map(setup_new_node, tag_labels))
        cls.insert_then_verify(mock_tree, parent_node=None, new_node=mock_tags[0])
        cls.insert_then_verify(mock_tree, mock_tags[0], new_node=mock_tags[1])
        cls.insert_then_verify(mock_tree, mock_tags[0], new_node=mock_tags[2])
        cls.insert_then_verify(mock_tree, mock_tags[1], new_node=mock_tags[3])
        cls.insert_then_verify(mock_tree, mock_tags[3], new_node=mock_tags[4])
        cls.insert_then_verify(mock_tree, mock_tags[4], new_node=mock_tags[5])
        cls.insert_then_verify(mock_tree, mock_tags[5], new_node=mock_tags[6])
        verify_node_ends(mock_tags[1], expect=(2, 11))
        verify_node_ends(mock_tags[2], expect=(12, 13))
        verify_node_ends(mock_tags[3], expect=(3, 10))
        verify_node_ends(mock_tags[6], expect=(6, 7))
        cls.insert_then_verify(mock_tree, mock_tags[6], new_node=mock_tags[7])
        verify_node_ends(mock_tags[6], expect=(6, 9))
        verify_node_ends(mock_tags[7], expect=(7, 8))
        cls.insert_then_verify(mock_tree, mock_tags[4], new_node=mock_tags[8])
        cls.insert_then_verify(mock_tree, mock_tags[5], new_node=mock_tags[9])
        cls.insert_then_verify(mock_tree, mock_tags[6], new_node=mock_tags[10])
        verify_node_ends(mock_tags[0], expect=(1, 22))
        verify_node_ends(mock_tags[1], expect=(2, 19))
        verify_node_ends(mock_tags[2], expect=(20, 21))
        verify_node_ends(mock_tags[3], expect=(3, 18))
        verify_node_ends(mock_tags[4], expect=(4, 17))
        verify_node_ends(mock_tags[5], expect=(5, 14))
        verify_node_ends(mock_tags[6], expect=(6, 11))
        verify_node_ends(mock_tags[7], expect=(7, 8))

    @staticmethod
    def verify_nested_set_property(tree: TagTreeModel):
        for node in tree.nodes:  # each node should still be valid
            assert node._limit_left < node._limit_right, f"Node {node._id} has invalid limits."

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

        # Ensure all limits are unique
        limits = [(node._limit_left, node._limit_right) for node in tree.nodes]
        flat_limits = [limit for pair in limits for limit in pair]
        assert len(flat_limits) == len(set(flat_limits)), "Duplicate limits found in the tree."

    @classmethod
    def insert_then_verify(
        cls, tree: TagTreeModel, parent_node: Optional[TagModel], new_node: TagModel
    ):
        parent_node_id = parent_node._id if parent_node else None
        tree.try_insert(new_node, parent_node_id)

        # Ensure the new node exists in the tree
        assert new_node._id > 0
        inserted_node = next((node for node in tree.nodes if node._id == new_node._id), None)
        assert inserted_node is not None, f"New node with ID {new_node._id} not found."
        assert new_node._label == inserted_node._label

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

        cls.verify_nested_set_property(tree)

    def test_find_node_from_tree_ok(self):
        cls = type(self)
        mock_tree = TagTreeModel(_id="8964tank")
        tag_labels = ["t1", "t2", "t3", "t4", "t5", "t6", "t7"]
        mock_tags = list(map(setup_new_node, tag_labels))
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

    def test_find_nodes_bulk_ok(self):
        (_, mock_tree) = TestRemoval.setup_treenode_insertions(num_nodes=31)
        # ---- subcase 1 ----
        req_ids = [16, 19, 24]
        (actual_found, actual_missing) = mock_tree.find_nodes(req_ids)
        assert not any(actual_missing)
        assert len(req_ids) == len(actual_found)
        labels_read = set([t._label for t in actual_found])
        assert labels_read == set(["virus24", "virus19", "virus16"])
        # ---- subcase 2 ----
        req_ids = [26, 100, 5]
        (actual_found, actual_missing) = mock_tree.find_nodes(req_ids)
        assert actual_missing == [100]
        assert len(actual_found) == 2
        labels_read = [t._label for t in actual_found]
        assert set(labels_read) == set(["virus5", "virus26"])

    def test_find_ancestors_bulk_ok(self):
        (mock_tags, mock_tree) = TestRemoval.setup_treenode_insertions(num_nodes=31)
        # ---- subcase 1 ----
        req_ids = [16, 19, 12]
        (actual_found, actual_missing) = mock_tree.find_nodes(req_ids)
        assert not any(actual_missing)
        assert len(req_ids) == len(actual_found)
        assert set([n._id for n in actual_found]) == set(req_ids)
        result = mock_tree.find_ancestors_bulk(curr_nodes=actual_found)
        expect = [n._id for n in mock_tags if n._id in [1, 2, 4, 8, 9, 3, 6]]
        actual = [n._id for n in result]
        assert set(actual) == set(expect)
        # ---- subcase 2 ----
        req_ids = [20, 21, 11]
        (actual_found, _) = mock_tree.find_nodes(req_ids)
        result = mock_tree.find_ancestors_bulk(curr_nodes=actual_found)
        expect = [n._id for n in mock_tags if n._id in [1, 2, 5, 10]]
        actual = [n._id for n in result]
        assert set(actual) == set(expect)

    def test_find_ancestors_descendants_ok(self):
        cls = type(self)
        mock_tree = TagTreeModel(_id="winnnieTheFlu")

        def gen_tag_labels(nitems):
            for idx in range(nitems):
                yield "halo%d" % (idx + 1)

        mock_tags = list(map(setup_new_node, gen_tag_labels(35)))
        cls.insert_then_verify(mock_tree, parent_node=None, new_node=mock_tags[0])
        for idx_p in range(15):
            c_left = idx_p * 2 + 1
            c_right = idx_p * 2 + 2
            cls.insert_then_verify(mock_tree, mock_tags[idx_p], new_node=mock_tags[c_left])
            cls.insert_then_verify(mock_tree, mock_tags[idx_p], new_node=mock_tags[c_right])

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


class TestRemoval:
    @staticmethod
    def setup_treenode_insertions(
        num_nodes: int,
    ) -> Tuple[List[TagModel], TagTreeModel]:
        mock_tree = TagTreeModel(_id="cisipea")

        def gen_tag_labels(nitems):
            for idx in range(nitems):
                yield "virus%d" % (idx + 1)

        mock_tags = list(map(setup_new_node, gen_tag_labels(num_nodes)))
        TestCreate.insert_then_verify(mock_tree, parent_node=None, new_node=mock_tags[0])
        for idx in range(num_nodes >> 1):
            left = idx * 2 + 1
            right = idx * 2 + 2
            TestCreate.insert_then_verify(mock_tree, mock_tags[idx], new_node=mock_tags[left])
            TestCreate.insert_then_verify(mock_tree, mock_tags[idx], new_node=mock_tags[right])
        return (mock_tags, mock_tree)

    @staticmethod
    def verify_labels(nodes, expect_labels):
        actual_labels = [a._label for a in nodes]
        assert set(expect_labels) == set(actual_labels)

    @staticmethod
    def remove_then_verify(tree: TagTreeModel, node_id: int) -> Optional[TagModel]:
        removed = tree.try_remove(node_id=node_id)
        TestCreate.verify_nested_set_property(tree)
        return removed

    def test_remove_check_property_ok(self):
        cls = type(self)
        (mock_tags, mock_tree) = cls.setup_treenode_insertions(num_nodes=31)

        verify_node_ends(mock_tags[0], expect=(1, 62))
        verify_node_ends(mock_tags[1], expect=(2, 31))
        verify_node_ends(mock_tags[2], expect=(32, 61))
        verify_node_ends(mock_tags[3], expect=(3, 16))
        verify_node_ends(mock_tags[4], expect=(17, 30))
        verify_node_ends(mock_tags[9], expect=(18, 23))
        verify_node_ends(mock_tags[10], expect=(24, 29))
        removed = cls.remove_then_verify(mock_tree, 5)
        assert removed is mock_tags[4]
        verify_node_ends(mock_tags[1], expect=(2, 29))
        verify_node_ends(mock_tags[2], expect=(30, 59))
        verify_node_ends(mock_tags[3], expect=(3, 16))
        verify_node_ends(mock_tags[9], expect=(17, 22))
        verify_node_ends(mock_tags[10], expect=(23, 28))

        verify_node_ends(mock_tags[7], expect=(4, 9))
        verify_node_ends(mock_tags[14], expect=(52, 57))
        verify_node_ends(mock_tags[15], expect=(5, 6))
        verify_node_ends(mock_tags[16], expect=(7, 8))
        verify_node_ends(mock_tags[30], expect=(55, 56))
        removed = cls.remove_then_verify(mock_tree, 16)
        assert removed is mock_tags[15]
        verify_node_ends(mock_tags[7], expect=(4, 7))
        verify_node_ends(mock_tags[14], expect=(50, 55))
        verify_node_ends(mock_tags[16], expect=(5, 6))
        verify_node_ends(mock_tags[30], expect=(53, 54))
        verify_node_ends(mock_tags[1], expect=(2, 27))
        verify_node_ends(mock_tags[2], expect=(28, 57))

        verify_node_ends(mock_tags[9], expect=(15, 20))
        verify_node_ends(mock_tags[10], expect=(21, 26))
        verify_node_ends(mock_tags[21], expect=(22, 23))
        verify_node_ends(mock_tags[22], expect=(24, 25))
        removed = cls.remove_then_verify(mock_tree, 11)
        assert removed is mock_tags[10]
        verify_node_ends(mock_tags[1], expect=(2, 25))
        verify_node_ends(mock_tags[9], expect=(15, 20))
        verify_node_ends(mock_tags[21], expect=(21, 22))
        verify_node_ends(mock_tags[22], expect=(23, 24))

    def test_remove_check_ancestors(self):
        cls = type(self)
        (mock_tags, mock_tree) = cls.setup_treenode_insertions(num_nodes=31)

        removed = cls.remove_then_verify(mock_tree, 2)
        assert removed is mock_tags[1]
        asc = mock_tree.find_ancestors(mock_tags[16])
        cls.verify_labels(asc, ["virus1", "virus4", "virus8"])
        asc = mock_tree.find_ancestors(mock_tags[17])
        cls.verify_labels(asc, ["virus1", "virus4", "virus9"])

        removed = cls.remove_then_verify(mock_tree, 4)
        assert removed is mock_tags[3]
        asc = mock_tree.find_ancestors(mock_tags[16])
        cls.verify_labels(asc, ["virus1", "virus8"])
        asc = mock_tree.find_ancestors(mock_tags[17])
        cls.verify_labels(asc, ["virus1", "virus9"])

        removed = cls.remove_then_verify(mock_tree, 8)
        assert removed is mock_tags[7]
        removed = cls.remove_then_verify(mock_tree, 16)
        assert removed is mock_tags[15]
        asc = mock_tree.find_ancestors(mock_tags[16])
        cls.verify_labels(asc, ["virus1"])
        asc = mock_tree.find_ancestors(mock_tags[17])
        cls.verify_labels(asc, ["virus1", "virus9"])
        asc = mock_tree.find_ancestors(mock_tags[22])
        cls.verify_labels(asc, ["virus1", "virus5", "virus11"])
        asc = mock_tree.find_ancestors(mock_tags[23])
        cls.verify_labels(asc, ["virus1", "virus3", "virus6", "virus12"])

        removed = cls.remove_then_verify(mock_tree, 11)
        assert removed is mock_tags[10]
        asc = mock_tree.find_ancestors(mock_tags[22])
        cls.verify_labels(asc, ["virus1", "virus5"])

        desc = mock_tree.find_descendants(mock_tags[11], 2)
        cls.verify_labels(desc, ["virus24", "virus25"])
        removed = cls.remove_then_verify(mock_tree, 24)
        assert removed is mock_tags[23]
        desc = mock_tree.find_descendants(mock_tags[11], 2)
        cls.verify_labels(desc, ["virus25"])
        removed = cls.remove_then_verify(mock_tree, 25)
        assert removed is mock_tags[24]
        desc = mock_tree.find_descendants(mock_tags[11], 1)
        assert len(desc) == 0

    def test_remove_check_descendants(self):
        cls = type(self)
        (mock_tags, mock_tree) = cls.setup_treenode_insertions(num_nodes=31)

        removed = cls.remove_then_verify(mock_tree, 4)
        assert removed is mock_tags[3]
        # fmt: off
        desc = mock_tree.find_descendants(mock_tags[4], 99)
        cls.verify_labels(desc, ["virus10","virus11","virus20","virus21","virus22","virus23"])
        desc = mock_tree.find_descendants(mock_tags[5], 99)
        cls.verify_labels(desc, ["virus12","virus13","virus24","virus25","virus26","virus27"])
        # fmt: on
        removed = cls.remove_then_verify(mock_tree, 20)
        assert removed is mock_tags[19]
        desc = mock_tree.find_descendants(mock_tags[4], 99)
        cls.verify_labels(desc, ["virus10", "virus11", "virus21", "virus22", "virus23"])

    def test_remove_siblings(self):
        cls = type(self)
        (mock_tags, mock_tree) = cls.setup_treenode_insertions(num_nodes=63)

        # fmt: off
        desc = mock_tree.find_descendants(mock_tags[5], 2)
        cls.verify_labels(desc, ['virus12','virus13','virus24','virus25','virus26','virus27'])
        desc = mock_tree.find_descendants(mock_tags[6], 2)
        cls.verify_labels(desc, ['virus14','virus15','virus28','virus29','virus30','virus31'])
        # fmt: on
        for idx in range(11, 15):
            removed = cls.remove_then_verify(mock_tree, idx + 1)
            assert removed is mock_tags[idx]
        desc = mock_tree.find_descendants(mock_tags[5], 1)
        cls.verify_labels(desc, ["virus24", "virus25", "virus26", "virus27"])
        desc = mock_tree.find_descendants(mock_tags[6], 1)
        cls.verify_labels(desc, ["virus28", "virus29", "virus30", "virus31"])

        desc = mock_tree.find_descendants(mock_tags[24], 1)
        cls.verify_labels(desc, ["virus50", "virus51"])
        removed = cls.remove_then_verify(mock_tree, 25)
        assert removed is mock_tags[24]
        desc = mock_tree.find_descendants(mock_tags[5], 1)
        cls.verify_labels(desc, ["virus24", "virus50", "virus51", "virus26", "virus27"])

    def test_remove_onenode_tree(self):
        cls = type(self)
        mock_tree = TagTreeModel(_id="cisipea")
        mock_tag = setup_new_node("virus0")
        TestCreate.insert_then_verify(mock_tree, parent_node=None, new_node=mock_tag)
        assert not mock_tree.empty()
        removed = cls.remove_then_verify(mock_tree, 1)
        assert removed is mock_tag
        assert mock_tree.empty()
        removed = cls.remove_then_verify(mock_tree, 1)
        assert removed is None

    def test_remove_nonexist_node(self):
        cls = type(self)
        (mock_tags, mock_tree) = cls.setup_treenode_insertions(num_nodes=5)
        removed = cls.remove_then_verify(mock_tree, 9999)
        assert not removed
        desc = mock_tree.find_descendants(mock_tags[0], 9)
        cls.verify_labels(desc, ["virus2", "virus3", "virus4", "virus5"])
        desc = mock_tree.find_descendants(mock_tags[1], 9)
        cls.verify_labels(desc, ["virus4", "virus5"])

    def test_remove_degenerate(self):
        cls = type(self)
        mock_tree = TagTreeModel(_id="CCPbioweap0n")

        def gen_tag_labels(nitems):
            for idx in range(nitems):
                yield "virus%d" % (idx + 1)

        num_nodes = 10
        mock_tags = list(map(setup_new_node, gen_tag_labels(num_nodes)))
        TestCreate.insert_then_verify(mock_tree, parent_node=None, new_node=mock_tags[0])
        for i in range(num_nodes - 1):
            TestCreate.insert_then_verify(mock_tree, mock_tags[i], new_node=mock_tags[i + 1])

        removed = cls.remove_then_verify(mock_tree, 5)
        assert removed is mock_tags[4]
        desc = mock_tree.find_descendants(mock_tags[5], 4)
        cls.verify_labels(desc, ["virus7", "virus8", "virus9", "virus10"])
        asc = mock_tree.find_ancestors(mock_tags[5])
        cls.verify_labels(asc, ["virus1", "virus2", "virus3", "virus4"])

        removed = cls.remove_then_verify(mock_tree, 3)
        assert removed is mock_tags[2]
        asc = mock_tree.find_ancestors(mock_tags[6])
        cls.verify_labels(asc, ["virus1", "virus2", "virus4", "virus6"])


class TestTagError:
    def test_invalid_node_ids(self):
        ids_decomposed = {"goat": [19, 30], "lamb": [45, 596]}
        err = TagErrorModel.invalid_node_ids(ids_decomposed)
        expect = ["lamb-45", "goat-30", "goat-19", "lamb-596"]
        assert set(err.detail["tag_nonexist"]) == set(expect)
