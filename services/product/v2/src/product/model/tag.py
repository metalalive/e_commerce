from enum import Enum
from dataclasses import dataclass, field
import logging
import re
from typing import Tuple, Optional, List, Dict, Self

from ..api.dto import TagCreateReqDto, TagUpdateRespDto, TagNodeDto

_logger = logging.getLogger(__name__)


class TagErrorReason(Enum):
    MissingParent = 1
    MissingTree = 2
    UnknownTree = 3
    DecodeInvalidId = 4
    InvalidNodeLimitRange = 5


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


@dataclass
class TagModel:
    _label: str
    _id: int = 0  # identifier of current node
    # in this application, tree nodes are maintained using nested set model,
    # parent node can be calculated by the range limit , so I don't declare
    # extra field for parent node ID
    _limit_left: int = 0
    _limit_right: int = 0

    @classmethod
    def from_req(cls, d: TagCreateReqDto) -> Self:
        # TODO, let repository handles duplicate IDs
        return cls(_label=d.name)

    def to_resp(self, tree_id: int, parent_node_id: Optional[int]) -> TagUpdateRespDto:
        if parent_node_id:
            parent_id_resp = "%d-%d" % (tree_id, parent_node_id)
        else:
            parent_id_resp = None
        curr_id_resp = "%d-%d" % (tree_id, self._id)
        return TagUpdateRespDto(
            node=TagNodeDto(name=self._label, id_=curr_id_resp),
            parent=parent_id_resp,
        )

    def decode_req_id(id_s: str) -> Tuple[int, int]:
        match = re.fullmatch(r"(\d+)-(\d+)", id_s)
        if not match:
            raise TagErrorModel.decode_invalid_id(id_s)
        try:
            tree_id = int(match.group(1))
            parent_node_id = int(match.group(2))
        except Exception as e:
            _logger.debug("%s", str(e))
            raise TagErrorModel.decode_invalid_id(id_s)
        return (tree_id, parent_node_id)


@dataclass
class TagTreeModel:
    _id: int  # identifier of the tree
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
