import logging
import json
from typing import Tuple, Dict, List, Self
from asyncio.events import AbstractEventLoop

from aiohttp import TCPConnector, ClientSession

from ecommerce_common.util import flatten_nested_iterable
from product.model import AttrLabelModel
from .. import AbstractAttrLabelRepo, AppRepoError, AppRepoFnLabel

_logger = logging.getLogger(__name__)


class ElasticSearchAttrLabelRepo(AbstractAttrLabelRepo):
    def __init__(
        self,
        secure_conn_enable: bool,
        cfdntl: Dict,
        index_name: str,
        connector: TCPConnector,
    ):
        proto = "https" if secure_conn_enable else "http"
        domain_host = "%s://%s:%d" % (proto, cfdntl["HOST"], cfdntl["PORT"])
        headers = {"content-type": "application/x-ndjson", "accept": "application/json"}
        self._session = ClientSession(
            base_url=domain_host, headers=headers, connector=connector
        )
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
        url = "/%s/the-only-type/_bulk" % (self._index_name)

        def convert_to_doc(m: AttrLabelModel) -> Tuple[str, str]:
            metadata = {"create": {"_id": m.id_}}
            source = {"name": m.name, "dtype": m.dtype.value}
            return (
                cls.serial_nd_json(metadata),
                cls.serial_nd_json(source),
            )

        req_ops_gen = flatten_nested_iterable(map(convert_to_doc, ms))
        req_ops = list(req_ops_gen)
        resp = await self._session.post(url, data="".join(req_ops))
        async with resp:
            if resp.status >= 400:
                import pdb

                pdb.set_trace()
                reason = await resp.json()
                raise AppRepoError(
                    fn_label=AppRepoFnLabel.AttrLabelCreate, reason=reason
                )
        _logger.debug("ElasticSearchAttrLabelRepo.create done successfully")

    async def update(self, ms: List[AttrLabelModel]):
        if not ms:
            reason = {"detail": "input-empty"}
            raise AppRepoError(fn_label=AppRepoFnLabel.AttrLabelUpdate, reason=reason)
        cls = type(self)
        url = "/%s/the-only-type/_bulk" % (self._index_name)

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
            if resp.status >= 400:
                reason = await resp.json()
                raise AppRepoError(
                    fn_label=AppRepoFnLabel.AttrLabelUpdate, reason=reason
                )
        _logger.debug("ElasticSearchAttrLabelRepo.update done successfully")

    async def delete(self, ids: List[str]):
        if not ids:
            reason = {"detail": "input-empty"}
            raise AppRepoError(fn_label=AppRepoFnLabel.AttrLabelDelete, reason=reason)
        cls = type(self)
        url = "/%s/the-only-type/_bulk" % (self._index_name)

        def convert_to_doc(id_: str) -> str:
            metadata = {"delete": {"_id": id_}}
            return cls.serial_nd_json(metadata)

        req_ops = list(map(convert_to_doc, ids))
        resp = await self._session.post(url, data="".join(req_ops))
        async with resp:
            if resp.status >= 400:
                reason = await resp.json()
                raise AppRepoError(
                    fn_label=AppRepoFnLabel.AttrLabelDelete, reason=reason
                )
        _logger.debug("ElasticSearchAttrLabelRepo.delete done successfully")

    async def search(self, keyword: str) -> List[AttrLabelModel]:
        if not keyword:
            reason = {"detail": "input-empty"}
            raise AppRepoError(fn_label=AppRepoFnLabel.AttrLabelSearch, reason=reason)
        _logger.debug("ElasticSearchAttrLabelRepo.search done successfully")
        return []
