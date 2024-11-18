from enum import Enum, auto
from typing import Dict
from dataclasses import dataclass

from product.model import TagTreeModel


class AppRepoFnLabel(Enum):
    TagSaveTree = auto()


@dataclass
class AppRepoError(Exception):
    fn_label: AppRepoFnLabel
    reason: Dict


class AbstractTagRepo:
    async def init(setting: Dict):
        raise NotImplementedError("AbstractTagRepo.init")

    async def deinit(self):
        raise NotImplementedError("AbstractTagRepo.deinit")

    async def fetch_tree(self, t_id: int) -> TagTreeModel:
        raise NotImplementedError("AbstractTagRepo.fetch_tree")

    async def save_tree(self, tree: TagTreeModel):
        raise NotImplementedError("AbstractTagRepo.save_tree")

    async def new_tree_id(self) -> int:
        raise NotImplementedError("AbstractTagRepo.new_tree_id")
