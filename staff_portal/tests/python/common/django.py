import urllib
import json

from django.test import Client as DjangoTestClient


class _BaseMockTestClientInfoMixin:
    stored_models = {}
    _json_mimetype = 'application/json'
    _client = DjangoTestClient(enforce_csrf_checks=False, HTTP_ACCEPT=_json_mimetype)
    _forwarded_pattern = 'by=proxy_api_gateway;for=%s;host=testserver;proto=http'

    def _send_request_to_backend(self, path, method='post', body=None, expect_shown_fields=None,
            ids=None, extra_query_params=None, headers=None):
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
        return send_fn(path=path_with_querystring, data=body,  content_type=self._json_mimetype,
               **headers)
## end of class _BaseMockTestClientInfoMixin

