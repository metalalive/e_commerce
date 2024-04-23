from datetime import timedelta

from django.core import signing
from django.utils.crypto import salted_hmac
from django.contrib.sessions.backends.file import SessionStore as FileSessionStore


def _get_webapp_signkey(secrets_path):
    """
    * in this project, session is used only in web application.
      For (REST) API calls, JWT will be used.
    """
    import json

    secrets = None
    with open(secrets_path, "r") as f:
        secrets = json.load(f)
        secrets = secrets["backend_apps"]
    assert secrets, "failed to load secrets from file %s" % secrets_path
    key = secrets["secret_key"]["staff"]["web"]
    return key


def _monkey_patch():
    from django.contrib.sessions.backends.base import SessionBase
    from django.contrib.auth.base_user import AbstractBaseUser

    _sign_key = _get_webapp_signkey(secrets_path="./common/data/secrets.json")

    def get_session_auth_hash(self):
        key_salt = "django.contrib.auth.models.AbstractBaseUser.get_session_auth_hash"
        return salted_hmac(
            key_salt, self.password, secret=_sign_key, algorithm="sha256"
        ).hexdigest()

    AbstractBaseUser.get_session_auth_hash = get_session_auth_hash

    def encode_with_webapp_secret(self, session_dict):
        return signing.dumps(
            session_dict,
            salt=self.key_salt,
            serializer=self.serializer,
            compress=True,
            key=_sign_key,
        )

    def decode_with_webapp_secret(self, session_data):
        try:
            return signing.loads(
                session_data,
                key=_sign_key,
                salt=self.key_salt,
                serializer=self.serializer,
            )
        except Exception:
            return self._legacy_decode(session_data)

    SessionBase.encode = encode_with_webapp_secret
    SessionBase.decode = decode_with_webapp_secret


_monkey_patch()


class SessionStore(FileSessionStore):
    """
    * override set_expiry() of its parent class, to return correct expiry time
      of a seesion after set_expiry() was called
    * override encode() and decode() in django.contrib.session.backend.base.SessionBase
      in order to change signature key
    """

    # if set_expiry() and _expiry_date() are used together, it will cause get_expiry_age() return
    # constant integer rather than reflect real time period available for accessing that session,
    # don't use Django's default implementation

    def set_expiry(self, value):
        """force int to be convert to timedelta format"""
        if value and isinstance(value, int):
            value = timedelta(seconds=value)
        super().set_expiry(value=value)

    # def _expiry_date(self, session_data):
    #     custom_expiry_secs = session_data.get('_session_expiry')
    #     if custom_expiry_secs and not isinstance(custom_expiry_secs, int):
    #         from django.core.exceptions import ImproperlyConfigured
    #         errmsg = "_session_expiry in file session has to be integer, not object type."
    #         raise ImproperlyConfigured(errmsg)
    #     exp_secs = custom_expiry_secs or self.get_session_cookie_age()
    #     out =  self._last_modification() + timedelta(seconds=exp_secs)
    #     # print("refined file session, _expiry_date , exp_secs: "+ str(exp_secs))
    #     # print("refined file session, _expiry_date , out: "+ str(out))
    #     # print("refined file session, _expiry_date , _last_modification(): "+ str(self._last_modification()))
    #     return out
