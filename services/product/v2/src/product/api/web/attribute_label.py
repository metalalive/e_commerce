from typing import List, Optional

from blacksheep import Response, Request, Content
from blacksheep.server.authorization import auth
from blacksheep.server.controllers import APIController
from blacksheep.server.responses import created, ok, no_content  # status_code
from blacksheep.server.bindings import FromJSON, FromQuery, QueryBinder, BoundValue
import pydantic_core

from product.model import AttrLabelModel
from product.shared import SharedContext

from . import router
from ..dto import AttrCreateReqDto, AttrUpdateReqDto


class FromQueryListStr(BoundValue[str]):
    pass


class ListStrQueryBinder(QueryBinder):
    handle = FromQueryListStr

    async def get_value(self, request: Request) -> Optional[List[str]]:
        serial = await super().get_value(request)
        return serial.split(",")


class AttrLabelController(APIController):
    @auth("authed_staff_only")
    @router.post("/attributes")
    async def create(
        self, shr_ctx: SharedContext, reqbody: FromJSON[List[AttrCreateReqDto]]
    ) -> Response:
        reqbody = reqbody.value
        repo = shr_ctx.datastore.prod_attri
        ms = AttrLabelModel.from_create_reqs(reqbody)
        await repo.create(ms)
        respbody = AttrLabelModel.to_resps(ms)
        return created(message=respbody)

    @auth("authed_staff_only")
    @router.put("/attributes")
    async def update(
        self, shr_ctx: SharedContext, reqbody: FromJSON[List[AttrUpdateReqDto]]
    ) -> Response:
        reqbody = reqbody.value
        repo = shr_ctx.datastore.prod_attri
        ms = AttrLabelModel.from_update_reqs(reqbody)
        await repo.update(ms)
        respbody = AttrLabelModel.to_resps(ms)
        return ok(message=respbody)

    @auth("authed_staff_only")
    @router.delete("/attributes")
    async def delete(self, shr_ctx: SharedContext, ids: FromQueryListStr) -> Response:
        repo = shr_ctx.datastore.prod_attri
        await repo.delete(ids=ids.value)
        return no_content()

    @router.get("/attributes")
    async def search(self, shr_ctx: SharedContext, keyword: FromQuery[str]) -> Response:
        repo = shr_ctx.datastore.prod_attri
        ms = await repo.search(keyword=keyword.value)
        attrs_d = AttrLabelModel.to_resps(ms)
        respbody = pydantic_core.to_json(attrs_d)
        return Response(200, content=Content(b"application/json", respbody))
