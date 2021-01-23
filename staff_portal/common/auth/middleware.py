from datetime import datetime, timezone

from django.conf  import  settings
from django.middleware.csrf import rotate_token, CsrfViewMiddleware

from common.util.python  import get_header_name


# header name for CSRF token authentication
csrf_header_name = get_header_name(name=settings.CSRF_HEADER_NAME)

class ExtendedCsrfViewMiddleware(CsrfViewMiddleware):

    def _set_token(self, request, response):
        """
        * make expiry time of CSRF token as long as session expiry,
          since session expiry time depends on staff status of logged-in user
        * add extra parameters for identifying CSRF token in cookie
        """
        # In this project, CSRF token is generated for authenticated users only on new session creation,
        # it will NOT be refreshed again before session expiry
        backup = settings.CSRF_COOKIE_AGE
        sess_age = request.session.get_expiry_age()
        settings.CSRF_COOKIE_AGE = sess_age

        if settings.CSRF_USE_SESSIONS:
            pass
        else:
            response.set_cookie(
                key='csrf_header_name',  value=csrf_header_name,
                max_age=settings.CSRF_COOKIE_AGE,
                domain=settings.CSRF_COOKIE_DOMAIN,
                path=settings.CSRF_COOKIE_PATH,
                secure=settings.CSRF_COOKIE_SECURE,
                httponly=settings.CSRF_COOKIE_HTTPONLY,
                samesite=settings.CSRF_COOKIE_SAMESITE,
            )
            response.set_cookie(
                key='csrf_cookie_name',  value=settings.CSRF_COOKIE_NAME,
                max_age=settings.CSRF_COOKIE_AGE,
                domain=settings.CSRF_COOKIE_DOMAIN,
                path=settings.CSRF_COOKIE_PATH,
                secure=settings.CSRF_COOKIE_SECURE,
                httponly=settings.CSRF_COOKIE_HTTPONLY,
                samesite=settings.CSRF_COOKIE_SAMESITE,
            )
        super()._set_token(request=request, response=response)
        settings.CSRF_COOKIE_AGE = backup



