from enum import Enum
from dataclasses import dataclass
from typing import Optional, List, Dict, Self

from ..api.dto import TagCreateReqDto, TagUpdateRespDto, TagNodeDto


class TagErrorReason(Enum):
    MissingParent = 1
    MissingTree = 2
    UnknownTree = 3


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


@dataclass
class TagModel:
    _label: str
    _id: int = 0  # identifier of current node
    # in this application, tree nodes are maintained using nested set model
    _limit_left: int = 0
    _limit_right: int = 0

    @classmethod
    def from_req(cls, d: TagCreateReqDto) -> Self:
        # TODO, let repository handles duplicate IDs
        return cls(_label=d.name)

    @staticmethod
    def extract_parent(
        tree: List[Self], req_parent_id: Optional[int]
    ) -> Optional[Self]:
        if req_parent_id and not tree:
            raise TagErrorModel.missing_tree(req_parent_id)
        if not req_parent_id and tree:
            raise TagErrorModel.unknown_tree(tree)

        if req_parent_id:
            parent_node = next((n for n in tree if n._id == req_parent_id), None)
            if not parent_node:
                raise TagErrorModel.missing_parent(req_parent_id)
            return parent_node

    def try_update(self, tree: List[Self], req_parent_id: Optional[int]):
        cls = type(self)
        parent_node = cls.extract_parent(tree, req_parent_id)
        if parent_node:
            new_left = parent_node._limit_right
            new_right = new_left + 1
        else:
            new_left = 1
            new_right = 2

        self._limit_left = new_left
        self._limit_right = new_right

        for node in tree:
            if node._limit_right >= new_left:
                node._limit_right += 2
            if node._limit_left > new_left:
                node._limit_left += 2

        self._id = max([node._id for node in tree], default=0) + 1
        tree.append(self)

    def to_resp(self, parent_id) -> TagUpdateRespDto:
        return TagUpdateRespDto(
            node=TagNodeDto(name=self._label, id_=self._id),
            parent=parent_id,
        )
