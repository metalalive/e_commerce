from .common import *

AUTH_KEYSTORE["persist_secret_handler"]["init_kwargs"]["filepath"] = BASE_DIR.joinpath(
    "tmp/cache/test/jwks/privkey/current.json"
)
AUTH_KEYSTORE["persist_pubkey_handler"]["init_kwargs"]["filepath"] = BASE_DIR.joinpath(
    "tmp/cache/test/jwks/pubkey/current.json"
)
AUTH_KEYSTORE["persist_secret_handler"]["init_kwargs"]["flush_threshold"] = 4
AUTH_KEYSTORE["persist_pubkey_handler"]["init_kwargs"]["flush_threshold"] = 4

# Django test only uses `default` alias , which does NOT allow users to switch
# between different database credentials
DATABASES["default"].update(DATABASES['test_site2_dba'])
DATABASES["default"]["NAME"] = DATABASES["default"]["TEST"]["NAME"]
## does NOT work for testing
##DATABASES['usermgt_service'].update(secrets)
DATABASE_ROUTERS.clear()
render_logging_handler_localfs(BASE_DIR.joinpath("tmp/log/test"))
