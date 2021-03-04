from datetime  import datetime
from importlib import import_module
import logging

from django.conf  import settings as django_settings
from django.middleware.csrf import rotate_token
from django.contrib.auth  import  _get_backends, login as _sessionid_based_login, logout as sessionid_based_logout

_logger = logging.getLogger(__name__)
sess_engine = import_module(django_settings.SESSION_ENGINE)

def _determine_expiry(user):
    """
    determine session expiry time in seconds dynamically for different status of users
    e.g. superuser, staff, customers
    """
    # TODO: find better way to idoing this
    if user.is_superuser:
        expiry_secs = django_settings.SESSION_COOKIE_AGE
    elif  user.is_staff:
        expiry_secs = django_settings.SESSION_COOKIE_AGE << 1
    else:
        expiry_secs = django_settings.SESSION_COOKIE_AGE << 2
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
    from rest_framework.request  import Request as DRFRequest
    http_req = request._request if isinstance(request, DRFRequest) else request
    http_req.session = sess_engine.SessionStore() # create new empty session only on login success
    http_req.session.set_expiry(value=init_expiry_secs)
    _sessionid_based_login(request=request, user=user, backend=backend)
    log_args.extend(['init_expiry_secs', init_expiry_secs,])


def jwt_based_login(request, user, backend):
    from rest_framework.request  import Request as DRFRequest
    from .middleware import JWT
    if user is None:
        user = request.user
    backend = _get_backend(user, backend)
    acc_id = user._meta.pk.value_to_string(user)
    _jwt = getattr(request, 'jwt', JWT()) # overwrite existing JWT or create new one
    _jwt.payload['iat'] = datetime.utcnow()
    _jwt.payload['acc_id'] = acc_id
    _jwt.payload['bkn_id'] = backend
    ##print('backend: %s , _jwt.payload : %s' % (backend, _jwt.payload))
    http_req = request._request if isinstance(request, DRFRequest) else request
    if not hasattr(http_req, 'jwt'):
        http_req.jwt = _jwt
    ##print('http_req: %s, http_req.jwt at login() : %s' % (http_req, http_req.jwt))
    if hasattr(request, 'user'):
        request.user = user
    rotate_token(request) # CSRF token refresh, or http_req ?
    # TODO: emit signal at the end of login


def logout(request, use_token, use_session):
    if use_session:
        from rest_framework.request  import Request as DRFRequest
        http_req = request._request if isinstance(request, DRFRequest) else request
        session_key = http_req.COOKIES.get(django_settings.SESSION_COOKIE_NAME, None)
        if session_key: # load existing session only on logout
            http_req.session = sess_engine.SessionStore(session_key)
            sessionid_based_logout(request=request)
    if use_token:
        jwt_based_logout(request=request)


def jwt_based_logout(request):
    from rest_framework.request  import Request as DRFRequest
    from .middleware import jwt_httpreq_verify
    http_req = request._request if isinstance(request, DRFRequest) else request
    if hasattr(http_req, 'jwt'):
        if http_req.jwt.valid is True:
            # already verified successfully at middleware level
            http_req.jwt.destroy = True
    else:
        payld = jwt_httpreq_verify(request=http_req)
        if payld and http_req.jwt.valid is True: # verified successfully
            http_req.jwt.destroy = True
    if hasattr(request, 'user'):
        from django.contrib.auth.models import AnonymousUser
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


def _monkey_patch_baseusermgr():
    """
    monkey patch BaseUserManager.get_queryset at server startup,
    because I attempt to minimize access permission to those django application
    server which are not for user/account management
    """
    from django.contrib.auth.base_user import BaseUserManager
    origin_get_qset = BaseUserManager.get_queryset

    def monkey_patch_get_queryset(self):
        qset = origin_get_qset(self)
        only_list = ['id','last_login', 'username']
        qset = qset.only(*only_list)
        log_args = ['raw_sql', str(qset.query)]
        _logger.debug(None, *log_args)
        return qset

    is_usermgt_service = 'usermgt_service' in  django_settings.DATABASES.keys()
    if not is_usermgt_service:
        BaseUserManager.get_queryset = monkey_patch_get_queryset


_monkey_patch_baseusermgr()


