import logging
from typing import Dict, List, Self

from product.model import TagModel
from .. import AbstractTagRepo

_logger = logging.getLogger(__name__)


class ElasticSearchTagRepo(AbstractTagRepo):
    async def init(setting: Dict) -> Self:
        _logger.warning("ElasticSearchTagRepo.init not implemented")
        return ElasticSearchTagRepo()

    async def deinit(self):
        _logger.warning("ElasticSearchTagRepo.deinit not implemented")

    async def fetch_tree(self, tag_id: int) -> List[TagModel]:
        _logger.warning("ElasticSearchTagRepo.fetch_tree not implemented")
        return []

    async def save_tree(self, old_tree: List[TagModel]):
        _logger.warning("ElasticSearchTagRepo.save_tree not implemented")
        pass
