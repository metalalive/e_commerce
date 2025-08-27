import logging
from copy import copy
from datetime import datetime, UTC
from collections import defaultdict
from typing import Any, Dict, Self, Tuple, Optional, List
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
        # TODO , use credential from setting["cfdntl"] once XPACK plugin is available
        address = setting["db_addr"]

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
        domain_host = "%s://%s:%d" % (proto, address["HOST"], address["PORT"])
        headers = {"content-type": "application/json", "accept": "application/json"}
        self._session = ClientSession(base_url=domain_host, headers=headers, connector=connector)
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
        t0 = item_m.last_update.astimezone(UTC)
        out = {
            "usr_prof": item_m.usr_prof,
            "name": item_m.name,
            "visible": item_m.visible,
            "tags": tags,
            "attributes": list(map(convert_attribute, item_m.attributes)),
            "media_set": item_m.media_set,
            "last_update": t0.strftime(SaleableItemModel.STRING_DATETIME_FORMAT()),
        }
        return out

    async def save_primary(self, item_m: SaleableItemModel, force_create: bool) -> Tuple[int, Dict]:
        cls = type(self)
        op_type = "create" if force_create else "index"
        url = "/%s/_doc/%d/?op_type=%s" % (self._index_primary_name, item_m.id_, op_type)
        reqbody = cls.convert_to_primary_doc(item_m)
        resp = await self._session.post(url, json=reqbody)
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
                    raise AppRepoError(fn_label=AppRepoFnLabel.SaleItemCreate, reason=respbody)
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
        olditem_rawdoc = await self.fetch_doc_primary_single_id(
            id_=saleitem_id, fn_label=AppRepoFnLabel.SaleItemArchiveUpdate
        )
        snapshot_ts: str = olditem_rawdoc.pop("last_update")
        assert snapshot_ts
        snapshot_ts: datetime = datetime.strptime(
            snapshot_ts, SaleableItemModel.STRING_DATETIME_FORMAT()
        ).replace(tzinfo=UTC)
        cls = type(self)
        cls.rawdoc_primary_to_snapshot(olditem_rawdoc)
        idx_name = self.snapshot_index_name(snapshot_ts)
        snapshot_id = self.snapshot_id(snapshot_ts, saleitem_id)
        url = "/%s/_doc/%s" % (idx_name, snapshot_id)
        resp = await self._session.post(url, json=olditem_rawdoc)
        _logger.debug("index:%s, id:%s, resp:%d", idx_name, snapshot_id, resp.status)
        async with resp:
            respbody = await resp.json()
            if resp.status >= 400:
                raise AppRepoError(fn_label=AppRepoFnLabel.SaleItemArchiveUpdate, reason=respbody)

    async def archive_and_update(self, item_m: SaleableItemModel):
        await self.do_archive(item_m.id_)
        status_code, respbody = await self.save_primary(item_m, force_create=False)
        if status_code >= 400:
            raise AppRepoError(fn_label=AppRepoFnLabel.SaleItemArchiveUpdate, reason=respbody)
        _logger.debug("ElasticSearchSaleItemRepo.archive_and_update done successfully")

    async def delete(self, id_: int):
        await self.do_archive(id_)
        url = "/%s/_doc/%d" % (self._index_primary_name, id_)
        resp = await self._session.delete(url)
        async with resp:
            respbody = await resp.json()
            if resp.status >= 400:
                raise AppRepoError(fn_label=AppRepoFnLabel.SaleItemDelete, reason=respbody)
        _logger.debug("ElasticSearchSaleItemRepo.delete done successfully")

    async def fetch_doc_primary_single_id(self, id_: int, fn_label: AppRepoFnLabel) -> Dict:
        url = "/%s/_doc/%d" % (self._index_primary_name, id_)
        resp = await self._session.get(url)
        async with resp:
            respbody = await resp.json()
            if resp.status != 200:
                respbody["remote_database_done"] = True
                raise AppRepoError(fn_label=fn_label, reason=respbody)
        return respbody["_source"]

    async def fetch_doc_primary_multi_id(
        self,
        ids: List[int],
        visible: Optional[bool],
        fn_label: AppRepoFnLabel,
        usrprof: Optional[int] = None,
    ) -> List[Tuple[int, Dict]]:
        fields_present = ["hits.total", "hits.hits._id", "hits.hits._source", "_shards"]
        url = "/%s/_search?filter_path=%s" % (
            self._index_primary_name,
            ",".join(fields_present),
        )
        reqbody = {"query": {"bool": {"filter": [{"ids": {"values": ids}}]}}}
        if visible:
            term = {"term": {"visible": True}}
            reqbody["query"]["bool"]["filter"].append(term)
        if usrprof is not None:
            term = {"term": {"usr_prof": usrprof}}
            reqbody["query"]["bool"]["filter"].append(term)

        headers = {"content-type": "application/json"}
        resp = await self._session.request("GET", url, json=reqbody, headers=headers)
        async with resp:
            respbody = await resp.json()
            if resp.status != 200:
                respbody["remote_database_done"] = True
                raise AppRepoError(fn_label=fn_label, reason=respbody)
        fetched_docs = respbody["hits"].get("hits", [])
        return [(int(r["_id"]), r["_source"]) for r in fetched_docs]

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

        last_update: datetime = datetime.strptime(
            raw["last_update"], SaleableItemModel.STRING_DATETIME_FORMAT()
        ).replace(tzinfo=UTC)

        return SaleableItemModel(
            id_=id_,
            usr_prof=raw["usr_prof"],
            name=raw["name"],
            visible=raw["visible"],
            tags=tag_map,
            attributes=list(map(cvt_attri, raw["attributes"])),
            media_set=raw["media_set"],
            last_update=last_update,
        )

    async def fetch(self, id_: int, visible_only: Optional[bool] = None) -> SaleableItemModel:
        cls = type(self)
        if visible_only:
            rawdocs = await self.fetch_doc_primary_multi_id(
                ids=[id_],
                visible=True,
                fn_label=AppRepoFnLabel.SaleItemFetchOneModel,
            )
            if len(rawdocs) != 1:
                reason = {
                    "remote_database_done": True,
                    "num_docs_fetched": len(rawdocs),
                }
                raise AppRepoError(fn_label=AppRepoFnLabel.SaleItemFetchOneModel, reason=reason)
            fetched_id, rawdoc = rawdocs[0]
            assert fetched_id == id_
        else:
            rawdoc = await self.fetch_doc_primary_single_id(
                id_=id_,
                fn_label=AppRepoFnLabel.SaleItemFetchOneModel,
            )
        try:
            obj = cls.convert_from_primary_doc(id_, rawdoc)
        except (ValueError, ValidationError) as e:
            _logger.error("corruption detail: %s" % str(e))
            reason = {"detail": "data-corruption"}
            raise AppRepoError(fn_label=AppRepoFnLabel.SaleItemFetchOneModel, reason=reason)
        _logger.debug("ElasticSearchSaleItemRepo.fetch done successfully")
        return obj

    async def fetch_many(
        self,
        ids: List[int],
        usrprof: int,
        visible_only: Optional[bool] = None,
    ) -> List[SaleableItemModel]:
        cls = type(self)
        rawdocs = await self.fetch_doc_primary_multi_id(
            ids=ids,
            visible=visible_only,
            usrprof=usrprof,
            fn_label=AppRepoFnLabel.SaleItemFetchManyModel,
        )
        try:
            objs = [cls.convert_from_primary_doc(id_, doc) for id_, doc in rawdocs]
        except (ValueError, ValidationError) as e:
            _logger.error("corruption detail: %s" % str(e))
            reason = {"detail": "data-corruption"}
            raise AppRepoError(fn_label=AppRepoFnLabel.SaleItemFetchManyModel, reason=reason)
        _logger.debug("ElasticSearchSaleItemRepo.fetch_many done successfully")
        return objs

    async def get_maintainer(self, id_: int) -> int:
        fields_present = ["_source.usr_prof", "_id"]
        url = "/%s/_doc/%d?filter_path=%s" % (
            self._index_primary_name,
            id_,
            ",".join(fields_present),
        )
        resp = await self._session.get(url)
        async with resp:
            respbody = await resp.json()
            if resp.status != 200:
                respbody["remote_database_done"] = True
                raise AppRepoError(fn_label=AppRepoFnLabel.SaleItemGetMaintainer, reason=respbody)
        _logger.debug("ElasticSearchSaleItemRepo.get_maintainer done successfully")
        return respbody["_source"]["usr_prof"]

    async def num_items_created(self, usr_id: int) -> int:
        fields_present = ["hits.total"]
        url = "/%s/_search?filter_path=%s" % (self._index_primary_name, ",".join(fields_present))
        reqbody = {
            "_source": False,
            "size": 0,
            "query": {"match": {"usr_prof": usr_id}},
        }
        headers = {"content-type": "application/json"}
        resp = await self._session.request("GET", url, json=reqbody, headers=headers)
        async with resp:
            respbody = await resp.json()
            if resp.status != 200:
                respbody["remote_database_done"] = True
                raise AppRepoError(fn_label=AppRepoFnLabel.SaleItemNumCreated, reason=respbody)
        _logger.debug("ElasticSearchSaleItemRepo.num_items_created done successfully")
        return respbody["hits"]["total"]["value"]

    @staticmethod
    def base_search_query(keywords: List[str]) -> Dict:
        # fmt: off
        tags_clause = {"nested": {
            "path": "tags",
            "query": {"bool": {
                "should": [{"match": {"tags.label.as_english": k}} for k in keywords],
                "minimum_should_match": 1,
            }},
        }}
        item_name_clause = {"bool": {
            "should": [{"match": {"name.as_english": k}} for k in keywords],
            "minimum_should_match": 1,
        }}
        attris_clause = {"nested": {
            "path": "attributes",
            "query": {"bool": {
                "should": [
                    {"multi_match": {
                        "query": k,
                        "fields": [
                            "attributes.label.name.as_english",
                            "attributes.value.string_value.as_english",
                        ],
                        "operator": "or",
                    }}
                    for k in keywords
                ],
                "minimum_should_match": 1,
            }},
        }}
        return {
            "query": {
                "bool": {
                    "should": [tags_clause, item_name_clause, attris_clause],
                    "minimum_should_match": 1,
                }
            },
            "size": 30,  # TODO, pagination
        }
        # fmt: on

    async def search(
        self,
        keywords: List[str],
        visible_only: Optional[bool] = None,
        usr_id: Optional[int] = None,
    ) -> List[SaleableItemModel]:
        cls = type(self)
        # fmt: off
        fields_present = ["hits.total", "hits.hits._id", "hits.hits._source", "hits.hits._score", "_shards"]
        url = "/%s/_search?filter_path=%s" % (
            self._index_primary_name, ",".join(fields_present),
        )
        # fmt: on
        reqbody = cls.base_search_query(keywords)
        if visible_only:
            reqbody["query"]["bool"]["filter"] = [{"term": {"visible": True}}]
        elif usr_id:
            reqbody["query"]["bool"]["filter"] = [{"term": {"usr_prof": usr_id}}]
        headers = {"content-type": "application/json"}
        resp = await self._session.request("GET", url, json=reqbody, headers=headers)
        async with resp:
            respbody = await resp.json()
            if resp.status != 200:
                respbody["remote_database_done"] = True
                raise AppRepoError(fn_label=AppRepoFnLabel.SaleItemSearch, reason=respbody)
        try:
            result = [
                cls.convert_from_primary_doc(int(d["_id"]), d["_source"])
                for d in respbody["hits"]["hits"]
            ]
        except (ValueError, ValidationError) as e:
            _logger.error("corruption detail: %s" % str(e))
            reason = {"detail": "data-corruption"}
            raise AppRepoError(fn_label=AppRepoFnLabel.SaleItemSearch, reason=reason)
        _logger.debug("ElasticSearchSaleItemRepo.search")
        return result
