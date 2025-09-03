import logging
import json
from typing import Tuple, Dict, List, Self
from asyncio.events import AbstractEventLoop

from aiohttp import TCPConnector, ClientSession

from ecommerce_common.util import flatten_nested_iterable
from product.api.dto import AttrDataTypeDto
from product.model import AttrLabelModel
from .. import AbstractAttrLabelRepo, AppRepoError, AppRepoFnLabel

_logger = logging.getLogger(__name__)


class ElasticSearchAttrLabelRepo(AbstractAttrLabelRepo):
    def __init__(
        self,
        secure_conn_enable: bool,
        cfdntl: Dict,
        address: Dict,
        index_name: str,
        connector: TCPConnector,
    ):
        proto = "https" if secure_conn_enable else "http"
        domain_host = "%s://%s:%d" % (proto, address["HOST"], address["PORT"])
        headers = {"content-type": "application/x-ndjson", "accept": "application/json"}
        self._session = ClientSession(base_url=domain_host, headers=headers, connector=connector)
        self._index_name = index_name

    async def init(setting: Dict, loop: AbstractEventLoop) -> Self:
        secure_conn_enable = setting.get("ssl_enable", True)
        connector = TCPConnector(
            limit=setting.get("num_conns", 2),
            keepalive_timeout=setting.get("timeout_secs", 30),
            ssl=secure_conn_enable,
            loop=loop,
        )
        _logger.debug("ElasticSearchAttrLabelRepo.init done successfully")
        return ElasticSearchAttrLabelRepo(
            secure_conn_enable,
            cfdntl=setting["cfdntl"],
            address=setting["db_addr"],
            index_name=setting["db_name"],
            connector=connector,
        )

    async def deinit(self):
        await self._session.close()
        _logger.debug("ElasticSearchAttrLabelRepo.deinit done successfully")

    @staticmethod
    def serial_nd_json(d: Dict) -> str:
        return json.dumps(d) + "\n"

    async def create(self, ms: List[AttrLabelModel]):
        if not ms:
            reason = {"detail": "input-empty"}
            raise AppRepoError(fn_label=AppRepoFnLabel.AttrLabelCreate, reason=reason)
        cls = type(self)
        fields_present = [
            "items.create._id",
            "items.create.status",
            "items.create.error",
            "items.create.created",
            "errors",
        ]
        url = "/%s/_bulk?filter_path=%s" % (
            self._index_name,
            ",".join(fields_present),
        )
        ms_work = [m for m in ms]

        def convert_to_doc(m: AttrLabelModel) -> Tuple[str, str]:
            metadata = {"create": {"_id": m.id_}}
            source = {"name": m.name, "dtype": m.dtype.value}
            return (
                cls.serial_nd_json(metadata),
                cls.serial_nd_json(source),
            )

        retry = 5
        while True:
            req_ops_gen = flatten_nested_iterable(map(convert_to_doc, ms_work))
            req_ops = list(req_ops_gen)
            resp = await self._session.post(url, data="".join(req_ops))
            async with resp:
                respbody = await resp.json()
            if resp.status >= 400:
                raise AppRepoError(fn_label=AppRepoFnLabel.AttrLabelCreate, reason=respbody)
            if respbody["errors"]:
                recoverable_err_status = [409]
                conflict_ids = [
                    d["create"]["_id"]
                    for d in respbody["items"]
                    if d["create"]["status"] in recoverable_err_status
                ]
                ms_work = [m.rotate_id() for m in ms_work if m.id_ in conflict_ids]
                if ms_work:
                    if retry == 0:
                        raise AppRepoError(
                            fn_label=AppRepoFnLabel.AttrLabelCreate, reason=respbody
                        )  # partially created
                    retry -= 1
                else:  # unable to recover
                    raise AppRepoError(
                        fn_label=AppRepoFnLabel.AttrLabelCreate, reason=respbody
                    )  # partially created
            else:
                break

        _logger.debug("ElasticSearchAttrLabelRepo.create done successfully")

    async def update(self, ms: List[AttrLabelModel]):
        if not ms:
            reason = {"detail": "input-empty"}
            raise AppRepoError(fn_label=AppRepoFnLabel.AttrLabelUpdate, reason=reason)
        cls = type(self)
        fields_present = [
            "items.update._id",
            "items.update.status",
            "items.update.error",
            "items.update.result",
            "errors",
        ]
        url = "/%s/_bulk?filter_path=%s" % (
            self._index_name,
            ",".join(fields_present),
        )

        def convert_to_doc(m: AttrLabelModel) -> Tuple[str, str]:
            metadata = {"update": {"_id": m.id_}}
            source = {"doc": {"name": m.name, "dtype": m.dtype.value}}
            return (
                cls.serial_nd_json(metadata),
                cls.serial_nd_json(source),
            )

        req_ops_gen = flatten_nested_iterable(map(convert_to_doc, ms))
        req_ops = list(req_ops_gen)
        resp = await self._session.post(url, data="".join(req_ops))
        async with resp:
            respbody = await resp.json()
        if respbody["errors"]:
            raise AppRepoError(fn_label=AppRepoFnLabel.AttrLabelUpdate, reason=respbody)
        _logger.debug("ElasticSearchAttrLabelRepo.update done successfully")

    async def delete(self, ids: List[str]):
        if not ids:
            reason = {"detail": "input-empty"}
            raise AppRepoError(fn_label=AppRepoFnLabel.AttrLabelDelete, reason=reason)
        cls = type(self)
        url = "/%s/_bulk" % (self._index_name)

        def convert_to_doc(id_: str) -> str:
            metadata = {"delete": {"_id": id_}}
            return cls.serial_nd_json(metadata)

        req_ops = list(map(convert_to_doc, ids))
        resp = await self._session.post(url, data="".join(req_ops))
        async with resp:
            if resp.status >= 400:
                reason = await resp.json()
                raise AppRepoError(fn_label=AppRepoFnLabel.AttrLabelDelete, reason=reason)
        _logger.debug("ElasticSearchAttrLabelRepo.delete done successfully")

    async def search(self, keyword: str) -> List[AttrLabelModel]:
        if not keyword:
            reason = {"detail": "input-empty"}
            raise AppRepoError(fn_label=AppRepoFnLabel.AttrLabelSearch, reason=reason)
        fields_present = ["hits.total", "hits.hits._id", "hits.hits._source", "_shards"]
        url = "/%s/_search?filter_path=%s" % (
            self._index_name,
            ",".join(fields_present),
        )
        reqbody = {
            "_source": True,
            "query": {"match": {"name.as_english": keyword}},
            # "size": 10 # TODO, limit range, number of docs fetched
        }
        headers = {"content-type": "application/json"}
        resp = await self._session.request("GET", url, json=reqbody, headers=headers)
        async with resp:
            respbody = await resp.json()
        if resp.status >= 400:
            raise AppRepoError(fn_label=AppRepoFnLabel.AttrLabelSearch, reason=respbody)

        nodes_involved = respbody["_shards"]["total"]
        nodes_replied = respbody["_shards"]["successful"]
        nodes_failed = respbody["_shards"]["failed"]
        if nodes_failed > 0:
            _logger.warning("nodes_failed:%d", nodes_failed)
        if nodes_involved > nodes_replied:
            _logger.warning("nodes_involved:%d, nodes_replied:%d", nodes_involved, nodes_replied)
            if nodes_replied == 0:
                raise AppRepoError(
                    fn_label=AppRepoFnLabel.AttrLabelSearch, reason=respbody["_shards"]
                )
        if respbody["hits"]["total"]["value"] > 1000:
            _logger.warning("hits-total:%s", str(respbody["hits"]["total"]))
        cls = type(self)
        ms = list(map(cls.convert_from_doc, respbody["hits"].get("hits", [])))
        _logger.debug("ElasticSearchAttrLabelRepo.search done successfully")
        return ms

    async def fetch_by_ids(self, ids: List[str]) -> List[AttrLabelModel]:
        if not any(ids):
            reason = {"detail": "input-empty"}
            raise AppRepoError(fn_label=AppRepoFnLabel.AttrLabelFetchByID, reason=reason)
        fields_present = ["docs._id", "docs._source", "docs.found"]
        url = "/%s/_mget?filter_path=%s" % (
            self._index_name,
            ",".join(fields_present),
        )
        reqbody = {"docs": [{"_id": _id} for _id in ids]}
        headers = {"content-type": "application/json"}
        resp = await self._session.request("GET", url, json=reqbody, headers=headers)
        async with resp:
            respbody = await resp.json()
        if resp.status >= 400:
            raise AppRepoError(fn_label=AppRepoFnLabel.AttrLabelFetchByID, reason=respbody)
        cls = type(self)
        # if len(respbody["docs"]) > 0 and not respbody["docs"][0].get("_source"):
        #     pass
        iter0 = filter(lambda r: r["found"], respbody["docs"])
        ms = list(map(cls.convert_from_doc, iter0))
        _logger.debug("ElasticSearchAttrLabelRepo.fetch_by_ids done successfully")
        return ms

    @staticmethod
    def convert_from_doc(d: Dict) -> AttrLabelModel:
        src = d["_source"]
        name = src["name"]
        dtype = AttrDataTypeDto(src["dtype"])
        return AttrLabelModel(id_=d["_id"], name=name, dtype=dtype)
