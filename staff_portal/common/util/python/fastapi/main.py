import os
import json
from importlib import import_module
# patch this before importing fastapi
from common.util.python import monkeypatch_typing_specialform
monkeypatch_typing_specialform()

from fastapi import FastAPI
from fastapi.middleware.cors import CORSMiddleware

from .settings import settings

def _init_app(_setting):
    routers = []
    # json format , seek `routers` section
    with open(_setting.fastapi_config_filepath, 'r') as f:
        jf = json.load(f)
        routers = jf['routers']
    out = FastAPI()
    for router in routers:
        _mod = import_module(router)
        out.include_router(_mod.router)
    # init CORS middleware
    from common.cors import config as cors_cfg
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

