from .common import *

DEBUG = False

AUTH_KEYSTORE["persist_secret_handler"]["init_kwargs"]["filepath"] = BASE_DIR.joinpath(
    "tmp/cache/production/jwks/privkey/current.json"
)
AUTH_KEYSTORE["persist_pubkey_handler"]["init_kwargs"]["filepath"] = BASE_DIR.joinpath(
    "tmp/cache/production/jwks/pubkey/current.json"
)
render_logging_handler_localfs(BASE_DIR.joinpath("tmp/log/production"))
