from typing import Dict, List

from product.model import TagModel


class AbstractTagRepo:
    async def init(setting: Dict):
        raise NotImplementedError("AbstractTagRepo.init")

    async def deinit(self):
        raise NotImplementedError("AbstractTagRepo.deinit")

    async def fetch_ancestors(self, tag_id: int) -> List[TagModel]:
        raise NotImplementedError("AbstractTagRepo.fetch_ancestors")

    async def create_node(self, ancesters: List[TagModel], newnode: TagModel):
        raise NotImplementedError("AbstractTagRepo.create_node")
