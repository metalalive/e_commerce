from dataclasses import dataclass
from typing import Optional, Self, Dict, List, Tuple, Union

from pydantic import NonNegativeInt
from product.api.dto import (
    SaleItemAttriReqDto,
    SaleItemCreateReqDto,
    SaleItemUpdateReqDto,
    SaleItemAttriDto,
    SaleItemDto,
)
from product.util import gen_random_number

from .tag import TagModel
from .attribute import AttrLabelModel, AttriLabelError


@dataclass
class SaleItemAttriModel:
    label: AttrLabelModel
    value: Union[bool, NonNegativeInt, int, str]

    @classmethod
    def from_req(
        cls, labels: List[AttrLabelModel], reqs: List[SaleItemAttriReqDto]
    ) -> List[Self]:
        req_ids: List[str] = [r.id_ for r in reqs]
        label_ids: List[str] = [a.id_ for a in labels]
        missing_ids = set(req_ids) - set(label_ids)
        if len(missing_ids) > 0:
            raise AttriLabelError.missing_ids(missing_ids)
        objs: List[Self] = []
        errors: List[Tuple[AttrLabelModel, SaleItemAttriReqDto]] = []
        for req in reqs:
            result = [lb for lb in labels if lb.id_ == req.id_]
            label = result[0]
            if label.dtype.validate(req.value):
                obj = cls(label=label, value=req.value)
                objs.append(obj)
            else:
                errors.append((label, req))
        if any(errors):
            raise AttriLabelError.invalid_data(errors)
        return objs


@dataclass
class SaleableItemModel:
    id_: int
    usr_prof: int
    name: str
    visible: bool
    tags: Dict[str, List[TagModel]]
    attributes: List[SaleItemAttriModel]
    media_set: List[str]  # List of resource IDs to external multimedia systems

    @classmethod
    def from_req(
        cls,
        req: Union[SaleItemCreateReqDto, SaleItemUpdateReqDto],
        tag_ms_map: Dict[str, List[TagModel]],
        attri_val_ms: List[SaleItemAttriModel],
        usr_prof: int,
        id_: Optional[int] = None,
    ) -> Self:
        if not id_:
            id_ = gen_random_number(64)
        return cls(
            id_=id_,
            usr_prof=usr_prof,
            name=req.name,
            visible=req.visible,
            tags=tag_ms_map,
            attributes=attri_val_ms,
            media_set=req.media_set,
        )

    def to_dto(self) -> SaleItemDto:
        tags_d = [
            node.to_node_dto(tree_id)
            for tree_id, nodes in self.tags.items()
            for node in nodes
        ]
        attris_d = [
            SaleItemAttriDto(label=a.label.to_dto(), value=a.value)
            for a in self.attributes
        ]
        return SaleItemDto(
            id_=self.id_,
            name=self.name,
            visible=self.visible,
            usrprof=self.usr_prof,
            tags=tags_d,
            attributes=attris_d,
            media_set=self.media_set,
        )
