from typing import Optional

from blacksheep import FromJSON, Response
from blacksheep.server.controllers import APIController
from blacksheep.server.responses import created, ok, no_content  # status_code

from product.model import TagModel, TagTreeModel
from product.shared import SharedContext

from . import router
from ..dto import TagCreateReqDto, TagUpdateReqDto, TagReadRespDto


class TagController(APIController):
    @router.post("/tag")
    async def create(
        self, shr_ctx: SharedContext, reqbody: FromJSON[TagCreateReqDto]
    ) -> Response:
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

    @router.patch("/tag/{tag_id}")
    async def modify(
        self, shr_ctx: SharedContext, tag_id: str, reqbody: FromJSON[TagUpdateReqDto]
    ) -> Response:
        repo = shr_ctx.datastore.tag
        reqbody = reqbody.value
        (orig_tree_id, orig_node_id) = TagModel.decode_req_id(tag_id)
        orig_tree = await repo.fetch_tree(orig_tree_id)
        if reqbody.parent:
            (dst_tree_id, dst_parent_node_id) = TagModel.decode_req_id(reqbody.parent)
            if orig_tree_id == dst_tree_id:
                dst_tree = orig_tree
            else:
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

    @router.delete("/tag/{tag_id}")
    async def remove(self, shr_ctx: SharedContext, tag_id: str) -> Response:
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
