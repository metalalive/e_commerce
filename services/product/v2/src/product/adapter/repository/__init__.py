from typing import Dict, List

from product.model import TagModel


class AbstractTagRepo:
    async def init(setting: Dict):
        raise NotImplementedError("AbstractTagRepo.init")

    async def deinit(self):
        raise NotImplementedError("AbstractTagRepo.deinit")

    async def fetch_tree(self, tag_id: int) -> List[TagModel]:
        raise NotImplementedError("AbstractTagRepo.fetch_tree")

    async def save_tree(self, old_tree: List[TagModel]):
        raise NotImplementedError("AbstractTagRepo.save_tree")
