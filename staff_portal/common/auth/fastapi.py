from jwt.exceptions import (
    DecodeError,    ExpiredSignatureError,    ImmatureSignatureError,
    InvalidAudienceError,    InvalidIssuedAtError,    InvalidIssuerError,
    MissingRequiredClaimError,
)

from common.auth.keystore import create_keystore_helper
from common.util.python import import_module_string
from common.auth.jwt import JWT

def base_authentication(token:str, audience, ks_cfg, error_obj=None):
    payld = None
    keystore = create_keystore_helper(cfg=ks_cfg, import_fn=import_module_string)
    try:
        jwt = JWT(encoded=token)
        payld = jwt.verify(keystore=keystore, audience=audience)
        if not payld:
            raise DecodeError("payload of jwt token is null, authentication failure")
    except (TypeError, DecodeError, ExpiredSignatureError, ImmatureSignatureError, InvalidAudienceError, \
            InvalidIssuedAtError, InvalidIssuerError, MissingRequiredClaimError,) as e:
        if error_obj:
            raise error_obj # TODO, log error
        payld = None
    return payld


def base_permission_check(user:dict, app_code:int, required_perm_codes:set, error_obj):
    granted_perms = user.get('perms', [])
    granted_perms = filter(lambda d:d['app_code'] == app_code, granted_perms)
    granted_perm_codes = set(map(lambda d:d['codename'], granted_perms))
    coverage = required_perm_codes - granted_perm_codes
    if any(coverage):
        raise error_obj
    return user


def get_unverified_token_payld(token:str):
    try:
        jwt = JWT(encoded=token)
        payld = jwt.payload
    except DecodeError as e :
        payld = {}
    return payld

