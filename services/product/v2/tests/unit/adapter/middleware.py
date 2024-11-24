import asyncio

import pytest
from blacksheep import Request, Response, Content
from blacksheep.server.responses import ok

from product.adapter.middleware import RateLimiter, ReqBodySizeLimiter


async def mock_req_handler(req: Request) -> Response:
    return ok("done success")


class TestRateLimiter:
    @pytest.mark.asyncio(loop_scope="class")
    async def test_ok(self):
        mock_req = Request(
            method="POST",
            url=b"http://example.com/api/resource",
            headers=None,
        )
        rl_m = RateLimiter(max_reqs=5, interval_secs=1)
        for _ in range(5):
            resp = await rl_m(mock_req, mock_req_handler)
            assert resp.status == 200
        await asyncio.sleep(1)
        resp = await rl_m(mock_req, mock_req_handler)
        assert resp.status == 200

    @pytest.mark.asyncio(loop_scope="class")
    async def test_limit_reached(self):
        mock_req = Request(
            method="POST",
            url=b"http://example.com/api/resource",
            headers=None,
        )
        rl_m = RateLimiter(max_reqs=1, interval_secs=2)
        resp = await rl_m(mock_req, mock_req_handler)
        assert resp.status == 200
        resp = await rl_m(mock_req, mock_req_handler)
        assert resp.status == 503


class TestReqBodySzLimiter:
    @pytest.mark.asyncio(loop_scope="class")
    async def test_ok(self):
        mock_req = Request(
            method="POST",
            url=b"http://example.com/api/resource",
            headers=None,
        )
        bsz_m = ReqBodySizeLimiter(max_nbytes=6)
        mock_req.content = Content(b"application/json", b"abcd")
        resp = await bsz_m(mock_req, mock_req_handler)
        assert resp.status == 200
        mock_req.content = Content(b"application/json", b"abcdefg")
        resp = await bsz_m(mock_req, mock_req_handler)
        assert resp.status == 413
