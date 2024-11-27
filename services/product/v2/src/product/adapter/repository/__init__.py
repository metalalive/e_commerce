from enum import Enum, auto
from typing import Dict, List, Self
from dataclasses import dataclass
from asyncio.events import AbstractEventLoop

from product.model import TagTreeModel, AttrLabelModel


class AppRepoFnLabel(Enum):
    TagSaveTree = auto()
    TagFetchTree = auto()
    TagDeleteTree = auto()
    TagNewTreeID = auto()
    AttrLabelCreate = auto()
    AttrLabelUpdate = auto()
    AttrLabelDelete = auto()
    AttrLabelSearch = auto()


@dataclass
class AppRepoError(Exception):
    fn_label: AppRepoFnLabel
    reason: Dict


class AbstractTagRepo:
    async def init(setting: Dict, loop: AbstractEventLoop):
        raise NotImplementedError("AbstractTagRepo.init")

    async def deinit(self):
        raise NotImplementedError("AbstractTagRepo.deinit")

    async def fetch_tree(self, t_id: str) -> TagTreeModel:
        raise NotImplementedError("AbstractTagRepo.fetch_tree")

    async def save_tree(self, tree: TagTreeModel):
        raise NotImplementedError("AbstractTagRepo.save_tree")

    async def delete_tree(self, tree: TagTreeModel):
        raise NotImplementedError("AbstractTagRepo.delete_tree")

    async def new_tree_id(self) -> str:
        raise NotImplementedError("AbstractTagRepo.new_tree_id")


class AbstractAttrLabelRepo:
    async def init(setting: Dict, loop: AbstractEventLoop) -> Self:
        raise NotImplementedError("AbstractAttrLabelRepo.init")

    async def deinit(self):
        raise NotImplementedError("AbstractAttrLabelRepo.deinit")

    async def create(self, ms: List[AttrLabelModel]):
        raise NotImplementedError("AbstractAttrLabelRepo.create")

    async def update(self, ms: List[AttrLabelModel]):
        raise NotImplementedError("AbstractAttrLabelRepo.update")

    async def delete(self, ids: List[str]):
        raise NotImplementedError("AbstractAttrLabelRepo.delete")

    async def search(self, keyword: str) -> List[AttrLabelModel]:
        raise NotImplementedError("AbstractAttrLabelRepo.search")
