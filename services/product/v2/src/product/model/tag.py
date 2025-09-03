from enum import Enum
import logging
import re
from dataclasses import dataclass, field
from typing import Tuple, Optional, List, Dict, Self

from ..api.dto import TagCreateReqDto, TagUpdateRespDto, TagNodeDto

_logger = logging.getLogger(__name__)


class TagErrorReason(Enum):
    MissingParent = 1
    MissingTree = 2
    UnknownTree = 3
    DecodeInvalidId = 4
    InvalidNodeLimitRange = 5
    InvalidNodeIDs = 6


@dataclass
class TagErrorModel(Exception):
    reason: TagErrorReason
    detail: Dict

    @classmethod
    def missing_tree(cls, parent_id: int) -> Self:
        return cls(
            reason=TagErrorReason.MissingTree,
            detail={"req_parent_id": parent_id},
        )

    @classmethod
    def unknown_tree(cls, tree: List["TagModel"]) -> Self:
        def extract_t_id(tag: TagModel) -> int:
            tag._id

        tree_ids = list(map(extract_t_id, tree))
        return cls(
            reason=TagErrorReason.UnknownTree,
            detail={"tree_ids": tree_ids},
        )

    @classmethod
    def missing_parent(cls, parent_id: int) -> Self:
        return cls(
            reason=TagErrorReason.MissingParent,
            detail={"req_parent_id": parent_id},
        )

    @classmethod
    def decode_invalid_id(cls, orig_req_id: str) -> Self:
        return cls(
            reason=TagErrorReason.DecodeInvalidId,
            detail={"req_id": orig_req_id},
        )

    @classmethod
    def invalid_limit_range(cls, tree_id: int, node_id: int) -> Self:
        return cls(
            reason=TagErrorReason.InvalidNodeLimitRange,
            detail={
                "tree_id": tree_id,
                "node_id": node_id,
            },
        )

    @classmethod
    def invalid_node_ids(cls, ids_decomposed: Dict[str, List[int]]) -> Self:
        tag_ids_total = []
        for tree_id, node_ids in ids_decomposed.items():
            tag_ids = ["%s-%d" % (tree_id, nid) for nid in node_ids]
            tag_ids_total.extend(tag_ids)

        return cls(reason=TagErrorReason.InvalidNodeIDs, detail={"tag_nonexist": tag_ids_total})


@dataclass
class TagModel:
    _label: str
    _id: int = 0  # identifier of current node
    # in this application, tree nodes are maintained using nested set model,
    # parent node can be calculated by the range limit , so I don't declare
    # extra field for parent node ID
    _limit_left: int = 0
    _limit_right: int = 0

    def reset_limit_range(self):
        self._limit_right = 0
        self._limit_left = 0

    @classmethod
    def from_req(cls, d: TagCreateReqDto) -> Self:
        # TODO, let repository handles duplicate IDs
        return cls(_label=d.name)

    def to_resp(self, tree_id: str, parent_node_id: Optional[int]) -> TagUpdateRespDto:
        if parent_node_id:
            parent_id_resp = "%s-%d" % (tree_id, parent_node_id)
        else:
            parent_id_resp = None
        curr_id_resp = "%s-%d" % (tree_id, self._id)
        return TagUpdateRespDto(
            node=TagNodeDto(name=self._label, id_=curr_id_resp),
            parent=parent_id_resp,
        )

    def to_node_dto(self, tree_id: str) -> TagNodeDto:
        curr_id_resp = "%s-%d" % (tree_id, self._id)
        return TagNodeDto(name=self._label, id_=curr_id_resp)

    def decode_req_id(id_s: str) -> Tuple[str, int]:
        match = re.fullmatch(r"([a-zA-Z0-9]+)-(\d+)$", id_s)
        if not match:
            raise TagErrorModel.decode_invalid_id(id_s)
        try:
            tree_id = match.group(1)
            parent_node_id = int(match.group(2))
        except Exception as e:
            _logger.debug("%s", str(e))
            raise TagErrorModel.decode_invalid_id(id_s)
        return (tree_id, parent_node_id)

    def is_ancestor_of(self, other: Self) -> bool:
        if (self is other) or (self._id == other._id):
            return False
        left_covered = other._limit_left > self._limit_left
        right_covered = other._limit_right < self._limit_right
        return left_covered and right_covered


@dataclass
class TagTreeModel:
    _id: str  # identifier of the tree
    nodes: List[TagModel] = field(default_factory=list)

    def extract_parent(self, parent_node_id: Optional[int]) -> Optional[TagModel]:
        if parent_node_id and not self.nodes:
            raise TagErrorModel.missing_tree(parent_node_id)
        if not parent_node_id and self.nodes:
            raise TagErrorModel.unknown_tree(self.nodes)

        if parent_node_id:
            parent_node = next((n for n in self.nodes if n._id == parent_node_id), None)
            if not parent_node:
                raise TagErrorModel.missing_parent(parent_node_id)
            return parent_node

    def try_insert(self, newnode: TagModel, parent_node_id: Optional[int]):
        if newnode._limit_left != 0 or newnode._limit_right != 0:
            raise TagErrorModel.invalid_limit_range(self._id, newnode._id)
        parent_node = self.extract_parent(parent_node_id)
        if parent_node:
            new_left = parent_node._limit_right
            new_right = new_left + 1
        else:
            assert len(self.nodes) == 0
            new_left = 1
            new_right = 2

        for node in self.nodes:
            if node._limit_right >= new_left:
                node._limit_right += 2
            if node._limit_left > new_left:
                node._limit_left += 2

        newnode._limit_left = new_left
        newnode._limit_right = new_right
        newnode._id = max([node._id for node in self.nodes], default=0) + 1
        self.nodes.append(newnode)

    def find_node(self, node_id: int) -> Optional[TagModel]:
        def _find_by_id(n: TagModel):
            return n._id == node_id

        iter0 = filter(_find_by_id, self.nodes)
        try:
            node = next(iter0)
        except StopIteration:
            node = None
        return node

    def find_nodes(self, node_ids: List[int]) -> Tuple[List[TagModel], List[int]]:
        def _find_by_id(n: TagModel):
            return n._id in node_ids

        found = list(filter(_find_by_id, self.nodes))
        found_ids = [n._id for n in found]
        not_found = set(node_ids) - set(found_ids)
        return (found, list(not_found))

    def find_ancestors(self, curr_node: TagModel) -> List[TagModel]:
        def find_by_limit(node: TagModel) -> bool:
            return node.is_ancestor_of(curr_node)

        return list(filter(find_by_limit, self.nodes))

    def find_ancestors_bulk(self, curr_nodes: List[TagModel]) -> List[TagModel]:
        def find_by_limit(node: TagModel) -> bool:
            acs_found = [node.is_ancestor_of(curr_node) for curr_node in curr_nodes]
            return any(acs_found)

        return list(filter(find_by_limit, self.nodes))

    def find_descendants(self, curr_node: TagModel, max_desc_lvl: int) -> List[TagNodeDto]:
        if max_desc_lvl <= 0:
            return []

        def find_by_limit(node: TagModel) -> bool:
            return curr_node.is_ancestor_of(node)

        all_descs = list(filter(find_by_limit, self.nodes))

        def sort_by_left_limit(node: TagModel) -> int:
            return node._limit_left

        all_descs.sort(key=sort_by_left_limit)
        chosen = []
        for dsc in all_descs:
            curr_lvl = sum([1 for asc in all_descs if asc.is_ancestor_of(dsc)])
            if curr_lvl < max_desc_lvl:
                chosen.append(dsc)
        return chosen

    def ancestors_dto(self, curr_node: TagModel) -> List[TagNodeDto]:
        ms = self.find_ancestors(curr_node)
        return [m.to_node_dto(self._id) for m in ms]

    def descendants_dto(self, curr_node: TagModel, desc_lvl: int) -> List[TagNodeDto]:
        ms = self.find_descendants(curr_node, desc_lvl)
        return [m.to_node_dto(self._id) for m in ms]

    def try_remove(self, node_id: int) -> Optional[TagModel]:
        node2rm = self.find_node(node_id)
        if not node2rm:
            return None
        self.nodes.remove(node2rm)
        if not self.empty():
            for node in self.nodes:
                if node2rm.is_ancestor_of(node):
                    node._limit_left -= 1
                    node._limit_right -= 1
                else:
                    if node._limit_left > node2rm._limit_left:
                        node._limit_left -= 2
                    if node._limit_right > node2rm._limit_right:
                        node._limit_right -= 2
        return node2rm

    def empty(self) -> bool:
        return len(self.nodes) == 0
