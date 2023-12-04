from unittest.mock import Mock, patch

from django.test import SimpleTestCase
from django.http import HttpResponse

from common.util.python import get_request_meta_key
from common.cors.middleware import CorsHeaderMiddleware, conf as cors_conf, ACCESS_CONTROL_REQUEST_METHOD, ACCESS_CONTROL_REQUEST_HEADERS, ACCESS_CONTROL_ALLOW_ORIGIN,  ACCESS_CONTROL_ALLOW_METHODS, ACCESS_CONTROL_ALLOW_HEADERS, ACCESS_CONTROL_ALLOW_CREDENTIALS, ACCESS_CONTROL_MAX_AGE


# no need to initialize test database
# TODO, test referrer header
class CorsMiddlewareTestCase(SimpleTestCase):
    def setUp(self):
        self.expect_succeed_msg = 'CORS verification passed'
        mock_get_response = lambda req : HttpResponse(self.expect_succeed_msg)
        self.middleware = CorsHeaderMiddleware(get_response=mock_get_response)
        def _get_host():
            return 'localhost:8008'
        raw_headers = {}
        self.base_mock_request = Mock(path='/', scheme='http', META=raw_headers,
                get_host=_get_host, method='GET')


    def test_samesite_request(self):
        response = self.middleware(request=self.base_mock_request)
        self.assertEqual(int(response.status_code), 200)
        actual_value = response.content.decode()
        self.assertEqual(self.expect_succeed_msg, actual_value)

    def test_crosssite_preflight_invalid_origin(self):
        self.base_mock_request.method = 'OPTIONS'
        raw_headers = self.base_mock_request.META
        raw_headers['HTTP_ORIGIN'] = 'http://invalid.web.myproject.org'
        response = self.middleware(request=self.base_mock_request)
        headers = dict(response.items())
        headers_should_not_exist = (ACCESS_CONTROL_ALLOW_CREDENTIALS, ACCESS_CONTROL_ALLOW_METHODS, ACCESS_CONTROL_ALLOW_ORIGIN)
        for h_label in headers_should_not_exist:
            with self.assertRaises(KeyError):
                headers[h_label]

    def test_crosssite_preflight_invalid_method(self):
        self.base_mock_request.method = 'OPTIONS'
        raw_headers = self.base_mock_request.META
        raw_headers['HTTP_ORIGIN'] = cors_conf.ALLOWED_ORIGIN['web']
        key = get_request_meta_key(ACCESS_CONTROL_REQUEST_METHOD)
        raw_headers[key] = 'INVALID_METHOD'
        response = self.middleware(request=self.base_mock_request)
        headers = dict(response.items())
        with self.assertRaises(KeyError):
            headers[ACCESS_CONTROL_ALLOW_METHODS]

    def test_crosssite_preflight_ok(self):
        expect_req_mthd = 'POST'
        expect_origin = cors_conf.ALLOWED_ORIGIN['web']
        self.base_mock_request.method = 'OPTIONS'
        raw_headers = self.base_mock_request.META
        raw_headers['HTTP_ORIGIN'] = expect_origin
        key = get_request_meta_key(ACCESS_CONTROL_REQUEST_METHOD)
        raw_headers[key] = expect_req_mthd
        response = self.middleware(request=self.base_mock_request)
        self.assertEqual(int(response.status_code), 200)
        headers = dict(response.items())
        self.assertGreaterEqual(cors_conf.PREFLIGHT_MAX_AGE, int(headers[ACCESS_CONTROL_MAX_AGE]))
        expect_value = cors_conf.ALLOWED_HEADERS
        actual_value = headers[ACCESS_CONTROL_ALLOW_HEADERS].split(',')
        self.assertSetEqual(set(actual_value), set(expect_value))
        self.assertEqual(headers[ACCESS_CONTROL_ALLOW_ORIGIN], expect_origin)
        self.assertEqual(headers[ACCESS_CONTROL_ALLOW_METHODS], expect_req_mthd)


    def test_crosssite_2ndflight_invalid_origin(self):
        self.base_mock_request.method = 'PUT'
        raw_headers = self.base_mock_request.META
        raw_headers['HTTP_ORIGIN'] = 'http://invalid.web.myproject.org'
        response = self.middleware(request=self.base_mock_request)
        self.assertEqual(int(response.status_code), 401)
        headers = dict(response.items())
        headers_should_not_exist = (ACCESS_CONTROL_ALLOW_CREDENTIALS, ACCESS_CONTROL_ALLOW_ORIGIN)
        for h_label in headers_should_not_exist:
            with self.assertRaises(KeyError):
                headers[h_label]

    def test_crosssite_2ndflight_ok(self):
        expect_req_mthd = 'PUT'
        expect_origin = cors_conf.ALLOWED_ORIGIN['web']
        self.base_mock_request.method = expect_req_mthd
        raw_headers = self.base_mock_request.META
        raw_headers['HTTP_ORIGIN'] = expect_origin
        response = self.middleware(request=self.base_mock_request)
        self.assertEqual(int(response.status_code), 200)
        headers = dict(response.items())
        self.assertEqual(headers[ACCESS_CONTROL_ALLOW_ORIGIN], expect_origin)
        self.assertTrue(bool(headers[ACCESS_CONTROL_ALLOW_CREDENTIALS].lower()))
        actual_value = response.content.decode()
        self.assertEqual(self.expect_succeed_msg, actual_value)

