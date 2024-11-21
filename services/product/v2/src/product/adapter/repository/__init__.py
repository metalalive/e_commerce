from enum import Enum, auto
from typing import Dict
from dataclasses import dataclass
from asyncio.events import AbstractEventLoop

from product.model import TagTreeModel


class AppRepoFnLabel(Enum):
    TagSaveTree = auto()
    TagFetchTree = auto()
    TagNewTreeID = auto()


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

    async def new_tree_id(self) -> str:
        raise NotImplementedError("AbstractTagRepo.new_tree_id")
