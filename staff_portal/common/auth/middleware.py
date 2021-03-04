from datetime import datetime, timedelta, timezone
import secrets
import logging
import jwt

from django.conf       import settings as django_settings
from django.core.cache       import caches as DjangoBuiltinCaches
from django.utils.http       import http_date
from django.utils.cache      import patch_vary_headers
from django.utils.module_loading import import_string

from common.models.db import db_middleware_exception_handler
from common.util.python  import ExtendedDict

_logger = logging.getLogger(__name__)


class JWTbaseMiddleware:
    DEFAULT_AUTH_BACKEND_INDEX = 0
    enable_setup = True
    enable_verify = True

    def __init__(self, get_response):
        self.get_response = get_response
        jwt_cookie_name = getattr(django_settings, 'JWT_COOKIE_NAME', None)
        assert jwt_cookie_name, 'jwt_cookie_name must be set when applying JWTbaseMiddleware'
        self._backend_map = {}
        cnt = self.DEFAULT_AUTH_BACKEND_INDEX
        for bkn_path in django_settings.AUTHENTICATION_BACKENDS:
            self._backend_map[cnt] = bkn_path
            self._backend_map[bkn_path] = cnt
            cnt += 1


    @db_middleware_exception_handler
    def __call__(self, request):
        if self.enable_verify :
            self.process_request(request)
        response = self.get_response(request)
        if self.enable_setup :
            self.process_response(request, response)
        return response


    def process_response(self, request, response):
        if not hasattr(request, 'jwt'):
            return
        ##print('request.jwt in middleware : %s' % request.jwt)
        ##print('request.jwt.modified : %s' % request.jwt.modified)
        if request.jwt.destroy:
            self._remove_token(request, response)
        elif request.jwt.modified:
            self._set_token_to_cookie(request, response)

    def _remove_token(self, request, response):
        response.delete_cookie(
            django_settings.JWT_COOKIE_NAME,
            path=django_settings.SESSION_COOKIE_PATH,
            domain=django_settings.SESSION_COOKIE_DOMAIN,
        )
        # TODO, delete corresponding JWT secret key
        acc_id = request.jwt.payload['acc_id']
        cache_jwt_secret = DjangoBuiltinCaches['jwt_secret']
        result = cache_jwt_secret.delete(acc_id)
        patch_vary_headers(response, ('Cookie',))
        ##print('user %s delete jwt from cookie : %s , result: %s' % \
        ##        (acc_id, request.jwt.encoded, result))

    def _set_token_to_cookie(self, request, response):
        from . import _determine_expiry
        # encode backend module path to index
        default_backend_path = self._backend_map[self.DEFAULT_AUTH_BACKEND_INDEX]
        backend_path = request.jwt.payload.get('bkn_id', default_backend_path)
        request.jwt.payload['bkn_id'] = self._backend_map[backend_path]
        # defaults some claims in header & payload section,
        max_age = _determine_expiry(user=request.user)
        issued_at = datetime.utcnow()
        expires = issued_at + timedelta(max_age)
        # exp & iat field must be NumericDate, see section 4.1.4 , RFC7519
        default_payld = {'exp':  expires, 'iat': issued_at, 'iss':'YOUR_ISSUER'}
        default_header = {'alg':'HS384'}
        request.jwt.default_claims(header_kwargs=default_header, payld_kwargs=default_payld)
        # then encode & sign the token
        encoded = request.jwt.encode(refresh_secret=True)
        response.set_cookie(
            key=django_settings.JWT_COOKIE_NAME, value=encoded,  max_age=max_age,
            expires=http_date(expires.timestamp()),
            domain=django_settings.SESSION_COOKIE_DOMAIN,
            path=django_settings.SESSION_COOKIE_PATH,
            secure=django_settings.SESSION_COOKIE_SECURE or None,
            samesite=django_settings.SESSION_COOKIE_SAMESITE,
            httponly=django_settings.SESSION_COOKIE_HTTPONLY
        )
        ##print('set jwt to cookie : %s' % encoded)


    def process_request(self, request):
        from django.contrib.auth.models  import AnonymousUser # cannot be imported initially ?
        user = None
        payld = jwt_httpreq_verify(request=request)
        if payld and request.jwt.valid is True:
            acc_id = payld.get('acc_id', None)
            backend_id = payld.get('bkn_id', self.DEFAULT_AUTH_BACKEND_INDEX)
            user = self._get_user(acc_id, backend_id)
        request.user = user or AnonymousUser()

    def _get_user(self, acc_id, backend_id):
        """
        since django auth.get_user() is tied closely with session object,
        the get_user() cannot be applied after JWT authentication.

        load backend class for manipulating user model
        """
        assert backend_id is not None and backend_id >= self.DEFAULT_AUTH_BACKEND_INDEX, 'backend_id must not be null'
        backend_path = self._backend_map[backend_id] # decode index to module path
        backend = import_string(backend_path)()
        return backend.get_user(acc_id)

## end of class JWTbaseMiddleware


class JWTsetupMiddleware(JWTbaseMiddleware):
    enable_setup = True
    enable_verify = False

class JWTverifyMiddleware(JWTbaseMiddleware):
    enable_setup = False
    enable_verify = True


def jwt_httpreq_verify(request):
    jwt_cookie_name = django_settings.JWT_COOKIE_NAME
    encoded = request.COOKIES.get(jwt_cookie_name, None)
    if encoded is None:
        return
    _jwt = JWT(encoded=encoded)
    request.jwt = _jwt
    return  _jwt.verify()



class JWT:
    """
    internal wrapper class for detecting JWT write, verify, and generate encoded token,
    in this wrapper, `acc_id` claim is required in payload
    """
    SECRET_SIZE = 40

    def __init__(self, encoded=None):
        self.encoded = encoded
        self._destroy = False
        self._valid = None

    @property
    def encoded(self):
        return self._encoded

    @encoded.setter
    def encoded(self, value):
        self._encoded = value
        if value:
            header = jwt.get_unverified_header(value)
            payld  = jwt.decode(value, options={'verify_signature':False})
        else:
            header = {}
            payld  = {}
        self._payld  = ExtendedDict(payld)
        self._header = ExtendedDict(header)

    @property
    def payload(self):
        return self._payld

    @property
    def header(self):
        return self._header

    @property
    def modified(self):
        return self.header.modified or self.payload.modified

    @property
    def valid(self):
        """ could be True, False, or None (not verified yet) """
        return self._valid

    @property
    def destroy(self):
        return self._destroy

    @destroy.setter
    def destroy(self, value:bool):
        self._destroy = value

    def verify(self, unverified=None):
        self._valid = False
        unverified = unverified or self.encoded
        if unverified is None:
            return
        alg    = self.header['alg']
        acc_id = self.payload['acc_id']
        cache_jwt_secret = DjangoBuiltinCaches['jwt_secret']
        secret = cache_jwt_secret.get(acc_id , None)
        if secret is None:
            return
        try:
            verified = jwt.decode(unverified, secret, algorithms=alg)
            errmsg = 'data inconsistency, self.payload = %s , verified = %s'
            assert self.payload == verified, errmsg % (self.payload, verified)
            self._valid = True
        except Exception as e:
            # TODO, logging at debug/warning level
            print('exception when verifying jwt : %s' % e)
            verified = None
        # TODO, refresh check
        return verified


    def encode(self, refresh_secret):
        if self.modified:
            alg    = self.header['alg']
            acc_id = self.payload['acc_id']
            cache_jwt_secret = DjangoBuiltinCaches['jwt_secret']
            if refresh_secret:
                secret = secrets.token_urlsafe(self.SECRET_SIZE)
                cache_jwt_secret.set(acc_id , secret)
            else:
                secret = cache_jwt_secret[acc_id]
            out = jwt.encode(self.payload, secret, algorithm=alg)
        else:
            out = self.encoded
        return out

    def default_claims(self, header_kwargs, payld_kwargs):
        assert header_kwargs and payld_kwargs, 'Both of kwargs arguments must contain claim fields'
        self.header.update(header_kwargs, overwrite=False)
        self.payload.update(payld_kwargs, overwrite=False)


