from dataclasses import dataclass
from typing import List, Self, Dict

from product.api.dto import (
    AttrCreateReqDto,
    AttrUpdateReqDto,
    AttrLabelDto,
    AttrDataTypeDto,
)
from product.util import gen_random_string


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

    def to_resps(ms: List[Self]) -> List[Dict]:
        return [
            AttrLabelDto(id_=m.id_, name=m.name, dtype=m.dtype).model_dump() for m in ms
        ]
