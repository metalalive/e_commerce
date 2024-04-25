import json
import time
from unittest.mock import Mock, patch

from django.test import SimpleTestCase
from django.http import HttpResponse
from django.conf import settings as django_settings
from django.middleware.csrf import rotate_token, REASON_NO_CSRF_COOKIE, REASON_BAD_TOKEN
from django.utils.http import http_date

from ecommerce_common.util import get_request_meta_key
from ecommerce_common.csrf.middleware import ExtendedCsrfViewMiddleware


class CsrfMiddlewareTestCase(SimpleTestCase):
    def setUp(self):
        self.middleware = ExtendedCsrfViewMiddleware()
        raw_headers = {}
        self.base_mock_request = Mock(
            META=raw_headers,
            COOKIES={},
            csrf_cookie_set=False,
            POST={},
            csrf_cookie_age=123,
            csrf_processing_done=False,
            _dont_enforce_csrf_checks=False,
            headers={"accept": "application/json"},
        )
        # refresh request.META['CSRF_COOKIE']
        rotate_token(request=self.base_mock_request)

    def test_set_token_unauth_user(self):
        from django.contrib.auth.models import AnonymousUser

        mock_request = self.base_mock_request
        mock_request.user = AnonymousUser()
        django_settings.CSRF_COOKIE_HTTPONLY = False
        response_before = HttpResponse()
        response_after = self.middleware.process_response(
            request=mock_request, response=response_before
        )
        self._check_response_cookie(
            mock_request,
            response=response_after,
            expect_max_age=mock_request.csrf_cookie_age,
            expect_httponly_status="",
        )

    def test_set_token_auth_user(self):
        mock_request = self.base_mock_request
        mock_request.user.pk = 12
        mock_request.user.is_authenticated = True
        mock_request.user.is_active = True
        mock_request.user.is_staff = True
        mock_request.user.is_superuser = True
        response_before = HttpResponse()
        django_settings.CSRF_COOKIE_HTTPONLY = True
        response_after = self.middleware.process_response(
            request=mock_request, response=response_before
        )
        self._check_response_cookie(
            mock_request,
            response=response_after,
            expect_max_age=django_settings.SESSION_COOKIE_AGE,
            expect_httponly_status=True,
        )

    def _check_response_cookie(
        self, mock_request, response, expect_max_age, expect_httponly_status
    ):
        self.assertNotEqual(response.cookies.get("csrf_header_name"), None)
        csrf_cookie_name = django_settings.CSRF_COOKIE_NAME
        cookie_obj = response.cookies[csrf_cookie_name]
        self.assertEqual(cookie_obj["max-age"], expect_max_age)
        actual_tok = cookie_obj.value
        expect_tok = mock_request.META["CSRF_COOKIE"]
        self.assertEqual(expect_tok, actual_tok)
        expect_expiry = http_date(time.time() + cookie_obj["max-age"])
        actual_expiry = cookie_obj["expires"]
        self.assertEqual(expect_expiry, actual_expiry)
        # httponly is off by default in Django, frontend JavaScript can access the token
        # and construct CSRF token header for subsequent unsafe requests (e.g. POST, DELETE)
        self.assertEqual(cookie_obj["httponly"], expect_httponly_status)
        # TODO, check the fields : 'domain', 'samesite'

    def test_verify_token_not_found(self):
        mock_request = self.base_mock_request
        mock_request.method = "DELETE"
        mock_request.is_secure.return_value = (
            False  # TODO, test for is_secure() is True
        )
        mock_callback = Mock(csrf_exempt=False)
        response = self.middleware.process_view(
            request=mock_request,
            callback=mock_callback,
            callback_args=None,
            callback_kwargs=None,
        )
        self.assertEqual(int(response.status_code), 403)
        err_info = json.loads(response.content.decode())
        self.assertIn(REASON_NO_CSRF_COOKIE, err_info["__all__"])

    def test_verify_invalid_cookie_token(self):
        mock_request = self.base_mock_request
        mock_request.method = "PUT"
        mock_request.is_secure.return_value = (
            False  # TODO, test for is_secure() is True
        )
        mock_request.csrf_cookie_needs_reset = False
        mock_request.COOKIES[django_settings.CSRF_COOKIE_NAME] = (
            "invalid_csrf_token_in_cookie"
        )
        mock_callback = Mock(csrf_exempt=False)
        response = self.middleware.process_view(
            request=mock_request,
            callback=mock_callback,
            callback_args=None,
            callback_kwargs=None,
        )
        self.assertEqual(int(response.status_code), 403)
        err_info = json.loads(response.content.decode())
        self.assertIn(REASON_BAD_TOKEN, err_info["__all__"])
        self.assertTrue(mock_request.csrf_cookie_needs_reset)

    def test_verify_missing_header_token(self):
        mock_request = self.base_mock_request
        mock_request.method = "POST"
        mock_request.is_secure.return_value = (
            False  # TODO, test for is_secure() is True
        )
        mock_request.csrf_cookie_needs_reset = False
        mock_request.COOKIES[django_settings.CSRF_COOKIE_NAME] = mock_request.META[
            "CSRF_COOKIE"
        ]
        mock_callback = Mock(csrf_exempt=False)
        response = self.middleware.process_view(
            request=mock_request,
            callback=mock_callback,
            callback_args=None,
            callback_kwargs=None,
        )
        self.assertEqual(int(response.status_code), 403)
        err_info = json.loads(response.content.decode())
        self.assertIn(REASON_BAD_TOKEN, err_info["__all__"])
        self.assertFalse(mock_request.csrf_cookie_needs_reset)

    def test_verify_token_ok(self):
        mock_request = self.base_mock_request
        mock_request.method = "POST"
        mock_request.is_secure.return_value = (
            False  # TODO, test for is_secure() is True
        )
        mock_request.csrf_cookie_needs_reset = False
        expect_tok = mock_request.META["CSRF_COOKIE"]
        mock_request.COOKIES[django_settings.CSRF_COOKIE_NAME] = expect_tok
        mock_request.META[django_settings.CSRF_HEADER_NAME] = expect_tok
        mock_callback = Mock(csrf_exempt=False)
        # process_view() is where the middleware verifies both of the CSRF tokens
        # from cookie and header
        response = self.middleware.process_view(
            request=mock_request,
            callback=mock_callback,
            callback_args=None,
            callback_kwargs=None,
        )
        self.assertIsNone(response)
        self.assertTrue(mock_request.csrf_processing_done)


## end of class CsrfMiddlewareTestCase
