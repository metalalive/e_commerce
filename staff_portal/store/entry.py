
from fastapi import FastAPI
from fastapi.middleware.cors import CORSMiddleware

from common.util.python import import_module_string
from common.cors        import config as cors_cfg

from .settings import common as settings

def _init_app(_setting):
    out = FastAPI()
    for path in _setting.ROUTERS:
        router = import_module_string(dotted_path=path)
        out.include_router(router)
    # init CORS middleware
    out.add_middleware(
        CORSMiddleware,
        allow_origins=cors_cfg.ALLOWED_ORIGIN.values(),
        allow_credentials=True, # disable cookie from other (even trusted) domains
        allow_methods=cors_cfg.ALLOWED_METHODS,
        allow_headers=cors_cfg.ALLOWED_HEADERS,
        max_age=cors_cfg.PREFLIGHT_MAX_AGE
    )
    return out

app = _init_app(_setting=settings)

