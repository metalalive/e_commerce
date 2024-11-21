import logging
import random
import string
from datetime import datetime
from asyncio.events import AbstractEventLoop
from typing import Dict, Self, List

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
        tree_id_length: int,
    ):
        proto = "https" if secure_conn_enable else "http"
        domain_host = "%s://%s:%d" % (proto, cfdntl["HOST"], cfdntl["PORT"])
        headers = {"content-type": "application/json", "accept": "application/json"}
        self._session = ClientSession(
            base_url=domain_host, headers=headers, connector=connector
        )
        self._index_name = index_name
        self._tree_id_length = tree_id_length

    async def init(setting: Dict, loop: AbstractEventLoop) -> Self:
        secure_conn_enable = setting.get("ssl_enable", True)
        connector = TCPConnector(
            limit=setting.get("num_conns", 10),
            keepalive_timeout=setting.get("timeout_secs", 60),
            ssl=secure_conn_enable,
            loop=loop,
        )
        _logger.debug("ElasticSearchTagRepo.init done successfully")
        tree_id_length = setting.get("tree_id_length", 8)
        return ElasticSearchTagRepo(
            secure_conn_enable,
            cfdntl=setting["cfdntl"],
            index_name=setting["db_name"],
            connector=connector,
            tree_id_length=tree_id_length,
        )

    async def deinit(self):
        await self._session.close()
        _logger.debug("ElasticSearchTagRepo.deinit done successfully")

    async def fetch_tree(self, t_id: str) -> TagTreeModel:
        url = "/%s/the-only-type/%s" % (self._index_name, t_id)
        resp = await self._session.get(url)
        async with resp:
            respbody = await resp.json()
            if resp.status < 300:
                cls = type(self)
                nodes = cls.parse_doc_tree_nodes(respbody)
            else:
                raise AppRepoError(
                    fn_label=AppRepoFnLabel.TagFetchTree, reason=respbody
                )
        _logger.debug("ElasticSearchTagRepo.fetch_tree done")
        return TagTreeModel(_id=t_id, nodes=nodes)

    async def save_tree(self, tree: TagTreeModel):
        if not tree.nodes:
            reason = {"num_nodes": 0}
            raise AppRepoError(fn_label=AppRepoFnLabel.TagSaveTree, reason=reason)
        cls = type(self)
        url = "/%s/the-only-type/%s" % (self._index_name, tree._id)
        reqbody = {"nodes": list(map(cls.convert_to_doc, tree.nodes))}
        resp = await self._session.put(url, json=reqbody)
        async with resp:
            if resp.status >= 400:
                reason = await resp.json()
                raise AppRepoError(fn_label=AppRepoFnLabel.TagSaveTree, reason=reason)
        _logger.debug("ElasticSearchTagRepo.save_tree done")

    async def new_tree_id(self) -> str:
        t0 = datetime.now()
        random.seed(a=t0.timestamp())
        next_doc_id = None
        characters = string.ascii_letters + string.digits
        url = "/%s/the-only-type/_search" % (self._index_name)
        reqbody = {
            "_source": False,
            "stored_fields": ["_none_"],
            "query": {"term": {"_id": None}},
        }
        for _ in range(5):
            candidate = "".join(random.choices(characters, k=self._tree_id_length))
            reqbody["query"]["term"]["_id"] = candidate
            resp = await self._session.get(url, json=reqbody)
            async with resp:
                respbody = await resp.json()
                if respbody["hits"]["total"] == 0:
                    next_doc_id = candidate
                    break
        if not next_doc_id:
            reason = {"detail": "too-many-conflict"}
            raise AppRepoError(fn_label=AppRepoFnLabel.TagNewTreeID, reason=reason)
        _logger.debug("ElasticSearchTagRepo.new_tree_id  done")
        return next_doc_id

    @staticmethod
    def convert_to_doc(m: TagModel) -> Dict:
        return {
            "label": m._label,
            "sub_id": m._id,
            "limit_left": m._limit_left,
            "limit_right": m._limit_right,
        }

    @staticmethod
    def convert_from_doc(d: Dict) -> TagModel:
        return TagModel(
            _label=d["label"],
            _id=d["sub_id"],
            _limit_left=d["limit_left"],
            _limit_right=d["limit_right"],
        )

    @classmethod
    def parse_doc_tree_nodes(cls, raw: Dict) -> List[TagModel]:
        try:
            assert raw["found"]
            assert raw["_id"]
            nodes_raw = raw["_source"]["nodes"]
            nodes = list(map(cls.convert_from_doc, nodes_raw))
        except Exception as e:
            reason = {
                "req_tree_id": raw.get("_id", -1),
                "corrupt_on_parsing": True,
                "detail": str(e),
            }
            raise AppRepoError(fn_label=AppRepoFnLabel.TagFetchTree, reason=reason)
        return nodes
