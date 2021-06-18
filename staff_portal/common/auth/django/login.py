from datetime  import datetime
from importlib import import_module
import logging

from django.conf  import settings as django_settings
from django.middleware.csrf import rotate_token
from django.contrib.auth  import  _get_backends, login as _sessionid_based_login, logout as sessionid_based_logout
from django.contrib.auth.models import User as AuthUser, AnonymousUser
from rest_framework.request  import Request as DRFRequest

from ..jwt import JWT
from .middleware import jwt_httpreq_verify, gen_jwt_token_set

_logger = logging.getLogger(__name__)
sess_engine = import_module(django_settings.SESSION_ENGINE)

def _determine_expiry(user):
    """
    determine session expiry time in seconds dynamically for different status of users
    e.g. superuser, staff, customers
    """
    # TODO: find better way to doing this
    if user and isinstance(user, AuthUser):
        if user.is_superuser:
            expiry_secs = django_settings.SESSION_COOKIE_AGE
        elif  user.is_staff:
            expiry_secs = django_settings.SESSION_COOKIE_AGE << 2
        else:
            expiry_secs = django_settings.SESSION_COOKIE_AGE << 4
    else:
        expiry_secs = -1
    return expiry_secs

def login(request, user, backend=None, use_session=False, use_token=False):
    """
    extended from original Django login() , provide 2 ways of login mechanism :
    typical sessionID-based login, and JWT-based login
    the lifetime of both are the same.
    """
    log_args = ['account_id', user.pk, 'use_session',use_session, 'use_token', use_token]
    if use_session:
        sessionid_based_login(request=request, user=user, backend=backend, log_args=log_args)
    if use_token:
        jwt_based_login(request=request, user=user, backend=backend)
    _logger.debug(None, *log_args)


def sessionid_based_login(request, user, backend, log_args):
    init_expiry_secs = _determine_expiry(user=user)
    http_req = request._request if isinstance(request, DRFRequest) else request
    http_req.session = sess_engine.SessionStore() # create new empty session only on login success
    http_req.session.set_expiry(value=init_expiry_secs)
    _sessionid_based_login(request=request, user=user, backend=backend)
    log_args.extend(['init_expiry_secs', init_expiry_secs,])


def jwt_based_login(request, user, backend):
    if user is None:
        user = request.user
    backend = _get_backend(user, backend)
    acc_id = user._meta.pk.value_to_string(user)
    now_time = datetime.utcnow()
    # overwrite existing JWT or create new one
    http_req = request._request if isinstance(request, DRFRequest) else request
    if not hasattr(http_req, 'jwt'):
        http_req.jwt = gen_jwt_token_set(acs=None, rfr=None, user=None)
    if not http_req.jwt.user:
        http_req.jwt.user = user
    for tok_name in http_req.jwt.valid_token_names:
        if getattr(http_req.jwt , tok_name, None) is None:
            token = JWT()
            token.payload['iat'] = now_time
            token.payload['acc_id'] = acc_id
            token.payload['bkn_id'] = backend
            # `aud` field has to be consistent with `ALLOWED_ORIGIN`
            # attribute in ./common/data/cors.json
            token.payload['aud'] = ['api','usermgt', 'product']
            setattr(http_req.jwt, tok_name, token)
    if hasattr(request, 'user'):
        request.user = user
    rotate_token(request) # CSRF token refresh, or http_req ?
    # TODO: emit signal at the end of login


def logout(request, use_token, use_session):
    if use_session:
        http_req = request._request if isinstance(request, DRFRequest) else request
        session_key = http_req.COOKIES.get(django_settings.SESSION_COOKIE_NAME, None)
        if session_key: # load existing session only on logout
            http_req.session = sess_engine.SessionStore(session_key)
            sessionid_based_logout(request=request)
    if use_token:
        jwt_based_logout(request=request)


def jwt_based_logout(request):
    http_req = request._request if isinstance(request, DRFRequest) else request
    if hasattr(http_req, 'jwt'):
        if http_req.jwt.valid is True:
            # already verified successfully at middleware level
            http_req.jwt.destroy = True
    else:
        payld = jwt_httpreq_verify(request=http_req)
        if payld: # verified successfully
            http_req.jwt.destroy = True
    if hasattr(request, 'user'):
        request.user = AnonymousUser()
    # TODO: emit signal at the end of logout


def _get_backend(user, backend):
    try:
        backend = backend or user.backend
    except AttributeError:
        backends = _get_backends(return_tuples=True)
        if len(backends) == 1:
            _, backend = backends[0]
        else:
            raise ValueError(
                'You have multiple authentication backends configured and '
                'therefore must provide the `backend` argument or set the '
                '`backend` attribute on the user.'
            )
    else:
        if not isinstance(backend, str):
            raise TypeError('backend must be a dotted import path string (got %r).' % backend)
    return backend


def monkeypatch_baseusermgr():
    """
    monkey patch BaseUserManager.get_queryset at server startup,
    because I attempt to minimize access permission to those django application
    server which are not for user/account management
    """
    from django.contrib.auth.base_user import BaseUserManager
    origin_get_qset = BaseUserManager.get_queryset

    def monkey_patch_get_queryset(self):
        qset = origin_get_qset(self)
        only_list = ['id','last_login']
        qset = qset.only(*only_list)
        log_args = ['raw_sql', str(qset.query)]
        _logger.debug(None, *log_args)
        return qset

    is_usermgt_service = 'usermgt_service' in  django_settings.DATABASES.keys()
    if not is_usermgt_service and not hasattr(BaseUserManager.get_queryset , '_patched'):
        BaseUserManager.get_queryset = monkey_patch_get_queryset
        setattr(BaseUserManager.get_queryset , '_patched', None)


