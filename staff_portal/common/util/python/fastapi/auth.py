from jwt.exceptions import (
    DecodeError,    ExpiredSignatureError,    ImmatureSignatureError,
    InvalidAudienceError,    InvalidIssuedAtError,    InvalidIssuerError,
    MissingRequiredClaimError,
)

from common.auth.keystore import create_keystore_helper
from common.util.python import import_module_string
from common.util.python.fastapi.settings import settings as fa_settings
from common.auth.jwt import JWT

def base_authentication(token:str, audience, error_obj=None):
    ks_cfg = fa_settings.keystore_config
    keystore = create_keystore_helper(cfg=ks_cfg, import_fn=import_module_string)
    jwt = JWT(encoded=token)
    try:
        payld = jwt.verify(keystore=keystore, audience=audience)
        if not payld:
            raise DecodeError("payload of jwt token is null, authentication failure")
        return payld
    except (TypeError, DecodeError, ExpiredSignatureError, ImmatureSignatureError, InvalidAudienceError, \
            InvalidIssuedAtError, InvalidIssuerError, MissingRequiredClaimError,) as e:
        if error_obj:
            raise error_obj # TODO, log error
        return None


def base_permission_check(user:dict, required_roles:set, error_obj):
    actual_roles = user.get('roles', [])
    for x in actual_roles:
        required_roles = required_roles - set(x['perm_code'])
    if any(required_roles):
        raise error_obj
    return user


def get_unverified_token_payld(token:str):
    try:
        jwt = JWT(encoded=token)
        payld = jwt.payload
    except DecodeError as e :
        payld = {}
    return payld

