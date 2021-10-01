import logging

from django.conf import settings as django_settings
from django.apps import apps as django_apps
from django.core.exceptions import ObjectDoesNotExist, MultipleObjectsReturned
from rest_framework import HTTP_HEADER_ENCODING
from rest_framework.permissions import BasePermission
from rest_framework.exceptions  import AuthenticationFailed

from common.auth.abstract import BaseGetProfileMixin
from common.auth.keystore import create_keystore_helper
from common.auth.jwt    import JWT
from common.cors.middleware import conf as cors_conf
from common.util.python import import_module_string

_logger = logging.getLogger(__name__)


class IsStaffUser(BasePermission):
    """
    Allows access only to staff or superusers.
    """
    def has_permission(self, request, view):
        return bool(request.user and (request.user.is_staff or request.user.is_superuser))


class IsSuperUser(BasePermission):
    """
    Allows access only to superusers.
    """
    def has_permission(self, request, view):
        return bool(request.user and request.user.is_superuser)


def get_authorization_header(request):
    """
    Return request's 'Authorization:' header, as a bytestring.
    Hide some test client ickyness where the header can be unicode.
    """
    auth = request.META.get('HTTP_AUTHORIZATION', b'')
    if isinstance(auth, str):
        # Work around django test client oddness
        auth = auth.encode(HTTP_HEADER_ENCODING)
    return auth



class DjangoGetProfileMixin(BaseGetProfileMixin):
    def get_account(self, profile_id):
        try:
            usr_model_path = django_settings.AUTH_USER_MODEL
            usr_model_cls = django_apps.get_model(usr_model_path, require_ready=False)
            account = usr_model_cls.objects.get(profile=profile_id)
            if not account.is_active:
                raise AuthenticationFailed('User inactive or deleted.')
            return account
        except ObjectDoesNotExist as e:
            raise AuthenticationFailed('User inactive or deleted.')
        except (AttributeError, ImportError, MultipleObjectsReturned) as e:
            raise # TODO, log the internal error


class RefreshJWTauthentication(DjangoGetProfileMixin):
    def authenticate(self, request):
        # grab refresh token from cookie
        refresh_token_key = django_settings.JWT_NAME_REFRESH_TOKEN
        encoded_rfr_tok = request.COOKIES.get(refresh_token_key, None)
        account = None
        payld_verified = None
        if encoded_rfr_tok:
            _keystore = create_keystore_helper(cfg=django_settings.AUTH_KEYSTORE, import_fn=import_module_string)
            rfr_jwt = JWT()
            result = rfr_jwt.verify(unverified=encoded_rfr_tok, audience=None, keystore=_keystore)
            if result:
                payld_verified = result
                profile_id = payld_verified.get('profile')
                account = self.get_account(profile_id)
        return (account, payld_verified)



class AccessJWTauthentication(DjangoGetProfileMixin):
    """
    JWT-based authentication without relying on django.contrib.auth and any
    database backend.

    This interface of this class is completely the same as DRF's TokenAuthentication
    , howeverm DRF's TokenAuthentication relies on database to store token value,
    and `django.contrib.auth` package is imported in `rest_framework.authentication`
    on module initialization, so I cannot import `rest_framework.authentication.TokenAuthentication`

    Clients should authenticate by passing the token key in the "Authorization"
    HTTP header, prepended with the string "Bearer ".  For example:

        Authorization: Bearer 401f7ac837da42b97f613d789819ff93537bee6a
    """
    keyword = 'Bearer' # stick to HTTP spec

    """
    A custom token model may be used, but must have the following properties.

    * key -- The string identifying the token
    * user -- The user to which the token belongs
    """

    def authenticate(self, request):
        auth = get_authorization_header(request).split()

        if not auth or auth[0].lower() != self.keyword.lower().encode():
            return None
        if len(auth) == 1:
            msg = _('Invalid token header. No credentials provided.')
            raise AuthenticationFailed(msg)
        elif len(auth) > 2:
            msg = _('Invalid token header. Token string should not contain spaces.')
            raise AuthenticationFailed(msg)
        try:
            token = auth[1].decode()
        except UnicodeError:
            msg = _('Invalid token header. Token string should not contain invalid characters.')
            raise AuthenticationFailed(msg)
        # looking for appropriate audience by reading host name
        host_name = '%s://%s' % (request.scheme, request.get_host())
        audience = filter(lambda kv: kv[1] == host_name, cors_conf.ALLOWED_ORIGIN.items())
        audience = list(map(lambda kv:kv[0], audience))
        return self.authenticate_credentials(encoded_acs_tok=token, audience=audience)


    def authenticate_credentials(self, encoded_acs_tok, audience):
        account = None
        payld_verified = None
        _keystore = create_keystore_helper(cfg=django_settings.AUTH_KEYSTORE, import_fn=import_module_string)
        acs_tok = JWT()
        result = acs_tok.verify(unverified=encoded_acs_tok, audience=audience, keystore=_keystore)
        if result:
            payld_verified = result
            profile_id = payld_verified.get('profile')
            account = self.get_account(profile_id)
        else:
            raise AuthenticationFailed('Invalid token.')
        return (account, payld_verified)

    def authenticate_header(self, request):
        return self.keyword

