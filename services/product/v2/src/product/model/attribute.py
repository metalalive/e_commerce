from __future__ import annotations

from enum import Enum
from dataclasses import dataclass
from typing import TYPE_CHECKING, List, Self, Dict, Union, Tuple

from product.api.dto import (
    AttrCreateReqDto,
    AttrUpdateReqDto,
    AttrLabelDto,
    AttrDataTypeDto,
    SaleItemAttriReqDto,
)
from product.util import gen_random_string

if TYPE_CHECKING:  # to resolve mutual reference below

    class AttriLabelError:
        pass

    class AttrLabelModel:
        pass


class AttriLabelErrorReason(Enum):
    MissingID = 1
    InvalidData = 2


@dataclass
class AttriLabelError(Exception):
    reason: AttriLabelErrorReason
    detail: List[Union[str, Dict]]

    @classmethod
    def missing_ids(cls, ids: List[str]) -> Self:
        return cls(reason=AttriLabelErrorReason.MissingID, detail=ids)

    @classmethod
    def invalid_data(cls, e: List[Tuple[AttrLabelModel, SaleItemAttriReqDto]]) -> Self:
        def gen_msg(pair: Tuple[AttrLabelModel, SaleItemAttriReqDto]) -> Dict:
            label, req = pair
            return {
                "id": label.id_,
                "expect_dtype": label.dtype,
                "received_value": req.value,
            }

        detail = list(map(gen_msg, e))
        return cls(reason=AttriLabelErrorReason.InvalidData, detail=detail)


@dataclass
class AttrLabelModel:
    id_: str
    name: str
    dtype: AttrDataTypeDto

    def from_create_reqs(data: List[AttrCreateReqDto]) -> List[Self]:
        def init_one(d: AttrCreateReqDto) -> Self:
            new_id = gen_random_string(max_length=6)
            return AttrLabelModel(id_=new_id, name=d.name, dtype=d.dtype)

        return list(map(init_one, data))

    def from_update_reqs(data: List[AttrUpdateReqDto]) -> List[Self]:
        def init_one(d: AttrUpdateReqDto) -> Self:
            return AttrLabelModel(id_=d.id_, name=d.name, dtype=d.dtype)

        return list(map(init_one, data))

    def to_dto(self) -> AttrLabelDto:
        return AttrLabelDto(id_=self.id_, name=self.name, dtype=self.dtype)

    def to_resps(ms: List[Self]) -> List[Dict]:
        return [m.to_dto().model_dump() for m in ms]

    def rotate_id(self) -> Self:
        self.id_ = gen_random_string(max_length=6)
        return self
