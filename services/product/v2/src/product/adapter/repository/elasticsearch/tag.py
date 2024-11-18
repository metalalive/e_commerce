import logging
import asyncio
from typing import Dict, Self

from aiohttp import TCPConnector, ClientSession

from product.model import TagTreeModel, TagModel
from .. import AbstractTagRepo, AppRepoError, AppRepoFnLabel

_logger = logging.getLogger(__name__)


class ElasticSearchTagRepo(AbstractTagRepo):
    def __init__(
        self,
        secure_conn_enable: bool,
        cfdntl: Dict,
        index_name: str,
        connector: TCPConnector,
    ):
        proto = "https" if secure_conn_enable else "http"
        domain_host = "%s://%s:%d" % (proto, cfdntl["HOST"], cfdntl["PORT"])
        headers = {"content-type": "application/json", "accept": "application/json"}
        self._session = ClientSession(
            base_url=domain_host, headers=headers, connector=connector
        )
        self._index_name = index_name

    async def init(setting: Dict, loop: asyncio.events.AbstractEventLoop) -> Self:
        secure_conn_enable = setting.get("ssl_enable", True)
        connector = TCPConnector(
            limit=setting.get("num_conns", 10),
            keepalive_timeout=setting.get("timeout_secs", 60),
            ssl=secure_conn_enable,
            loop=loop,
        )
        _logger.debug("ElasticSearchTagRepo.init done successfully")
        return ElasticSearchTagRepo(
            secure_conn_enable,
            cfdntl=setting["cfdntl"],
            index_name=setting["db_name"],
            connector=connector,
        )

    async def deinit(self):
        await self._session.close()
        _logger.debug("ElasticSearchTagRepo.deinit done successfully")

    async def fetch_tree(self, t_id: int) -> TagTreeModel:
        _logger.warning("ElasticSearchTagRepo.fetch_tree not implemented")
        return []

    async def save_tree(self, tree: TagTreeModel):
        if not tree.nodes:
            reason = {"num_nodes": 0}
            raise AppRepoError(fn_label=AppRepoFnLabel.TagSaveTree, reason=reason)
        cls = type(self)
        url = "/%s/the-only-type/%d" % (self._index_name, tree._id)
        reqbody = {"nodes": list(map(cls.convert_to_doc, tree.nodes))}
        resp = await self._session.put(url, json=reqbody)
        async with resp:
            if resp.status >= 400:
                reason = await resp.json()
                raise AppRepoError(fn_label=AppRepoFnLabel.TagSaveTree, reason=reason)
        _logger.debug("ElasticSearchTagRepo.save_tree done successfully")

    async def new_tree_id(self) -> int:
        _logger.warning("ElasticSearchTagRepo.new_tree_id  not implemented")
        return 1

    @staticmethod
    def convert_to_doc(m: TagModel) -> Dict:
        return {
            "label": m._label,
            "sub_id": m._id,
            "limit_left": m._limit_left,
            "limit_right": m._limit_right,
        }
