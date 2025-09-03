from typing import Optional
import logging

from blacksheep import FromJSON, Response
from blacksheep.server.responses import bad_request, not_found
from blacksheep.server.authorization import auth
from blacksheep.server.controllers import APIController
from blacksheep.server.responses import created, forbidden, ok, no_content
from guardpost import User as AuthUser

from product.model import TagModel, TagTreeModel, TagErrorModel, TagErrorReason
from product.shared import SharedContext
from product.util import PriviledgeLevel, permission_check

from . import router
from ..dto import TagCreateReqDto, TagUpdateReqDto, TagReadRespDto

_logger = logging.getLogger(__name__)


async def exception_handler(self, request, e: TagErrorModel) -> Response:
    if e.reason in (TagErrorReason.MissingTree, TagErrorReason.UnknownTree):
        return not_found(message=e.detail)
    elif e.reason == TagErrorReason.DecodeInvalidId:
        return bad_request(message=e.detail)
    _logger.error("Unhandled TagErrorModel, reason: %s, detail: %s", e.reason, e.detail)
    return Response(500, content=bytes("unexpected server error occurred.", "utf-8"))


class TagController(APIController):
    @auth(PriviledgeLevel.AuthedUser.value)
    @router.post("/tag")
    async def create(
        self,
        shr_ctx: SharedContext,
        reqbody: FromJSON[TagCreateReqDto],
        authed_user: AuthUser,
    ) -> Response:
        perm_err = permission_check(authed_user.claims, ["add_producttag"])
        if perm_err:
            return forbidden(message=perm_err)
        reqbody = reqbody.value
        repo = shr_ctx.datastore.tag
        newnode = TagModel.from_req(reqbody)
        if reqbody.parent:
            (tree_id, parent_node_id) = TagModel.decode_req_id(reqbody.parent)
            tree = await repo.fetch_tree(tree_id)
        else:
            parent_node_id = None
            tree_id = await repo.new_tree_id()
            tree = TagTreeModel(_id=tree_id)
        tree.try_insert(newnode, parent_node_id)
        await repo.save_tree(tree)
        tag_d = newnode.to_resp(tree_id, parent_node_id)
        return created(message=tag_d.model_dump())

    @auth(PriviledgeLevel.AuthedUser.value)
    @router.patch("/tag/{tag_id}")
    async def modify(
        self,
        shr_ctx: SharedContext,
        tag_id: str,
        authed_user: AuthUser,
        reqbody: FromJSON[TagUpdateReqDto],
    ) -> Response:
        perm_err = permission_check(authed_user.claims, ["change_producttag"])
        if perm_err:
            _logger.info("perm-err: %s", perm_err)
            return forbidden(message=perm_err)
        reqbody = reqbody.value
        repo = shr_ctx.datastore.tag
        (orig_tree_id, orig_node_id) = TagModel.decode_req_id(tag_id)
        # TODO, return 404 if not exists
        orig_tree = await repo.fetch_tree(orig_tree_id)
        if reqbody.parent:
            (dst_tree_id, dst_parent_node_id) = TagModel.decode_req_id(reqbody.parent)
            if orig_tree_id == dst_tree_id:
                dst_tree = orig_tree
            else:  # TODO, return 404 if not exists
                dst_tree = await repo.fetch_tree(dst_tree_id)
        else:
            dst_tree_id = await repo.new_tree_id()
            dst_parent_node_id = None
            dst_tree = TagTreeModel(_id=dst_tree_id)

        tag_m = orig_tree.try_remove(node_id=orig_node_id)
        tag_m.reset_limit_range()
        dst_tree.try_insert(tag_m, dst_parent_node_id)

        await repo.save_tree(dst_tree)
        if orig_tree != dst_tree:
            await repo.save_tree(orig_tree)

        tag_d = tag_m.to_resp(dst_tree_id, dst_parent_node_id)
        return ok(message=tag_d.model_dump())

    @auth(PriviledgeLevel.AuthedUser.value)
    @router.delete("/tag/{tag_id}")
    async def remove(self, shr_ctx: SharedContext, authed_user: AuthUser, tag_id: str) -> Response:
        perm_err = permission_check(authed_user.claims, ["delete_producttag"])
        if perm_err:
            return forbidden(message=perm_err)
        (tree_id, node_id) = TagModel.decode_req_id(tag_id)
        repo = shr_ctx.datastore.tag
        tree = await repo.fetch_tree(tree_id)  # TODO, return 410 if not exists
        removed_node = tree.try_remove(node_id)  # noqa: F841
        if tree.empty():
            await repo.delete_tree(tree)
        else:
            await repo.save_tree(tree)
        return no_content()

    @router.get("/tag/{tag_id}")
    async def get_tag(
        self,
        shr_ctx: SharedContext,
        tag_id: str,
        acs_req: Optional[int],
        desc_lvl: Optional[int],
    ) -> Response:
        (tree_id, node_id) = TagModel.decode_req_id(tag_id)
        repo = shr_ctx.datastore.tag
        tree = await repo.fetch_tree(tree_id)  # TODO, return 404 if not exists
        curr_tag = tree.find_node(node_id)  # TODO, return 404 if not exists
        ancestors = None
        if acs_req:
            ancestors = tree.ancestors_dto(curr_tag)
        descendants = None
        if desc_lvl:
            descendants = tree.descendants_dto(curr_tag, desc_lvl)
        tag_d = TagReadRespDto(
            curr_node=curr_tag.to_node_dto(tree_id),
            ancestors=ancestors,
            descendants=descendants,
        )
        return ok(message=tag_d.model_dump())
