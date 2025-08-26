import logging
from collections import defaultdict
from typing import List, Dict, Optional

from blacksheep import FromJSON, Response
from blacksheep.exceptions import NotFound
from blacksheep.server.authorization import auth
from blacksheep.server.controllers import APIController
from blacksheep.server.responses import (
    created,
    ok,
    no_content,
    not_found,
    forbidden,
    bad_request,
)
from guardpost import User as AuthUser

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
    AppRepoError,
    AbstractTagRepo,
    AbstractAttrLabelRepo,
    AbstractSaleItemRepo,
)
from product.util import (
    PriviledgeLevel,
    QuotaMaterial,
    QuotaMaterialCode,
    permission_check,
)

from . import router
from ..dto import SaleItemCreateReqDto, SaleItemUpdateReqDto, SaleItemAttriReqDto

_logger = logging.getLogger(__name__)


class SaleItemController(APIController):
    @staticmethod
    async def quota_check(
        repo: AbstractSaleItemRepo, claims: Dict, reqbody: SaleItemCreateReqDto
    ) -> Optional[Dict]:
        quotas: List[QuotaMaterial] = QuotaMaterial.extract(claims)
        num_allowed = QuotaMaterial.find_maxnum(quotas, QuotaMaterialCode.NumSaleItem)
        num_items_saved = await repo.num_items_created(usr_id=claims["profile"])
        if num_items_saved >= num_allowed:
            return {
                "mat_code": QuotaMaterialCode.NumSaleItem.value,
                "limit": num_allowed,
                "num_used": num_items_saved,
            }
        num_allowed = QuotaMaterial.find_maxnum(quotas, QuotaMaterialCode.NumAttributesPerItem)
        num_attris_req = len(reqbody.attributes)
        if num_attris_req > num_allowed:
            return {
                "mat_code": QuotaMaterialCode.NumAttributesPerItem.value,
                "limit": num_allowed,
                "num_used": num_attris_req,
            }

    @staticmethod
    async def load_tags(shr_ctx: SharedContext, tag_ids: List[str]) -> Dict[str, List[TagModel]]:
        out: Dict[str, List[TagModel]] = {}
        ids_not_found: Dict[str, List[int]] = {}
        id_decomposed = defaultdict(list)
        for tag_id in tag_ids:
            (tree_id, node_id) = TagModel.decode_req_id(tag_id)
            id_decomposed[tree_id].append(node_id)
        repo: AbstractTagRepo = shr_ctx.datastore.tag
        for tree_id, node_ids in id_decomposed.items():
            try:
                tree = await repo.fetch_tree(tree_id)
                (tag_ms, not_found) = tree.find_nodes(node_ids)
            except AppRepoError as e:
                _logger.info("%s", str(e))
                (tag_ms, not_found) = ([], node_ids)
            if any(not_found):
                ids_not_found[tree_id] = not_found
            else:
                tag_acs_ms = tree.find_ancestors_bulk(tag_ms)
                tag_acs_ms.extend(tag_ms)
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
        try:
            labels_found: List[AttrLabelModel] = await repo.fetch_by_ids(req_ids)
        except AppRepoError as e:
            _logger.info("%s", str(e))
            labels_found: List[AttrLabelModel] = []
        return SaleItemAttriModel.from_req(labels_found, attri_d)

    @auth(PriviledgeLevel.AuthedUser.value)
    @router.post("/item")
    async def create(
        self,
        shr_ctx: SharedContext,
        reqbody: FromJSON[SaleItemCreateReqDto],
        authed_user: AuthUser,
    ) -> Response:
        assert self is None
        perm_err = permission_check(authed_user.claims, ["add_saleableitem"])
        if perm_err:
            return forbidden(message=perm_err)
        reqbody = reqbody.value
        repo: AbstractSaleItemRepo = shr_ctx.datastore.saleable_item
        perm_err = await SaleItemController.quota_check(repo, authed_user.claims, reqbody)
        if perm_err:
            return forbidden(message=perm_err)
        usr_prof_id: int = authed_user.claims.get("profile", -1)
        try:
            tag_ms_map = await SaleItemController.load_tags(shr_ctx, reqbody.tags)
            attri_val_ms = await SaleItemController.resolve_attributes(shr_ctx, reqbody.attributes)
        except (TagErrorModel, AttriLabelError) as e:
            return bad_request(message=e.detail)
        item_m = SaleableItemModel.from_req(
            reqbody,
            tag_ms_map=tag_ms_map,
            attri_val_ms=attri_val_ms,
            usr_prof=usr_prof_id,
        )
        await repo.create(item_m)
        item_d = item_m.to_dto()
        return created(message=item_d.model_dump())

    @auth(PriviledgeLevel.AuthedUser.value)
    @router.put("/item/{item_id}")
    async def modify(
        self,
        shr_ctx: SharedContext,
        item_id: int,
        reqbody: FromJSON[SaleItemUpdateReqDto],
        authed_user: AuthUser,
    ) -> Response:
        assert self is None
        perm_err = permission_check(authed_user.claims, ["change_saleableitem"])
        if perm_err:
            return forbidden(message=perm_err)
        usr_prof_id: int = authed_user.claims.get("profile", -1)
        reqbody = reqbody.value
        try:
            tag_ms_map = await SaleItemController.load_tags(shr_ctx, reqbody.tags)
            attri_val_ms = await SaleItemController.resolve_attributes(shr_ctx, reqbody.attributes)
        except (TagErrorModel, AttriLabelError) as e:
            return bad_request(message=e.detail)
        item_m = SaleableItemModel.from_req(
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

    @staticmethod
    async def validate_maintainer(
        repo: AbstractSaleItemRepo, item_id: int, authed_user: AuthUser
    ) -> bool:
        usr_prof_id: int = authed_user.claims.get("profile", -1)
        try:
            maintainer_prof_id: int = await repo.get_maintainer(item_id)
        except AppRepoError as e:
            db_exist = e.reason.get("remote_database_done", False)
            data_found = e.reason.get("found", False)
            if db_exist and data_found:
                raise NotFound()
            else:
                raise e
        return usr_prof_id == maintainer_prof_id

    @auth(PriviledgeLevel.AuthedUser.value)
    @router.delete("/item/{item_id}")
    async def delete(self, shr_ctx: SharedContext, item_id: int, authed_user: AuthUser) -> Response:
        perm_err = permission_check(authed_user.claims, ["delete_saleableitem"])
        if perm_err:
            return forbidden(message=perm_err)

        repo: AbstractSaleItemRepo = shr_ctx.datastore.saleable_item
        match = await SaleItemController.validate_maintainer(repo, item_id, authed_user)
        if match:
            await repo.delete(item_id)
            return no_content()
        else:
            return forbidden()

    @staticmethod
    async def get_by_id_common(
        repo: AbstractSaleItemRepo, item_id: int, visible_only: bool
    ) -> Response:
        try:
            item_m: SaleableItemModel = await repo.fetch(item_id, visible_only=visible_only)
            item_d = item_m.to_dto()
            return ok(message=item_d.model_dump())
        except AppRepoError as e:
            db_exist = e.reason.get("remote_database_done", False)
            data_found = e.reason.get("found", False)
            if db_exist and not data_found:
                return not_found(message=None)
            else:
                raise e

    # TODO,
    # - optional specific time, to query historical data for existing orders
    @router.get("/item/{item_id}")
    async def get_by_id_unauth(self, shr_ctx: SharedContext, item_id: int) -> Response:
        assert self is None
        return await SaleItemController.get_by_id_common(
            shr_ctx.datastore.saleable_item, item_id, visible_only=True
        )

    @router.get("/item/{item_id}/private")
    async def get_by_id_privileged(
        self, shr_ctx: SharedContext, item_id: int, authed_user: AuthUser
    ) -> Response:
        assert self is None
        repo: AbstractSaleItemRepo = shr_ctx.datastore.saleable_item
        match = await SaleItemController.validate_maintainer(repo, item_id, authed_user)
        if match:
            return await SaleItemController.get_by_id_common(repo, item_id, visible_only=False)
        else:
            return forbidden()

    @router.get("/items/search")
    async def search_unauth(self, shr_ctx: SharedContext, k: str) -> Response:
        repo: AbstractSaleItemRepo = shr_ctx.datastore.saleable_item
        keywords: List[str] = k.rsplit()
        ms: List[SaleableItemModel] = await repo.search(keywords, visible_only=True)
        items_d = [m.to_dto() for m in ms]
        return ok(message=items_d.model_dump())

    @router.get("/items/search/private")
    async def search_priviledged(
        self, shr_ctx: SharedContext, k: str, authed_user: AuthUser
    ) -> Response:
        usr_prof_id: int = authed_user.claims.get("profile", -1)
        repo: AbstractSaleItemRepo = shr_ctx.datastore.saleable_item
        keywords: List[str] = k.rsplit()
        ms: List[SaleableItemModel] = await repo.search(keywords, usr_id=usr_prof_id)
        items_d = [m.to_dto() for m in ms]
        return ok(message=items_d.model_dump())
