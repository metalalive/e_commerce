from collections import defaultdict
from typing import Optional, List, Dict

from blacksheep import FromJSON, Response
from blacksheep.server.controllers import APIController
from blacksheep.server.responses import created, ok, no_content, forbidden

from product.model import (
    TagModel,
    TagErrorModel,
    AttrLabelModel,
    AttriLabelError,
    SaleItemAttriModel,
    SaleableItemModel,
)
from product.shared import SharedContext
from product.adapter.repository import (
    AbstractTagRepo,
    AbstractAttrLabelRepo,
    AbstractSaleItemRepo,
)

from . import router
from ..dto import SaleItemCreateReqDto, SaleItemUpdateReqDto, SaleItemAttriReqDto


class ProdItemController(APIController):
    @staticmethod
    async def load_tags(
        shr_ctx: SharedContext, tag_ids: List[str]
    ) -> Dict[str, List[TagModel]]:
        out: Dict[str, List[TagModel]] = {}
        ids_not_found: Dict[str, List[int]] = {}
        id_decomposed = defaultdict(list)
        for tag_id in tag_ids:
            (tree_id, node_id) = TagModel.decode_req_id(tag_id)
            id_decomposed[tree_id].append(node_id)
        repo: AbstractTagRepo = shr_ctx.datastore.tag
        for tree_id, node_ids in id_decomposed.items():
            tree = await repo.fetch_tree(tree_id)
            (tag_ms, not_found) = tree.find_nodes(node_ids)
            if any(not_found):
                ids_not_found[tree_id] = not_found
            else:
                tag_acs_ms = tree.find_ancestors_many(tag_ms)
                out[tree_id] = tag_acs_ms
        if any(not_found):
            raise TagErrorModel.invalid_node_ids(ids_not_found)
        return out

    @staticmethod
    async def resolve_attributes(
        shr_ctx: SharedContext, attri_d: List[SaleItemAttriReqDto]
    ) -> List[SaleItemAttriModel]:
        repo: AbstractAttrLabelRepo = shr_ctx.datastore.prod_attri
        req_ids: List[str] = [a.id_ for a in attri_d]
        labels_found: List[AttrLabelModel] = await repo.fetch_by_ids(req_ids)
        return SaleItemAttriModel.from_parts(labels_found, attri_d)

    @router.post("/item")
    async def create(
        self, shr_ctx: SharedContext, reqbody: FromJSON[SaleItemCreateReqDto]
    ) -> Response:
        cls = type(self)
        usr_prof_id: int = 123  # TODO: authorization
        reqbody = reqbody.value
        try:
            tag_ms_map = await cls.load_tags(shr_ctx, reqbody.tags)
            attri_val_ms = await cls.resolve_attributes(shr_ctx, reqbody.attributes)
        except (TagErrorModel, AttriLabelError) as e:
            return bad_request(message=e.detail)
        item_m = SaleableItemModel.from_parts(
            reqbody,
            tag_ms_map=tag_ms_map,
            attri_val_ms=attri_val_ms,
            usr_prof=usr_prof_id,
        )
        repo: AbstractSaleItemRepo = shr_ctx.datastore.saleable_item
        await repo.create(item_m)
        item_d = item_m.to_dto()
        return created(message=item_d.model_dump())

    @router.put("/item/{item_id}")
    async def modify(
        self,
        shr_ctx: SharedContext,
        item_id: int,
        reqbody: FromJSON[SaleItemUpdateReqDto],
    ) -> Response:
        cls = type(self)
        usr_prof_id: int = 123  # TODO: authorization
        reqbody = reqbody.value
        try:
            tag_ms_map = await cls.load_tags(shr_ctx, reqbody.tags)
            attri_val_ms = await cls.resolve_attributes(shr_ctx, reqbody.attributes)
        except (TagErrorModel, AttriLabelError) as e:
            return bad_request(message=e.detail)
        item_m = SaleableItemModel.from_parts(
            reqbody,
            tag_ms_map=tag_ms_map,
            attri_val_ms=attri_val_ms,
            usr_prof=usr_prof_id,
            id_=item_id,
        )
        repo: AbstractSaleItemRepo = shr_ctx.datastore.saleable_item
        await repo.archive_and_update(item_m)
        item_d = item_m.to_dto()
        return ok(message=item_d.model_dump())

    @router.delete("/item/{item_id}")
    async def delete(self, shr_ctx: SharedContext, item_id: int) -> Response:
        usr_prof_id: int = 123  # TODO: authorization
        repo: AbstractSaleItemRepo = shr_ctx.datastore.saleable_item
        maintainer_prof_id: int = await repo.get_maintainer(item_id)
        if usr_prof_id == maintainer_prof_id:
            await repo.delete(item_id)
            return no_content()
        else:
            return forbidden()

    @router.get("/item/{item_id}")
    async def get_by_id(self, shr_ctx: SharedContext, item_id: int) -> Response:
        # TODO, optional specific time, to query historical data for existing orders
        repo: AbstractSaleItemRepo = shr_ctx.datastore.saleable_item
        item_m: Optional[SaleableItemModel] = await repo.fetch(item_id)
        item_d = item_m.to_dto()
        return ok(message=item_d.model_dump())
