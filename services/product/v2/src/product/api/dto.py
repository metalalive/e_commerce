from enum import Enum
from typing import Optional, List, Union
from pydantic import BaseModel, Field, NonNegativeInt


class TagUpdateReqDto(BaseModel):
    name: str
    parent: Optional[str]


TagCreateReqDto = TagUpdateReqDto


class TagNodeDto(BaseModel):
    name: str
    id_: str


class TagUpdateRespDto(BaseModel):
    node: TagNodeDto
    parent: Optional[str]


class TagReadRespDto(BaseModel):
    curr_node: TagNodeDto
    ancestors: Optional[List[TagNodeDto]]
    descendants: Optional[List[TagNodeDto]]
    # children: Optional[List[int]]
    # item_cnt: int # TODO, add these aggregate data
    # pkg_cnt: int


class AttrDataTypeDto(Enum):
    Integer = 1
    UnsignedInteger = 2
    String = 3
    Boolean = 4

    def validate(self, value) -> bool:
        cls = type(self)
        if self == cls.Integer:
            return int is type(value)
        elif self == cls.UnsignedInteger:
            return (int is type(value)) and (value >= 0)
        elif self == cls.String:
            return str is type(value)
        elif self == cls.Boolean:
            return bool is type(value)
        else:
            return False


class AttrCreateReqDto(BaseModel):
    name: str = Field(min_length=2, max_length=128)
    dtype: AttrDataTypeDto


class AttrLabelDto(BaseModel):
    id_: str
    name: str = Field(min_length=2, max_length=128)
    dtype: AttrDataTypeDto


AttrUpdateReqDto = AttrLabelDto


class SaleItemAttriReqDto(BaseModel):
    id_: str  # References AttrLabelDto ID
    value: Union[bool, NonNegativeInt, int, str] = Field(union_mode="left_to_right")


class SaleItemCreateReqDto(BaseModel):
    name: str
    visible: bool
    tags: List[str]  # List of IDs to TagNodeDto references
    attributes: List[SaleItemAttriReqDto]
    media_set: List[str]  # List of resource IDs to external multimedia systems


SaleItemUpdateReqDto = SaleItemCreateReqDto


class SaleItemAttriDto(BaseModel):
    label: AttrLabelDto
    value: Union[bool, NonNegativeInt, int, str] = Field(union_mode="left_to_right")


class SaleItemDto(BaseModel):
    id_: int
    name: str
    visible: bool
    usrprof: int
    tags: List[TagNodeDto]
    attributes: List[SaleItemAttriDto]
    media_set: List[str]
