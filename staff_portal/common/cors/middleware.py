from django.http        import  HttpResponse
from common.util.python import get_request_meta_key
from . import config as conf

ACCESS_CONTROL_REQUEST_METHOD  = 'access-control-request-method'
ACCESS_CONTROL_REQUEST_HEADERS = 'access-control-request-headers'

ACCESS_CONTROL_ALLOW_ORIGIN  = 'access-control-allow-origin'
ACCESS_CONTROL_ALLOW_METHODS = 'access-control-allow-methods'
ACCESS_CONTROL_ALLOW_HEADERS = 'access-control-allow-headers'
ACCESS_CONTROL_ALLOW_CREDENTIALS = 'access-control-allow-credentials'
ACCESS_CONTROL_MAX_AGE = 'access-control-max-age'


class CorsHeaderMiddleware:
    def __init__(self, get_response):
        self.get_response = get_response
        self._get_request_meta_key = get_request_meta_key

    def __call__(self, request):
        origin_key = self._get_request_meta_key('origin')
        origin = request.META.get(origin_key, None)
        host = "%s://%s" % (request.scheme, request.get_host())
        ##print('cors check, host = %s , origin = %s' % (host, origin))
        is_options_req = request.method == 'OPTIONS'
        is_cross_site_req = origin is not None and origin != host

        if is_cross_site_req:
            host_allowed, host_label = self._is_request_allowed(host=host, origin=origin)
            if is_options_req: # circuit-break the preflight (OPTIONS request)
                req_mthd_key = self._get_request_meta_key(ACCESS_CONTROL_REQUEST_METHOD)
                request_method = request.META.get(req_mthd_key, None)
                req_mthd_allowed = request_method in conf.ALLOWED_METHODS
                ##print('preflight, request_method = %s' % (request_method))
                response = HttpResponse(status='200') # 200 ok
                response['content-length'] = '0'
                response[ACCESS_CONTROL_MAX_AGE] = conf.PREFLIGHT_MAX_AGE
                response[ACCESS_CONTROL_ALLOW_HEADERS] = ', '.join(conf.ALLOWED_HEADERS)
                if req_mthd_allowed:
                    response[ACCESS_CONTROL_ALLOW_METHODS] = request_method
            else: # second flight of cross-origin request
                req_mthd_allowed = request.method in conf.ALLOWED_METHODS
                if host_allowed and req_mthd_allowed:
                    request.cors_host_label = host_label
                    response = self.get_response(request)
                else:
                    response = HttpResponse(status='401')
            if host_allowed :
                response[ACCESS_CONTROL_ALLOW_ORIGIN] = origin
                response[ACCESS_CONTROL_ALLOW_CREDENTIALS] = conf.ALLOW_CREDENTIALS
        else: # must be same-site request
            response = self.get_response(request)
        return response

    def _is_request_allowed(self, host, origin):
        _fn = lambda x: x[1] == host
        host_exists   = filter(_fn, conf.ALLOWED_ORIGIN.items())
        host_exists   = list(host_exists)
        origin_exists = origin in conf.ALLOWED_ORIGIN.values()
        label = host_exists[0][0] if host_exists else None
        return (any(host_exists) and origin_exists, label)


