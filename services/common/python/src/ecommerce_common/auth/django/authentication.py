import logging

from django.conf import settings as django_settings
from django.apps import apps as django_apps
from django.core.exceptions import ObjectDoesNotExist, MultipleObjectsReturned
from django.utils.translation import gettext_lazy as _
from rest_framework import HTTP_HEADER_ENCODING
from rest_framework.permissions import BasePermission
from rest_framework.exceptions  import AuthenticationFailed
from jwt.exceptions import (
    DecodeError,    ExpiredSignatureError,    ImmatureSignatureError,
    InvalidAudienceError,    InvalidIssuedAtError,    InvalidIssuerError,
    MissingRequiredClaimError, InvalidKeyError
)

from ecommerce_common.auth.abstract import BaseGetProfileMixin
from ecommerce_common.auth.keystore import create_keystore_helper
from ecommerce_common.auth.jwt    import JWT
from ecommerce_common.models.constants  import ROLE_ID_SUPERUSER, ROLE_ID_STAFF
from ecommerce_common.cors.middleware   import conf as cors_conf
from ecommerce_common.util import import_module_string

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
        result  = None
        payld_verified = None
        if encoded_rfr_tok: # TODO, move keystore to global shared context
            _keystore = create_keystore_helper(cfg=django_settings.AUTH_KEYSTORE, import_fn=import_module_string)
            try:
                rfr_jwt = JWT()
                result = rfr_jwt.verify(unverified=encoded_rfr_tok, audience=None, keystore=_keystore)
            except (DecodeError, ExpiredSignatureError, ImmatureSignatureError, InvalidAudienceError, \
                InvalidIssuedAtError, InvalidIssuerError, MissingRequiredClaimError, InvalidKeyError) as e:
                raise AuthenticationFailed('Invalid token.')
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
        result = None
        payld_verified = None # TODO, move keystore to global shared context
        _keystore = create_keystore_helper(cfg=django_settings.AUTH_KEYSTORE, import_fn=import_module_string)
        try:
            acs_tok = JWT()
            result = acs_tok.verify(unverified=encoded_acs_tok, audience=audience, keystore=_keystore)
        except (DecodeError, ExpiredSignatureError, ImmatureSignatureError, InvalidAudienceError, \
            InvalidIssuedAtError, InvalidIssuerError, MissingRequiredClaimError, InvalidKeyError) as e:
            raise AuthenticationFailed('Invalid token.')
        if result:
            payld_verified = result
            profile_id = payld_verified.get('profile')
            account = self.get_account(profile_id)
        else:
            raise AuthenticationFailed('Invalid token.')
        return (account, payld_verified)

    def authenticate_header(self, request):
        return self.keyword
## end of class


class RemoteAccessJWTauthentication(AccessJWTauthentication):
    def authenticate_credentials(self, encoded_acs_tok, audience):
        account, payld_verified = super().authenticate_credentials(encoded_acs_tok=encoded_acs_tok, audience=audience)
        priv_status = payld_verified['priv_status']
        if ROLE_ID_SUPERUSER == priv_status:
            account.is_superuser = True
            account.is_staff = True
        elif ROLE_ID_STAFF == priv_status:
            account.is_superuser = False
            account.is_staff = True
        else:
            account.is_superuser = False
            account.is_staff = False
        return (account, payld_verified)


class AnonymousUser:
    # mostly comes from Django auth app, this class is used for other Django apps which
    # do NOT install Django auth app
    id = None
    pk = None
    username = 'Anonymous'
    is_staff = False
    is_active = False
    is_superuser = False
    _groups = None ## EmptyManager(Group)
    _user_permissions = None ## EmptyManager(Permission)

    def __str__(self):
        return 'AnonymousUser'

    def __eq__(self, other):
        return isinstance(other, self.__class__)

    def __hash__(self):
        return 1  # instances always return the same hash value

    def __int__(self):
        raise TypeError('Cannot cast AnonymousUser to int. Are you trying to use it in place of User?')

    def save(self):
        raise NotImplementedError("Django doesn't provide a DB representation for AnonymousUser.")

    def delete(self):
        raise NotImplementedError("Django doesn't provide a DB representation for AnonymousUser.")

    def set_password(self, raw_password):
        raise NotImplementedError("Django doesn't provide a DB representation for AnonymousUser.")

    def check_password(self, raw_password):
        raise NotImplementedError("Django doesn't provide a DB representation for AnonymousUser.")

    @property
    def groups(self):
        return self._groups

    @property
    def user_permissions(self):
        return self._user_permissions

    def get_user_permissions(self, obj=None):
        ## return _user_get_permissions(self, obj, 'user')
        return set()

    def get_group_permissions(self, obj=None):
        return set()

    def get_all_permissions(self, obj=None):
        ## return _user_get_permissions(self, obj, 'all')
        return set()

    def has_perm(self, perm, obj=None):
        ## return _user_has_perm(self, perm, obj=obj)
        return False

    def has_perms(self, perm_list, obj=None):
        return all(self.has_perm(perm, obj) for perm in perm_list)

    def has_module_perms(self, module):
        ## return _user_has_module_perms(self, module)
        return False

    @property
    def is_anonymous(self):
        return True

    @property
    def is_authenticated(self):
        return False

    def get_username(self):
        return self.username

