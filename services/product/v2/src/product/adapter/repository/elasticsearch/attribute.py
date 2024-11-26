import logging
from typing import Dict, List, Self
from asyncio.events import AbstractEventLoop

from product.model import AttrLabelModel
from .. import AbstractAttrLabelRepo

_logger = logging.getLogger(__name__)


class ElasticSearchAttrLabelRepo(AbstractAttrLabelRepo):
    async def init(setting: Dict, loop: AbstractEventLoop) -> Self:
        _logger.debug("ElasticSearchAttrLabelRepo.init done successfully")
        return ElasticSearchAttrLabelRepo()

    async def deinit(self):
        _logger.debug("ElasticSearchAttrLabelRepo.deinit done successfully")

    async def create(self, ms: List[AttrLabelModel]):
        _logger.debug("ElasticSearchAttrLabelRepo.create done successfully")

    async def update(self, ms: List[AttrLabelModel]):
        _logger.debug("ElasticSearchAttrLabelRepo.update done successfully")

    async def delete(self, ids: List[str]):
        _logger.debug("ElasticSearchAttrLabelRepo.delete done successfully")

    async def search(self, keyword: str) -> List[AttrLabelModel]:
        _logger.debug("ElasticSearchAttrLabelRepo.search done successfully")
        return []
