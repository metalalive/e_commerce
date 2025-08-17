from .common import *  # noqa : F403

ks_cfg = AUTH_KEYSTORE["persist_secret_handler"]["init_kwargs"]  # noqa : F405
ks_cfg["filepath"] = BASE_DIR.joinpath(  # noqa : F405
    "tmp/cache/test/jwks/privkey/current.json"
)
ks_cfg["flush_threshold"] = 4
ks_cfg = AUTH_KEYSTORE["persist_pubkey_handler"]["init_kwargs"]  # noqa : F405
ks_cfg["filepath"] = BASE_DIR.joinpath(  # noqa : F405
    "tmp/cache/test/jwks/pubkey/current.json"
)
ks_cfg["flush_threshold"] = 4

# Django test only uses `default` alias , which does NOT allow users to switch
# between different database credentials
DATABASES["default"].update(DATABASES["test_site2_dba"])  # noqa : F405
DATABASES["default"]["NAME"] = DATABASES["default"]["TEST"]["NAME"]  # noqa : F405
## does NOT work for testing
##DATABASES['usermgt_service'].update(secrets)
DATABASE_ROUTERS.clear()  # noqa : F405
