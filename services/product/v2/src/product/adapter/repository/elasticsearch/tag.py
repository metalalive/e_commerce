import logging
from typing import Dict, Self

from product.model import TagTreeModel
from .. import AbstractTagRepo

_logger = logging.getLogger(__name__)


class ElasticSearchTagRepo(AbstractTagRepo):
    async def init(setting: Dict) -> Self:
        _logger.warning("ElasticSearchTagRepo.init not implemented")
        return ElasticSearchTagRepo()

    async def deinit(self):
        _logger.warning("ElasticSearchTagRepo.deinit not implemented")

    async def fetch_tree(self, t_id: int) -> TagTreeModel:
        _logger.warning("ElasticSearchTagRepo.fetch_tree not implemented")
        return []

    async def save_tree(self, tree: TagTreeModel):
        _logger.warning("ElasticSearchTagRepo.save_tree not implemented")
        pass

    async def new_tree_id(self) -> int:
        _logger.warning("ElasticSearchTagRepo.new_tree_id  not implemented")
        return 1
