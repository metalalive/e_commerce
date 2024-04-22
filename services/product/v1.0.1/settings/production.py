# re-import the parameters from common.py , it is acceptable if some variables
# are unused at here
from .common import *  # noqa: F403

DEBUG = False
render_logging_handler_localfs(BASE_DIR.joinpath("tmp/log/production"))  # noqa: F405
