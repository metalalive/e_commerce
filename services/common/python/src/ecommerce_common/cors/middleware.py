import logging
from . import config as conf

_logger = logging.getLogger(__name__)

ACCESS_CONTROL_REQUEST_METHOD = "access-control-request-method"
ACCESS_CONTROL_REQUEST_HEADERS = "access-control-request-headers"

ACCESS_CONTROL_ALLOW_ORIGIN = "access-control-allow-origin"
ACCESS_CONTROL_ALLOW_METHODS = "access-control-allow-methods"
ACCESS_CONTROL_ALLOW_HEADERS = "access-control-allow-headers"
ACCESS_CONTROL_ALLOW_CREDENTIALS = "access-control-allow-credentials"
ACCESS_CONTROL_MAX_AGE = "access-control-max-age"


class CorsHeaderMiddleware:
    def __init__(self, get_response):
        from django.http import HttpResponse
        from ecommerce_common.util import get_request_meta_key

        self.get_response = get_response
        self._get_request_meta_key = get_request_meta_key
        self._default_response_cls = HttpResponse

    def __call__(self, request):
        origin_key = self._get_request_meta_key("origin")
        origin = request.META.get(origin_key, None)
        host = "%s://%s" % (request.scheme, request.get_host())
        ##print('cors check, host = %s , origin = %s' % (host, origin))
        is_options_req = request.method == "OPTIONS"
        is_cross_site_req = origin is not None and origin != host

        _logger.debug(
            None, "is_cross_site_req", is_cross_site_req, "is_options_req",
            is_options_req,"host", host, "origin", origin
        )
        if is_cross_site_req:
            host_allowed, host_label = _is_request_allowed(host=host, origin=origin)
            if is_options_req:  # circuit-break the preflight (OPTIONS request)
                response = self._gen_preflight_response(request)
            else:  # second flight of cross-origin request
                response = self._gen_second_flight_response(
                    request, host_allowed, host_label
                )
            if host_allowed:
                response[ACCESS_CONTROL_ALLOW_ORIGIN] = origin
                response[ACCESS_CONTROL_ALLOW_CREDENTIALS] = str(
                    conf.ALLOW_CREDENTIALS
                ).lower()
        else:  # must be same-site request
            response = self.get_response(request)
        return response

    def _gen_preflight_response(self, request):
        req_mthd_key = self._get_request_meta_key(ACCESS_CONTROL_REQUEST_METHOD)
        request_method = request.META.get(req_mthd_key, None)
        req_mthd_allowed = request_method in conf.ALLOWED_METHODS
        response = self._default_response_cls(status="200")  # 200 ok
        response["content-length"] = "0"
        response[ACCESS_CONTROL_MAX_AGE] = conf.PREFLIGHT_MAX_AGE
        response[ACCESS_CONTROL_ALLOW_HEADERS] = ",".join(conf.ALLOWED_HEADERS)
        if req_mthd_allowed:
            response[ACCESS_CONTROL_ALLOW_METHODS] = request_method
        return response

    def _gen_second_flight_response(self, request, host_allowed, host_label):
        req_mthd_allowed = request.method in conf.ALLOWED_METHODS
        _logger.debug(None, "req_mthd_allowed", req_mthd_allowed, "host_allowed", host_allowed)
        if host_allowed and req_mthd_allowed:
            request.cors_host_label = host_label
            response = self.get_response(request)
        else:
            response = self._default_response_cls(status="401")
        return response


## end of class CorsHeaderMiddleware


def _is_request_allowed(host, origin):
    _fn = lambda x: x[1] == host
    host_exists = filter(_fn, conf.ALLOWED_ORIGIN.items())
    host_exists = list(host_exists)
    origin_exists = origin in conf.ALLOWED_ORIGIN.values()
    label = host_exists[0][0] if host_exists else None
    return (any(host_exists) and origin_exists, label)
