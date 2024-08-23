import os
from importlib import import_module

app_setting_path = os.environ["APP_SETTINGS"]
settings = import_module(app_setting_path)

import logging
from contextlib import asynccontextmanager
from fastapi import FastAPI
from fastapi.middleware.cors import CORSMiddleware

from ecommerce_common.util import import_module_string
from ecommerce_common.cors import config as cors_cfg

_logger = logging.getLogger(__name__)


@asynccontextmanager
async def toplvl_lifespan_cb(app: FastAPI):
    """
    current workaround of app-level lifespan generator callback
    As of FastAPI v0.101 and startlette v0.27, lifespan is not supported yet
    at router (or sub-application) level, due to the following discussions / issues
    https://github.com/tiangolo/fastapi/discussions/9664
    https://github.com/tiangolo/fastapi/pull/9630
    https://github.com/encode/starlette/issues/649
    https://github.com/encode/starlette/pull/1988
    """
    fn = import_module_string(dotted_path=settings.INIT_SHARED_CONTEXT_FN)
    shr_ctx = await fn(app)
    _logger.info("[app]life-span starting")
    yield shr_ctx
    _logger.info("[app]life-span terminating")
    fn = import_module_string(dotted_path=settings.DEINIT_SHARED_CONTEXT_FN)
    await fn(app)


def _init_app(_setting):
    out = FastAPI(lifespan=toplvl_lifespan_cb)
    for path in _setting.ROUTERS:
        router = import_module_string(dotted_path=path)
        out.include_router(router)
    # init CORS middleware
    out.add_middleware(
        CORSMiddleware,
        allow_origins=cors_cfg.ALLOWED_ORIGIN.values(),
        allow_credentials=True,  # disable cookie from other (even trusted) domains
        allow_methods=cors_cfg.ALLOWED_METHODS,
        allow_headers=cors_cfg.ALLOWED_HEADERS,
        max_age=cors_cfg.PREFLIGHT_MAX_AGE,
    )
    return out


# Lifespan feature in `uvicorn` server requires `FastAPI` instance to
# be created as soon as this module is intially loaded by `uvicorn`
app: FastAPI = _init_app(_setting=settings)
