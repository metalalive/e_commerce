import urllib
import json

from django.test import Client as DjangoTestClient


class _BaseMockTestClientInfoMixin:
    stored_models = {}
    _json_mimetype = 'application/json'
    _client = DjangoTestClient(enforce_csrf_checks=False, HTTP_ACCEPT=_json_mimetype)
    _forwarded_pattern = 'by=proxy_api_gateway;for=%s;host=testserver;proto=http'

    def _send_request_to_backend(self, path, method='post', body=None, expect_shown_fields=None,
            ids=None, extra_query_params=None, enforce_csrf_checks=True, headers=None, cookies=None):
        if body is not None:
            body = json.dumps(body).encode()
        query_params = {}
        if extra_query_params:
            query_params.update(extra_query_params)
        if expect_shown_fields:
            query_params['fields'] = ','.join(expect_shown_fields)
        if ids:
            ids = tuple(map(str, ids))
            query_params['ids'] = ','.join(ids)
        querystrings = urllib.parse.urlencode(query_params)
        path_with_querystring = '%s?%s' % (path, querystrings)
        send_fn = getattr(self._client, method)
        headers = headers or {}
        cookies = cookies or {}
        bak_csrf_checks = self._client.handler.enforce_csrf_checks
        self._client.cookies.load(cookies) # do not use update()
        self._client.handler.enforce_csrf_checks = enforce_csrf_checks
        response = send_fn(path=path_with_querystring, data=body, content_type=self._json_mimetype,
                       **headers)
        self._client.handler.enforce_csrf_checks = bak_csrf_checks
        return response
## end of class _BaseMockTestClientInfoMixin

