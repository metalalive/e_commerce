from wsgiref.util import is_hop_by_hop
from requests.status_codes import codes as requests_codes

from django.http.response          import HttpResponse
from django.views.generic.base     import View as DjangoView

from common.views.api import BaseLoginView, BaseLogoutView
from common.views.proxy.mixins import DjangoProxyRequestMixin

class LoginView(BaseLoginView):
    is_staff_only = True
    use_session   = True
    use_token     = True


class LogoutView(BaseLogoutView):
    use_session   = True
    use_token     = True


class BaseRevProxyView(DjangoView, DjangoProxyRequestMixin):
    authenticate_required = {
        'OPTIONS': False,
        'GET': False,
        'POST': False,
        'PUT': False,
        'PATCH': False,
        'DELETE': False,
    }
    _http405 = HttpResponse( content='{"reason":"method not allowed"}',
                content_type='application/json',
                status=requests_codes['method_not_allowed'] )

    def collect(self, request, **kwargs):
        out = super().collect(request, **kwargs)
        extra = {'method':request.method, }
        out.update(extra)
        return out

    def dispatch(self, request, *args, **kwargs):
        response = self._authenticate(request)
        if response:
            return response
        # perform proxy operation at here for  the paths set in urls.py
        pxy_req_kwargs = self.collect(request, **kwargs)
        ##print('check headers before forwarding to app server ? %s' % headers)
        pxy_resp = self.send(**pxy_req_kwargs)
        return self._get_django_response(proxy_response=pxy_resp)


    def _authenticate(self, request):
        auth_required = self.authenticate_required.get(request.method.upper(), None)
        if auth_required is True:
            user = getattr(request, 'user', None)
            if not user or not user.is_active:
                return HttpResponse(
                        content='{"reason":"authentication required"}',
                        content_type='application/json',
                        status=requests_codes['unauthorized'])
        elif auth_required is False:
            pass
        else:
            return self._http405

    def _get_django_response(self, proxy_response):
        status = proxy_response.status_code
        content_type = proxy_response.headers.get('content-type', None)
        body =  proxy_response.content
        response = HttpResponse(content=body, status=status, content_type=content_type)
        for name,value in proxy_response.headers.items() :
            if is_hop_by_hop(name):
                continue
            response[name.lower()] = value
        return response


