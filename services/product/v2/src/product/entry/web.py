import logging
import os
from importlib import import_module
from typing import Dict

from blacksheep import Application, Router
from blacksheep.server.authentication.jwt import JWTBearerAuthentication
from guardpost import Policy
from guardpost.common import AuthenticatedRequirement

from ecommerce_common.util import import_module_string
from ..util import PriviledgeLevel

_logger = logging.getLogger(__name__)

app_setting_path = os.environ["APP_SETTINGS"]
setting = import_module(app_setting_path)


def init_app(setting) -> Application:
    # FIXME,
    # sub-router does not seem to work well with CORS feature in application class
    # , current workaround is to avoid sub routers, use only one router for entire
    # applicaiton. I'll retry this feature in future blacksheep version
    toplvl_router: Router = import_module_string(setting.ROUTER)

    def init_middlewares(cls_path: str, kwargs: Dict):
        cls = import_module_string(cls_path)
        return cls(**kwargs)

    middlewares = [init_middlewares(k, v) for k, v in setting.MIDDLEWARES.items()]
    _app = Application(router=toplvl_router)
    CorsConfig = import_module("ecommerce_common.cors.config")
    _app.use_cors(
        allow_methods=CorsConfig.ALLOWED_METHODS,
        allow_headers=CorsConfig.ALLOWED_HEADERS,
        allow_origins=[CorsConfig.ALLOWED_ORIGIN[k] for k in ["web", "product"]],
        allow_credentials=CorsConfig.ALLOW_CREDENTIALS,
        max_age=CorsConfig.PREFLIGHT_MAX_AGE,
    )
    _app.middlewares.extend(middlewares)
    key_provider_cls = import_module_string(dotted_path=setting.AUTH_KEY_PROVIDER)
    jwtauth = JWTBearerAuthentication(
        valid_audiences=["web", "product"],
        authority=setting.JWT_ISSUER,
        keys_provider=key_provider_cls(setting.KEYSTORE),
    )
    _app.use_authentication().add(jwtauth)
    authorization = _app.use_authorization()
    authorization += Policy(PriviledgeLevel.AuthedUser.value, AuthenticatedRequirement())
    return _app


app: Application = init_app(setting)


@app.lifespan
async def app_lifespan(app: Application):
    shr_ctx_cls = import_module_string(dotted_path=setting.SHARED_CONTEXT)
    shr_ctx = await shr_ctx_cls.init(setting)
    app.services.register(shr_ctx_cls, instance=shr_ctx)
    _logger.info("[app]life-span starting")
    yield shr_ctx
    _logger.info("[app]life-span terminating")
    await shr_ctx.deinit()
