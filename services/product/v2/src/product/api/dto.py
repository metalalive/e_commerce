from typing import Optional, List
from pydantic import BaseModel


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
