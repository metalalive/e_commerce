import os
from importlib import import_module
from blacksheep import Application, Router
from ecommerce_common.util import import_module_string

app_setting_path = os.environ["APP_SETTINGS"]
setting = import_module(app_setting_path)


def init_app(setting) -> Application:
    sub_routers = list(map(import_module_string, setting.ROUTERS))
    toplvl_router = Router(sub_routers=sub_routers)
    return Application(router=toplvl_router)


app: Application = init_app(setting)
