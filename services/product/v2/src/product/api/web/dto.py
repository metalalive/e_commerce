from typing import Optional, List
from pydantic import BaseModel


class TagCreateReqDto(BaseModel):
    name: str
    parent: Optional[int]


class TagUpdateReqDto(BaseModel):
    name: str
    curr_parent: Optional[int]
    new_parent: Optional[int]


class TagNodeDto(BaseModel):
    name: str
    id_: int


class TagUpdateRespDto(BaseModel):
    node: TagNodeDto
    parent: Optional[int]


class TagReadRespDto(BaseModel):
    curr_node: TagNodeDto
    ancestors: Optional[List[TagNodeDto]]
    descendants: Optional[List[TagNodeDto]]
    # children: Optional[List[int]]
    # item_cnt: int # TODO, add these aggregate data
    # pkg_cnt: int
