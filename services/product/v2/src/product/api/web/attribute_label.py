from typing import List, Optional

from blacksheep import Response, Request
from blacksheep.server.controllers import APIController
from blacksheep.server.responses import created, ok, no_content  # status_code
from blacksheep.server.bindings import FromJSON, QueryBinder, BoundValue

from product.model import AttrLabelModel
from product.shared import SharedContext

from . import router
from ..dto import AttrCreateReqDto, AttrUpdateReqDto, AttrLabelDto, AttrDataTypeDto


class FromQueryListStr(BoundValue[str]):
    pass


class ListStrQueryBinder(QueryBinder):
    handle = FromQueryListStr

    async def get_value(self, request: Request) -> Optional[List[str]]:
        serial = await super().get_value(request)
        return serial.split(",")


class AttrLabelController(APIController):
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

    @router.delete("/attributes")
    async def delete(self, ids: FromQueryListStr) -> Response:
        # ids = ids.value
        return no_content()

    @router.get("/attributes")
    async def search(self, keywords: FromQueryListStr) -> Response:
        # keywords = keywords.value
        respbody = [
            AttrLabelDto(
                id_="56neverFall",
                name="inner diameter",
                dtype=AttrDataTypeDto.UnsignedInteger,
            ).model_dump()
        ]
        return ok(message=respbody)
