from datetime import datetime, timezone

from django.conf  import settings as django_settings
from django.utils.cache      import patch_vary_headers
from django.utils.decorators import method_decorator, decorator_from_middleware
from django.middleware.csrf  import CsrfViewMiddleware
from rest_framework.settings    import api_settings as drf_settings
from rest_framework.renderers   import TemplateHTMLRenderer, JSONRenderer
from rest_framework.response    import Response as RestResponse
from rest_framework             import status as RestStatus

from common.util.python  import get_header_name


# header name for CSRF token authentication
csrf_header_name = get_header_name(name=django_settings.CSRF_HEADER_NAME)

class ExtendedCsrfViewMiddleware(CsrfViewMiddleware):
    supported_renderer_classes = [TemplateHTMLRenderer, JSONRenderer]

    def _set_token(self, request, response):
        """
        For logged-in users
        * make expiry time of CSRF token as long as session expiry for authenticated
          accesses (e.g. logged-in user), or apply django.conf.settings.CSRF_COOKIE_AGE
          for unauth accesses.
        * add extra parameters for identifying CSRF token in cookie

        In this project:
        * CSRF token is generated for authenticated users only on new session creation,
          it will NOT be refreshed again before session expiry
        """
        if request.session.session_key is not None:
            tkn_age = request.session.get_expiry_age()
        else: # for unauthenticated accesses
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
            response.set_cookie(
                key='csrf_cookie_name',  value=django_settings.CSRF_COOKIE_NAME,  max_age=tkn_age ,
                domain=django_settings.CSRF_COOKIE_DOMAIN,
                path=django_settings.CSRF_COOKIE_PATH,
                secure=django_settings.CSRF_COOKIE_SECURE,
                httponly=django_settings.CSRF_COOKIE_HTTPONLY,
                samesite=django_settings.CSRF_COOKIE_SAMESITE,
            )
            # Set the Vary header since content varies with the CSRF cookie.
            patch_vary_headers(response, ('Cookie',))


    def _reject(self, request, reason):
        # may return json response if client required, default is HTML response
        renderers = [renderer() for renderer in self.supported_renderer_classes]
        conneg = drf_settings.DEFAULT_CONTENT_NEGOTIATION_CLASS()
        neg = conneg.select_renderer(request, renderers)
        accepted_renderer, accepted_media_type = neg
        if isinstance(accepted_renderer, TemplateHTMLRenderer):
            # fall back to django default HTML response on CSRF failure
            response = super()._reject(request=request, reason=reason)
        else: # json
            data = {drf_settings.NON_FIELD_ERRORS_KEY: [reason], }
            response = RestResponse(data=data, status=RestStatus.HTTP_403_FORBIDDEN)
        return response


csrf_protect_m = method_decorator(decorator_from_middleware(ExtendedCsrfViewMiddleware))

