
def monkeypatch_django_error_view_production():
    import json
    from urllib.parse import quote
    from django.views import defaults
    from django.http import (HttpResponseBadRequest, HttpResponseForbidden, HttpResponseNotFound,
        HttpResponseServerError,)
    ERROR_404_TEMPLATE_NAME = '404.html'
    ERROR_403_TEMPLATE_NAME = '403.html'
    ERROR_400_TEMPLATE_NAME = '400.html'
    ERROR_500_TEMPLATE_NAME = '500.html'
    page_not_found_html = defaults.page_not_found
    server_error_html   = defaults.server_error

    def _collect_404_info(request, exception):
        exception_repr = exception.__class__.__name__
        # Try to get an "interesting" exception message, if any (and not the ugly
        # Resolver404 dictionary)
        try:
            message = exception.args[0]
        except (AttributeError, IndexError):
            pass
        else:
            if isinstance(message, str):
                exception_repr = message
        return {
            'request_path': quote(request.path),
            'exception': exception_repr,
        }

    def _collect_5xx_info(request):
        return {}

    def page_not_found_json(request, exception, **kwargs):
        body = _collect_404_info(request, exception)
        return HttpResponseNotFound(json.dumps(body), content_type='application/json')

    def server_error_json(request, **kwargs):
        body = _collect_5xx_info(request)
        return HttpResponseServerError(json.dumps(body), content_type='application/json')


    def _lookup_response_render_func(request, func_map, default_func):
        accept_list = request.headers['accept'].split(',')
        accept_list = list(map(lambda x: x.strip(), accept_list))
        fn = None
        for ct in accept_list:
            fn = func_map.get(ct, None)
            if fn:
                break
        if fn is None:
            # rollback to default response, which fixed to send text/html only
            fn = default_func
        return fn

    def patched_page_not_found(request, exception, template_name=ERROR_404_TEMPLATE_NAME):
        func_map = {
            'application/json': page_not_found_json,
            'text/html':        page_not_found_html,
        }
        fn = _lookup_response_render_func(request=request, default_func=page_not_found_html
                , func_map=func_map)
        return  fn(request=request, exception=exception, template_name=template_name)

    def patched_server_error(request, template_name=ERROR_500_TEMPLATE_NAME):
        func_map = {
            'application/json': server_error_json,
            'text/html':        server_error_html,
        }
        fn = _lookup_response_render_func(request=request, default_func=server_error_html
                , func_map=func_map)
        return  fn(request=request, template_name=template_name)

    if not hasattr(defaults.page_not_found , '_patched'):
        defaults.page_not_found  = patched_page_not_found
        setattr(defaults.page_not_found , '_patched', None)
    if not hasattr(defaults.server_error , '_patched'):
        defaults.server_error  = patched_server_error
        setattr(defaults.server_error , '_patched', None)
## end of monkeypatch_django_error_view_production




