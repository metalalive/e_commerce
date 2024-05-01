from ecommerce_common.models.constants import ROLE_ID_SUPERUSER, ROLE_ID_STAFF

_PRESERVED_ROLE_IDS = (
    ROLE_ID_SUPERUSER,
    ROLE_ID_STAFF,
)

# TODO, parameterize
WEB_HOST = "http://localhost:8006"
LOGIN_WEB_URL = WEB_HOST + "/login"

MAX_NUM_FORM = 7
