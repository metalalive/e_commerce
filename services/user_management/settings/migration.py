from .common import *  # noqa : F403

DATABASES["default"].update(DATABASES["usermgt_service"])  # noqa : F405
render_logging_handler_localfs(BASE_DIR.joinpath("tmp/log/dev"))  # noqa : F405
