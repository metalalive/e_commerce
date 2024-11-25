from typing import List

from blacksheep import FromJSON, FromQuery, Response
from blacksheep.server.controllers import APIController
from blacksheep.server.responses import created, ok, no_content  # status_code

from . import router
from ..dto import AttrCreateReqDto, AttrUpdateReqDto, AttrLabelDto, AttrDataTypeDto


class AttrLabelController(APIController):
    @router.post("/attributes")
    async def create(self, reqbody: FromJSON[List[AttrCreateReqDto]]) -> Response:
        reqbody = reqbody.value
        respbody = [
            AttrLabelDto(id_="56neverFall", name=r.name, dtype=r.dtype).model_dump()
            for r in reqbody
        ]
        return created(message=respbody)

    @router.put("/attributes")
    async def update(self, reqbody: FromJSON[List[AttrUpdateReqDto]]) -> Response:
        respbody = reqbody.value
        return ok(message=respbody)

    # TODO, auto binding on query parameter
    @router.delete("/attributes")
    async def delete(self, ids: FromQuery[str]) -> Response:
        return no_content()

    @router.get("/attributes")
    async def search(self, keyword: FromQuery[str]) -> Response:
        respbody = [
            AttrLabelDto(
                id_="56neverFall",
                name="inner diameter",
                dtype=AttrDataTypeDto.UnsignedInteger,
            ).model_dump()
        ]
        return ok(message=respbody)
