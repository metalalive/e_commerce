from .common import *  # noqa : F403

DATABASES["default"].update(DATABASES["usermgt_service"])  # noqa : F405
