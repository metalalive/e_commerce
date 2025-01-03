from jwt.exceptions import DecodeError
from ecommerce_common.auth.jwt import JWT


def base_authentication(token: str, audience, keystore, error_obj=None):
    jwt = JWT(encoded=token)
    payld = jwt.verify(keystore=keystore, audience=audience)
    if not payld:
        raise DecodeError("payload of jwt token is null, authentication failure")
    return payld


def base_permission_check(
    user: dict, app_code: int, required_perm_codes: set, error_obj
):
    granted_perms = user.get("perms", [])
    granted_perms = filter(lambda d: d["app_code"] == app_code, granted_perms)
    granted_perm_codes = set(map(lambda d: d["codename"], granted_perms))
    coverage = required_perm_codes - granted_perm_codes
    if any(coverage):
        raise error_obj
    return user


def get_unverified_token_payld(token: str):
    try:
        jwt = JWT(encoded=token)
        payld = jwt.payload
    except DecodeError as e:
        payld = {}
    return payld
