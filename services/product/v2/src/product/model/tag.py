from dataclasses import dataclass
from typing import Optional, List, Self
from random import randrange

from ..api.web.dto import TagCreateReqDto, TagUpdateRespDto, TagNodeDto


@dataclass
class TreeNodeLimitModel:
    left: int = 0
    right: int = 0


@dataclass
class TagModel:
    _label: str
    _id: int  # identifier of current node
    _parent: Optional[int]  # identifier of parent node
    # in this application, tree nodes are maintained using nested set model
    _range_limit: TreeNodeLimitModel

    @classmethod
    def from_req(cls, d: TagCreateReqDto) -> Self:
        limit = TreeNodeLimitModel()
        # TODO, let repository handles duplicate IDs
        _id = 1 + randrange(pow(2, 32) - 1)
        return cls(_label=d.name, _id=_id, _parent=d.parent, _range_limit=limit)

    @staticmethod
    def validate(acs: List[Self]) -> List[Self]:
        # TODO, finish implementation
        return acs

    def update_ancestors(self, acs: List[Self]):
        type(self).validate(acs)
        # TODO, finish implementation

    def to_resp(self) -> TagUpdateRespDto:
        return TagUpdateRespDto(
            node=TagNodeDto(name=self._label, id_=self._id),
            parent=self._parent,
        )
