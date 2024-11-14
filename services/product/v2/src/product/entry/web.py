import logging
import os
from importlib import import_module
from blacksheep import Application, Router
from ecommerce_common.util import import_module_string

_logger = logging.getLogger(__name__)

app_setting_path = os.environ["APP_SETTINGS"]
setting = import_module(app_setting_path)


def init_app(setting) -> Application:
    sub_routers = list(map(import_module_string, setting.ROUTERS))
    toplvl_router = Router(sub_routers=sub_routers)
    return Application(router=toplvl_router)


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
