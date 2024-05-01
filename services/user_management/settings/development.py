from .common import *

AUTH_KEYSTORE["persist_secret_handler"]["init_kwargs"]["filepath"] = BASE_DIR.joinpath(
    "tmp/cache/dev/jwks/privkey/current.json"
)
AUTH_KEYSTORE["persist_pubkey_handler"]["init_kwargs"]["filepath"] = BASE_DIR.joinpath(
    "tmp/cache/dev/jwks/pubkey/current.json"
)

render_logging_handler_localfs(BASE_DIR.joinpath("tmp/log/dev"))
