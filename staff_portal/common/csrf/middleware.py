import logging

from django.conf   import settings as django_settings
from django.middleware.csrf  import CsrfViewMiddleware
from django.utils.cache      import patch_vary_headers
from django.utils.decorators import method_decorator, decorator_from_middleware
from django.http import JsonResponse
from django.core.exceptions import NON_FIELD_ERRORS

from common.util.python  import get_header_name, accept_mimetypes_lookup
from common.auth.django.login import _determine_expiry

# header name for CSRF token authentication
csrf_header_name = get_header_name(name=django_settings.CSRF_HEADER_NAME)

class ExtendedCsrfViewMiddleware(CsrfViewMiddleware):
    def _set_token(self, request, response):
        """
        For logged-in users
        * make expiry time of CSRF token as long as session/jwt expiry for authenticated
          accesses (e.g. logged-in user), or apply django.conf.settings.CSRF_COOKIE_AGE
          for unauth accesses.
        * add extra parameters for identifying CSRF token in cookie

        In this project:
        * CSRF token is generated for authenticated users only on setting up response
          and new session/jwt , it will NOT be refreshed again before the CSRF token expiry
        """
        ##if request.session.session_key is not None:
        ##    tkn_age = request.session.get_expiry_age()
        if request.user.is_authenticated :
            tkn_age = _determine_expiry(request.user)
        else:
            tkn_age = getattr(request, 'csrf_cookie_age', django_settings.CSRF_COOKIE_AGE)
        if django_settings.CSRF_USE_SESSIONS:
            if request.session.get(CSRF_SESSION_KEY) != request.META['CSRF_COOKIE']:
                request.session[CSRF_SESSION_KEY] = request.META['CSRF_COOKIE']
        else: # store CSRF token to client cookie, apply Double Submit cookie to validate the token
            response.set_cookie(
                key=django_settings.CSRF_COOKIE_NAME, value=request.META['CSRF_COOKIE'],  max_age=tkn_age,
                domain=django_settings.CSRF_COOKIE_DOMAIN,
                path=django_settings.CSRF_COOKIE_PATH,
                secure=django_settings.CSRF_COOKIE_SECURE,
                httponly=django_settings.CSRF_COOKIE_HTTPONLY,
                samesite=django_settings.CSRF_COOKIE_SAMESITE,
            )
            response.set_cookie(
                key='csrf_header_name',  value=csrf_header_name,  max_age=tkn_age ,
                domain=django_settings.CSRF_COOKIE_DOMAIN,
                path=django_settings.CSRF_COOKIE_PATH,
                secure=django_settings.CSRF_COOKIE_SECURE,
                httponly=django_settings.CSRF_COOKIE_HTTPONLY,
                samesite=django_settings.CSRF_COOKIE_SAMESITE,
            )
            # Set the Vary header since content varies with the CSRF cookie.
            patch_vary_headers(response, ('Cookie',))

    def _reject(self, request, reason):
        json_mimetypes = ['application/json', 'application/x-ndjson', 'application/*']
        result = accept_mimetypes_lookup(
                http_accept=request.headers.get('accept', ''),
                expected_types=json_mimetypes )
        if any(result): # json
            data = {NON_FIELD_ERRORS : [reason], }
            response = JsonResponse(data=data, status=403) # forbidden access
        else: # fall back to django default HTML response on CSRF failure
            response = super()._reject(request=request, reason=reason)
        return response


csrf_protect_m = method_decorator(decorator_from_middleware(ExtendedCsrfViewMiddleware))

