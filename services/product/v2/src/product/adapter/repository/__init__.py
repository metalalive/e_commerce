from typing import Dict

from product.model import TagTreeModel


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
