import os
import argparse

# set ExtendedLogger as default logger
from ecommerce_common.logging.logger import ExtendedLogger  # noqa: F401
from importlib import import_module
from contextlib import asynccontextmanager
from typing import Callable, Tuple

app_setting_path = os.environ["APP_SETTINGS"]
settings = import_module(app_setting_path)

import logging  # noqa: E402
from fastapi import FastAPI  # noqa: E402
from fastapi.middleware.cors import CORSMiddleware  # noqa: E402

from ecommerce_common.util import import_module_string  # noqa: E402
from ecommerce_common.cors import config as cors_cfg  # noqa: E402

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
    _logger.info(None, "action", "lifespan-start")
    yield shr_ctx
    _logger.info(None, "action", "lifespan-terminating")
    fn = import_module_string(dotted_path=settings.DEINIT_SHARED_CONTEXT_FN)
    await fn(app)


def load_exc_hdlr(raw: Tuple[str, str]) -> Tuple[Exception, Callable]:
    path_hdlr, path_exc = raw
    hdlr = import_module_string(dotted_path=path_hdlr)
    exc_cls = import_module_string(dotted_path=path_exc)
    return (exc_cls, hdlr)


def _init_app(_setting):
    exc_hdlrs = dict(map(load_exc_hdlr, _setting.EXCEPTION_HANDLERS))
    out = FastAPI(lifespan=toplvl_lifespan_cb, exception_handlers=exc_hdlrs)
    for path in _setting.ROUTERS:
        router = import_module_string(dotted_path=path)
        out.include_router(router)
    # init CORS middleware
    out.add_middleware(
        CORSMiddleware,
        allow_origins=[cors_cfg.ALLOWED_ORIGIN[k] for k in ["web", "store"]],
        allow_credentials=True,  # disable cookie from other (even trusted) domains
        allow_methods=cors_cfg.ALLOWED_METHODS,
        allow_headers=cors_cfg.ALLOWED_HEADERS,
        max_age=cors_cfg.PREFLIGHT_MAX_AGE,
    )
    return out


# Lifespan feature in `uvicorn` server requires `FastAPI` instance to
# be created as soon as this module is intially loaded by `uvicorn`
app: FastAPI = _init_app(_setting=settings)

if __name__ == "__main__":
    import uvicorn

    parser = argparse.ArgumentParser(description="Run the FastAPI application.")
    parser.add_argument("--host", type=str, required=True, help="Host address to bind to.")
    parser.add_argument("--port", type=int, required=True, help="Port to listen on.")
    parser.add_argument(
        "--access-log",
        type=lambda x: (str(x).lower() == "true"),  # Convert string 'true'/'false' to boolean
        required=True,
        help="Enable access logging (true/false).",
    )
    parser.add_argument(
        "--log-config", type=str, required=True, help="Path to log configuration file."
    )
    args = parser.parse_args()
    uvicorn.run(
        "store.entry.web:app",
        host=args.host,
        port=args.port,
        access_log=args.access_log,
        log_config=args.log_config,
    )
