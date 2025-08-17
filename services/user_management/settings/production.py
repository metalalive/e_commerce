from .common import *  # noqa : F403

DEBUG = False

ks_cfg = AUTH_KEYSTORE["persist_secret_handler"]["init_kwargs"]  # noqa : F405
ks_cfg["filepath"] = BASE_DIR.joinpath(  # noqa : F405
    "tmp/cache/production/jwks/privkey/current.json"
)
ks_cfg = AUTH_KEYSTORE["persist_pubkey_handler"]  # noqa : F405
ks_cfg["init_kwargs"]["filepath"] = BASE_DIR.joinpath(  # noqa : F405
    "tmp/cache/production/jwks/pubkey/current.json"
)
DATABASES.pop("test_site2_dba")  # noqa : F405
DATABASES["default"].update(DATABASES["usermgt_service"])  # noqa : F405
