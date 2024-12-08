from .common import *

DATABASES["default"].update(DATABASES["usermgt_service"])
render_logging_handler_localfs(BASE_DIR.joinpath("tmp/log/dev"))
