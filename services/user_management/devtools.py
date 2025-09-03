import pathlib
from datetime import datetime, timedelta

from django.conf import settings as django_settings
from django.utils.module_loading import import_string

from ecommerce_common.auth.keystore import create_keystore_helper
from ecommerce_common.auth.jwt import JWT

"""
Helper functions used in development envisonment.
[Usage]
   go to Django shell console
>>> from user_management import devtools
>>> devtools.gen_auth_token_to_file(filepath='./tmp/log/dev/app_test_access_token', valid_minutes=1200, audiences=['service1','service2','media','service4'], perm_codes=[(9, 'can_do_sth'), (3, 'upload_files')], quota=[(17, 2, 2019), (3, 1, 248), (2454, 97, 590)], usr_id=117, issuer='http://my-app-e-comm')

"""


def gen_auth_token(
    valid_minutes: int,
    usr_id: int,
    audiences: list,
    perm_codes: list,
    quota: list,
    issuer: str,
    key_id: str,
):
    def perm_gen_fn(k):
        return {"app_code": k[0], "codename": k[1]}

    def quota_gen_fn(k):
        return {"app_code": k[0], "mat_code": k[1], "maxnum": k[2]}

    keystore = create_keystore_helper(
        cfg=django_settings.AUTH_KEYSTORE, import_fn=import_string
    )
    now_time = datetime.utcnow()
    expiry = now_time + timedelta(minutes=valid_minutes)
    token = JWT()
    token.header["kid"] = key_id
    payload = {
        "profile": usr_id,
        "aud": audiences,
        "iat": now_time,
        "exp": expiry,
        "perms": list(map(perm_gen_fn, perm_codes)),
        "quota": list(map(quota_gen_fn, quota)),
        "iss": issuer,
    }
    token.payload.update(payload)
    return token.encode(keystore=keystore)


def gen_auth_token_to_file(
    filepath: str,
    valid_minutes: int,
    usr_id: int,
    audiences: list,
    perm_codes: list,
    quota: list,
    issuer: str,
    key_id: str = "",
):
    pathdir = pathlib.Path(filepath).parent
    assert pathdir.exists() and pathdir.is_dir(), "pathdir not exists, %s" % pathdir
    encoded_token = gen_auth_token(
        valid_minutes, usr_id, audiences, perm_codes, quota, issuer, key_id
    )
    with open(filepath, "w") as f:
        f.write(encoded_token)
