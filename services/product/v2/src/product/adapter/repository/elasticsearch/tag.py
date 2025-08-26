import logging
from asyncio.events import AbstractEventLoop
from typing import Dict, Self, List

from aiohttp import TCPConnector, ClientSession

from product.model import TagTreeModel, TagModel
from product.util import gen_random_string
from .. import AbstractTagRepo, AppRepoError, AppRepoFnLabel

_logger = logging.getLogger(__name__)


class ElasticSearchTagRepo(AbstractTagRepo):
    def __init__(
        self,
        secure_conn_enable: bool,
        cfdntl: Dict,
        address: Dict,
        index_name: str,
        connector: TCPConnector,
        tree_id_length: int,
    ):
        proto = "https" if secure_conn_enable else "http"
        domain_host = "%s://%s:%d" % (proto, address["HOST"], address["PORT"])
        headers = {"content-type": "application/json", "accept": "application/json"}
        self._session = ClientSession(base_url=domain_host, headers=headers, connector=connector)
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
            address=setting["db_addr"],
            connector=connector,
            tree_id_length=tree_id_length,
        )

    async def deinit(self):
        await self._session.close()
        _logger.debug("ElasticSearchTagRepo.deinit done successfully")

    async def fetch_tree(self, t_id: str) -> TagTreeModel:
        url = "/%s/_doc/%s" % (self._index_name, t_id)
        resp = await self._session.get(url)
        async with resp:
            respbody = await resp.json()
            if resp.status < 300:
                cls = type(self)
                nodes = cls.parse_doc_tree_nodes(respbody)
            else:
                raise AppRepoError(fn_label=AppRepoFnLabel.TagFetchTree, reason=respbody)
        _logger.debug("ElasticSearchTagRepo.fetch_tree done")
        return TagTreeModel(_id=t_id, nodes=nodes)

    async def save_tree(self, tree: TagTreeModel):
        if not tree.nodes:
            reason = {"num_nodes": 0}
            raise AppRepoError(fn_label=AppRepoFnLabel.TagSaveTree, reason=reason)
        cls = type(self)
        url = "/%s/_doc/%s" % (self._index_name, tree._id)
        reqbody = {"nodes": list(map(cls.convert_to_doc, tree.nodes))}
        resp = await self._session.post(url, json=reqbody)
        async with resp:
            if resp.status >= 400:
                reason = await resp.json()
                raise AppRepoError(fn_label=AppRepoFnLabel.TagSaveTree, reason=reason)
        _logger.debug("ElasticSearchTagRepo.save_tree done")

    async def delete_tree(self, tree: TagTreeModel):
        url = "/%s/_doc/%s" % (self._index_name, tree._id)
        resp = await self._session.delete(url)
        async with resp:
            if resp.status >= 400:
                reason = {"found": False, "tree_id": tree._id}
                raise AppRepoError(fn_label=AppRepoFnLabel.TagDeleteTree, reason=reason)
        _logger.debug("ElasticSearchTagRepo.delete_tree done")

    async def new_tree_id(self) -> str:
        next_doc_id = None
        respbody = None
        url = "/%s/_search" % (self._index_name)
        reqbody = {
            "_source": False,
            "stored_fields": ["_none_"],
            "query": {"term": {"_id": None}},
        }
        for _ in range(5):
            candidate = gen_random_string(max_length=self._tree_id_length)
            reqbody["query"]["term"]["_id"] = candidate
            resp = await self._session.get(url, json=reqbody)
            async with resp:
                respbody = await resp.json()
                # print("[debug] new-tree-id, response body: %s \n" % str(respbody))
                if respbody["hits"]["total"]["value"] == 0:
                    next_doc_id = candidate
                    break
        if not next_doc_id:
            reason = {"detail": "too-many-conflict", "low-level": respbody}
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
