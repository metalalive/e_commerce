from datetime import datetime, timedelta
from importlib import import_module
import logging

from django.conf import settings as django_settings
from django.utils import timezone as django_timezone
from django.middleware.csrf import rotate_token
from django.contrib.auth import (
    login as _sessionid_based_login,
    logout as sessionid_based_logout,
)
from django.contrib.auth.models import AnonymousUser
from rest_framework.request import Request as DRFRequest

from .utils import _determine_expiry, jwt_httpreq_verify

_logger = logging.getLogger(__name__)


def sessionid_based_login(request, user, backend, log_args=None):
    init_expiry_secs = _determine_expiry(user=user)
    sess_engine = import_module(django_settings.SESSION_ENGINE)
    http_req = request._request if isinstance(request, DRFRequest) else request
    http_req.session = (
        sess_engine.SessionStore()
    )  # create new empty session only on login success
    http_req.session.set_expiry(value=init_expiry_secs)
    _sessionid_based_login(request=request, user=user, backend=backend)
    if log_args:
        log_args.extend(
            [
                "init_expiry_secs",
                init_expiry_secs,
            ]
        )


def jwt_based_login(request, user):
    if user is None:
        user = request.user
    # overwrite existing JWT or create new one
    # avoid frontend sends login request multiple times
    refresh_jwt = request.COOKIES.get(django_settings.JWT_NAME_REFRESH_TOKEN, None)
    if not refresh_jwt:  # the value may be null or empty string
        from ..jwt import JWT

        token = JWT()
        max_age = _determine_expiry(user=user)
        issued_at = django_timezone.now()  # datetime.utcnow()
        expires = issued_at + timedelta(seconds=max_age)
        # exp & iat field must be NumericDate, see section 4.1.4 , RFC7519
        default_payld = {
            "exp": expires,
            "iat": issued_at,
            "iss": "YOUR_ISSUER",
            "profile": user.profile.id,
        }
        default_header = {}
        token.default_claims(header_kwargs=default_header, payld_kwargs=default_payld)
        # refresh jwt doesn't include `aud` field
        refresh_jwt = token
        # refresh CSRF token to DRF request, it would write new CSRF token to META dictionary
        rotate_token(request)
    else:  # use the original token
        refresh_jwt = None
    if hasattr(request, "user"):
        request.user = user
    return refresh_jwt

