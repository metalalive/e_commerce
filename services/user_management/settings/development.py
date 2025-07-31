from .common import *  # noqa : F403

ks_cfg = AUTH_KEYSTORE["persist_secret_handler"]["init_kwargs"]  # noqa : F405
ks_cfg["filepath"] = BASE_DIR.joinpath(  # noqa : F405
    "tmp/cache/dev/jwks/privkey/current.json"
)
ks_cfg = AUTH_KEYSTORE["persist_pubkey_handler"]["init_kwargs"]  # noqa : F405
ks_cfg["filepath"] = BASE_DIR.joinpath(  # noqa : F405
    "tmp/cache/dev/jwks/pubkey/current.json"
)

# TODO, separate accounts in DB server
DATABASES.pop("test_site2_dba")  # noqa : F405
DATABASES["default"].update(DATABASES["usermgt_service"])  # noqa : F405
