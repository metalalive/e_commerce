import time
from collections import deque
from typing import Awaitable, Callable

from blacksheep import Request, Response
from blacksheep.server.responses import status_code


class RateLimiter:
    def __init__(self, max_reqs: int, interval_secs: int = 2):
        self._max_reqs = max_reqs
        self._interval_secs = interval_secs
        self._timestamps = deque(maxlen=max_reqs + 1)

    async def __call__(
        self, request: Request, handler: Callable[[Request], Awaitable[Response]]
    ) -> Response:
        curr_time = time.time()
        while (
            self._timestamps and self._timestamps[0] < curr_time - self._interval_secs
        ):
            self._timestamps.popleft()
        if len(self._timestamps) < self._max_reqs:
            self._timestamps.append(curr_time)
            response = await handler(request)
        else:
            response = status_code(503, "\n")
        return response


class ReqBodySizeLimiter:
    def __init__(self, max_nbytes: int):
        self._max_nbytes = max_nbytes

    async def __call__(
        self, request: Request, handler: Callable[[Request], Awaitable[Response]]
    ) -> Response:
        # TODO, extract expected size if it is streaming body
        if request.content is not None and request.content.body is not None:
            curr_body_sz = len(request.content.body)
        else:
            curr_body_sz = 0
        if curr_body_sz < self._max_nbytes:
            response = await handler(request)
        else:
            response = status_code(413, "\n")  # payload too large
        return response
