from .common import *

DATABASES["default"].update(DATABASES["usermgt_service"])
for d in DATABASES.values():
    d["HOST"] = "127.0.0.1"

render_logging_handler_localfs(BASE_DIR.joinpath("tmp/log/dev"))
