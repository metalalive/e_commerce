import logging
from copy import copy
from datetime import datetime, UTC
from collections import defaultdict
from typing import Any, Dict, Self, Tuple
from asyncio.events import AbstractEventLoop

from aiohttp import TCPConnector, ClientSession
from pydantic import NonNegativeInt, ValidationError

from product.api.dto import AttrDataTypeDto
from product.model import (
    TagModel,
    AttrLabelModel,
    SaleableItemModel,
    SaleItemAttriModel,
)
from .. import AbstractSaleItemRepo, AppRepoError, AppRepoFnLabel

_logger = logging.getLogger(__name__)


class ElasticSearchSaleItemRepo(AbstractSaleItemRepo):
    def __init__(self, setting: Dict, loop: AbstractEventLoop):
        cfdntl = setting["cfdntl"]

        self._index_primary_name = setting["db_names"]["latest"]
        self._index_snapshot_pattern = setting["db_names"]["history"]

        secure_conn_enable = setting.get("ssl_enable", True)
        connector = TCPConnector(
            limit=setting.get("num_conns", 10),
            keepalive_timeout=setting.get("timeout_secs", 35),
            ssl=secure_conn_enable,
            loop=loop,
        )
        proto = "https" if secure_conn_enable else "http"
        domain_host = "%s://%s:%d" % (proto, cfdntl["HOST"], cfdntl["PORT"])
        headers = {"content-type": "application/json", "accept": "application/json"}
        self._session = ClientSession(
            base_url=domain_host, headers=headers, connector=connector
        )
        _logger.debug("ElasticSearchSaleItemRepo.init done successfully")

    async def init(setting: Dict, loop: AbstractEventLoop) -> Self:
        return ElasticSearchSaleItemRepo(setting, loop)

    async def deinit(self):
        await self._session.close()
        _logger.debug("ElasticSearchSaleItemRepo.deinit done successfully")

    @staticmethod
    def convert_to_primary_doc(item_m: SaleableItemModel) -> Dict:
        def convert_attribute(attr: SaleItemAttriModel) -> Dict[str, Any]:
            label = {
                "id_": attr.label.id_,
                "name": attr.label.name,
                "dtype": attr.label.dtype.value,
            }
            value = {}
            if isinstance(attr.value, bool):
                value["boolean_value"] = attr.value
            elif isinstance(attr.value, int):
                value["integer_value"] = attr.value
            elif isinstance(attr.value, str):
                value["string_value"] = attr.value
            return {"label": label, "value": value}

        tags = [
            {"tree_id": tree_id, "node_id": node._id, "label": node._label}
            for tree_id, nodes in item_m.tags.items()
            for node in nodes
        ]
        out = {
            "usr_prof": item_m.usr_prof,
            "name": item_m.name,
            "visible": item_m.visible,
            "tags": tags,
            "attributes": list(map(convert_attribute, item_m.attributes)),
            "media_set": item_m.media_set,
        }
        return out

    async def save_primary(
        self, item_m: SaleableItemModel, force_create: bool
    ) -> Tuple[int, Dict]:
        cls = type(self)
        op_type = "create" if force_create else "index"
        url = "/%s/the-only-type/%d/?op_type=%s" % (
            self._index_primary_name,
            item_m.id_,
            op_type,
        )
        reqbody = cls.convert_to_primary_doc(item_m)
        resp = await self._session.put(url, json=reqbody)
        async with resp:
            respbody = await resp.json()
        return (resp.status, respbody)

    async def create(self, item_m: SaleableItemModel):
        num_retry = 5
        for idx in range(num_retry):
            status_code, respbody = await self.save_primary(item_m, force_create=True)
            if status_code >= 400:
                if status_code == 409:
                    item_m.rotate_id()
                else:
                    raise AppRepoError(
                        fn_label=AppRepoFnLabel.SaleItemCreate, reason=respbody
                    )
            else:
                break
        if idx == num_retry:
            reason = {"detail": "id-conflict", "num_retry": num_retry}
            raise AppRepoError(fn_label=AppRepoFnLabel.SaleItemCreate, reason=reason)
        _logger.debug("ElasticSearchSaleItemRepo.create done successfully")

    def snapshot_index_name(self, t: datetime) -> str:
        return self._index_snapshot_pattern % t.strftime("%Y")

    def snapshot_id(self, t: datetime, id_: int) -> str:
        return "%s-%d" % (t.strftime("%m%d%H%M%S"), id_)

    @staticmethod
    def rawdoc_primary_to_snapshot(raw: Dict) -> Dict:
        def cvt_attri(attr: Dict[str, Any]) -> SaleItemAttriModel:
            attrval = attr["value"]
            if "boolean_value" in attrval:
                value = attrval["boolean_value"]
            elif "integer_value" in attrval:
                value = attrval["integer_value"]
            elif "string_value" in attrval:
                value = attrval["string_value"]
            else:
                detail = copy(attr["label"])
                detail.update({"actual_value": value, "detail": "unknown-data-type"})
                raise ValueError(detail)
            attr["value"] = str(value)

        list(map(cvt_attri, raw["attributes"]))

    async def do_archive(self, saleitem_id: int):
        # TODO, get last-update timestamp from SaleableItemModel
        snapshot_ts = datetime.now(UTC)
        olditem_rawdoc = await self.fetch_doc_primary(
            id_=saleitem_id, fn_label=AppRepoFnLabel.SaleItemArchiveUpdate
        )
        cls = type(self)
        cls.rawdoc_primary_to_snapshot(olditem_rawdoc)
        idx_name = self.snapshot_index_name(snapshot_ts)
        snapshot_id = self.snapshot_id(snapshot_ts, saleitem_id)
        url = "/%s/the-only-type/%s" % (idx_name, snapshot_id)
        resp = await self._session.put(url, json=olditem_rawdoc)
        _logger.debug("index:%s, id:%s, resp:%d", idx_name, snapshot_id, resp.status)
        async with resp:
            respbody = await resp.json()
            if resp.status >= 400:
                raise AppRepoError(
                    fn_label=AppRepoFnLabel.SaleItemArchiveUpdate, reason=respbody
                )

    async def archive_and_update(self, item_m: SaleableItemModel):
        await self.do_archive(item_m.id_)
        status_code, respbody = await self.save_primary(item_m, force_create=False)
        if status_code >= 400:
            raise AppRepoError(
                fn_label=AppRepoFnLabel.SaleItemArchiveUpdate, reason=respbody
            )
        _logger.debug("ElasticSearchSaleItemRepo.archive_and_update done successfully")

    async def delete(self, id_: int):
        await self.do_archive(id_)
        url = "/%s/the-only-type/%d" % (self._index_primary_name, id_)
        resp = await self._session.delete(url)
        async with resp:
            respbody = await resp.json()
            if resp.status >= 400:
                raise AppRepoError(
                    fn_label=AppRepoFnLabel.SaleItemDelete, reason=respbody
                )
        _logger.debug("ElasticSearchSaleItemRepo.delete done successfully")

    async def fetch_doc_primary(self, id_: int, fn_label: AppRepoFnLabel) -> Dict:
        url = "/%s/the-only-type/%d" % (self._index_primary_name, id_)
        resp = await self._session.get(url)
        async with resp:
            respbody = await resp.json()
            if resp.status != 200:
                respbody["remote_database_done"] = True
                raise AppRepoError(fn_label=fn_label, reason=respbody)
        return respbody["_source"]

    @staticmethod
    def convert_from_primary_doc(id_: int, raw: Dict) -> SaleableItemModel:
        def cvt_attri(attr: Dict[str, Any]) -> SaleItemAttriModel:
            label = AttrLabelModel(
                id_=attr["label"]["id_"],
                name=attr["label"]["name"],
                dtype=AttrDataTypeDto(attr["label"]["dtype"]),
            )
            if "boolean_value" in attr["value"]:
                value = attr["value"]["boolean_value"]
            elif "integer_value" in attr["value"]:
                value = attr["value"]["integer_value"]
                if label.dtype == AttrDataTypeDto.UnsignedInteger:
                    value = NonNegativeInt(value)
            elif "string_value" in attr["value"]:
                value = attr["value"]["string_value"]
            else:
                detail = {
                    "label.id": label.id_,
                    "label.dtype": attr["label"]["dtype"],
                    "actual_value": value,
                    "detail": "unknown-data-type",
                }
                raise ValueError(detail)
            return SaleItemAttriModel(label=label, value=value)

        tag_map = defaultdict(list)
        for t in raw["tags"]:
            m = TagModel(_id=t["node_id"], _label=t["label"])
            tag_map[t["tree_id"]].append(m)
        return SaleableItemModel(
            id_=id_,
            usr_prof=raw["usr_prof"],
            name=raw["name"],
            visible=raw["visible"],
            tags=tag_map,
            attributes=list(map(cvt_attri, raw["attributes"])),
            media_set=raw["media_set"],
        )

    async def fetch(self, id_: int) -> SaleableItemModel:
        cls = type(self)
        # TODO, optional timestamp to retrieve snapshot
        rawdoc = await self.fetch_doc_primary(
            id_=id_, fn_label=AppRepoFnLabel.SaleItemFetchModel
        )
        try:
            obj = cls.convert_from_primary_doc(id_, rawdoc)
        except (ValueError, ValidationError) as e:
            _logger.error("corruption detail: %s" % str(e))
            reason = {"detail": "data-corruption"}
            raise AppRepoError(
                fn_label=AppRepoFnLabel.SaleItemFetchModel, reason=reason
            )
        _logger.debug("ElasticSearchSaleItemRepo.fetch done successfully")
        return obj

    async def get_maintainer(self, id_: int) -> int:
        fields_present = ["_source.usr_prof", "_id"]
        url = "/%s/the-only-type/%d?filter_path=%s" % (
            self._index_primary_name,
            id_,
            ",".join(fields_present),
        )
        resp = await self._session.get(url)
        async with resp:
            respbody = await resp.json()
            if resp.status != 200:
                respbody["remote_database_done"] = True
                raise AppRepoError(
                    fn_label=AppRepoFnLabel.SaleItemGetMaintainer, reason=respbody
                )
        _logger.debug("ElasticSearchSaleItemRepo.get_maintainer done successfully")
        return respbody["_source"]["usr_prof"]
