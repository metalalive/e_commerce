import json
from http.cookies import Morsel
from unittest.mock import Mock, patch
from tempfile import SpooledTemporaryFile

from django.conf import settings as django_settings
from django.test import TransactionTestCase, Client as DjangoTestClient
from django.db   import DEFAULT_DB_ALIAS
from django.contrib.auth.models import User as AuthUser
from jwt.exceptions import InvalidAudienceError

from common.util.python import get_header_name, import_module_string
from common.auth.keystore import create_keystore_helper
from common.auth.jwt import JWT
from common.views.proxy.mixins import  RemoteGetProfileIDMixin
from common.util.python.messaging.rpc import RpcReplyEvent

# in this project, all test cases that require database accesses will make
# Django create separate / empty database only for testing purpose.

_csrf_value = 'aNvypJVzkTUUBqs9G8I41RWvSAagIQktQ3fn91WyyAiHvIcEoWE0HFD96gz028ol'
_cookie_csrf = {
    'anticsrftok': _csrf_value,
    'csrf_header_name': get_header_name(name=django_settings.CSRF_HEADER_NAME),
    'csrf_cookie_name': django_settings.CSRF_COOKIE_NAME,
}

_header_csrf = {django_settings.CSRF_HEADER_NAME: _csrf_value}

_keystore = create_keystore_helper(cfg=django_settings.AUTH_KEYSTORE,
        import_fn=import_module_string)



class BaseAuthenticationTestFixture:
    databases = {DEFAULT_DB_ALIAS, 'default'} # always use default DB alias for testing
    _uname = 'YOUR_USERNAME'
    _passwd = 'YOUR_PASSWORD'
    _json_mimetype = 'application/json'
    _default_user_info = {'id': 5, 'username':_uname, 'password': _passwd,
            'is_active':False, 'is_superuser':False, 'is_staff':False, }
    _default_login_body = {'username': _uname, 'password': _passwd, }

    def _init_mock_rpc_get_profile(self):
        fake_amqp_msg = {'result': {}, 'status': RpcReplyEvent.status_opt.FAIL_CONN}
        mock_rpc_reply_evt = RpcReplyEvent(listener=None, timeout_s=1)
        mock_rpc_reply_evt.send(fake_amqp_msg)
        RemoteGetProfileIDMixin._usermgt_rpc = Mock()
        RemoteGetProfileIDMixin._usermgt_rpc.get_profile.return_value = mock_rpc_reply_evt

    def setUp(self):
        self._client = DjangoTestClient(enforce_csrf_checks=True,
                HTTP_ACCEPT=self._json_mimetype)
        self._usr = AuthUser.objects.create_user(**self._default_user_info)
        self._init_mock_rpc_get_profile()

    def tearDown(self):
        if self._client.session:
            self._client.session.delete()
        self._client.cookies.clear()
        AuthUser.objects.filter(username=self._uname).delete()
        self._usr = None

    def _send_req(self, headers:dict, path:str, body=None, method='post',
            enable_cookie_csrf=False):
        if enable_cookie_csrf:
            self._update_cookie_csrf()
        fn = getattr(self._client, method.lower())
        response = fn(path=path, data=body, content_type=self._json_mimetype, **headers)
        return response

    def _update_cookie_csrf(self):
        for k,v in _cookie_csrf.items():
            self._client.cookies[k] = v
        #self._client.cookies.update(_cookie_csrf)


class LoginTestCase(BaseAuthenticationTestFixture, TransactionTestCase):
    _path = '/login'

    def test_no_csrf_failure(self):
        response = self._send_req(headers={}, body={}, path=self._path)
        self.assertEqual(int(response.status_code) , 403)
        self.assertIn(b'CSRF cookie not set', response.content)

    def test_inactive_failure(self):
        response = self._send_req(headers=_header_csrf, path=self._path,
                body=self._default_login_body, enable_cookie_csrf=True)
        self.assertIn(b'authentication failure', response.content)
        self._verify_failure(response)

    def test_nonstaff_failure(self):
        self._usr.is_active = True
        self._usr.save()
        response = self._send_req(headers=_header_csrf, path=self._path,
                body=self._default_login_body, enable_cookie_csrf=True)
        self._verify_failure(response)

    def test_staff_succeed(self):
        self._usr.is_active = True
        self._usr.is_staff = True
        self._usr.save()
        response = self._send_req(headers=_header_csrf, path=self._path,
                body=self._default_login_body, enable_cookie_csrf=True)
        self._verify_succeed(response)
        self._verify_get_profile_stat()

    def test_superuser_succeed(self):
        self._usr.is_active = True
        self._usr.is_staff = True
        self._usr.is_superuser = True
        self._usr.save()
        response = self._send_req(headers=_header_csrf, path=self._path,
                body=self._default_login_body, enable_cookie_csrf=True)
        self._verify_succeed(response)
        self._verify_get_profile_stat()

    def _verify_failure(self, response):
        self.assertEqual(int(response.status_code) , 401)
        jwt_cookie = self._client.cookies.get('jwt', None)
        sessid_cookie = self._client.cookies.get('sessionid', None)
        self.assertEqual(jwt_cookie , None)
        self.assertEqual(sessid_cookie , None)

    def _verify_succeed(self, response):
        self.assertEqual(int(response.status_code) , 200)
        self.assertEqual(response.exc_info , None)
        sessid_cookie = self._client.cookies.get('sessionid', None)
        jwt_access_token  = self._client.cookies.get('jwt_access_token', None)
        jwt_refresh_token = self._client.cookies.get('jwt_refresh_token', None)
        self.assertNotEqual(jwt_access_token   , None)
        self.assertNotEqual(jwt_refresh_token  , None)
        self.assertNotEqual(sessid_cookie , None)
        self.assertEqual(type(jwt_access_token ) , Morsel)
        self.assertEqual(type(jwt_refresh_token) , Morsel)
        # verify jwt
        self._verify_recv_jwt(encoded=jwt_access_token.value)
        self._verify_recv_jwt(encoded=jwt_refresh_token.value)
        # the session comes from Django built-in function, no need to test

    def _verify_recv_jwt(self, encoded):
        _jwt = JWT(encoded=encoded)
        self.assertEqual(_jwt.header['typ'] , 'JWT')
        self.assertIn(_jwt.header['alg'] , ('RS256', 'RS384', 'RS512'))
        payld_unverified = _jwt.payload
        with self.assertRaises(InvalidAudienceError):
            payld_verified = _jwt.verify(keystore=_keystore, audience=None, raise_if_failed=True)
        with self.assertRaises(InvalidAudienceError):
            payld_verified = _jwt.verify(keystore=_keystore, audience=['sale', 'payment'], raise_if_failed=True)
        payld_verified = _jwt.verify(keystore=_keystore, audience=[], raise_if_failed=False)
        self.assertEqual(payld_verified , None)
        allowed_app_labels = ('api', 'usermgt', 'product')
        for app_label in allowed_app_labels:
            payld_verified = _jwt.verify(keystore=_keystore, audience=[app_label], raise_if_failed=True)
            self.assertNotEqual(payld_verified , None)
            self.assertEqual(payld_verified , payld_unverified)

    def _verify_get_profile_stat(self):
        mock_obj = RemoteGetProfileIDMixin._usermgt_rpc.get_profile
        get_profile_expected_call_cnt = 1
        get_profile_actual_call_cnt = mock_obj.call_count
        self.assertEqual(get_profile_expected_call_cnt , get_profile_actual_call_cnt)
        call_args = mock_obj.call_args
        self.assertIn('api', call_args.kwargs['services_label'])
        self.assertIn('id',  call_args.kwargs['field_names'])
## end of class LoginTestCase


class LogoutTestCase(BaseAuthenticationTestFixture, TransactionTestCase):
    _path = '/logout'

    def test_no_login_failure(self):
        headers = _header_csrf
        response = self._client.post(path=self._path, data='',
                content_type=self._json_mimetype, **headers)
        self.assertEqual(int(response.status_code) , 401)

    def test_succeed(self):
        self._usr.is_active = True
        self._usr.is_staff = True
        self._usr.save()
        login_response = self._send_req(headers=_header_csrf, path='/login',
                body=self._default_login_body, enable_cookie_csrf=True)
        self.assertEqual(int(login_response.status_code) , 200)
        sessid_cookie = self._client.cookies.get('sessionid', None)
        self.assertNotEqual(sessid_cookie.value , '')

        self._update_cookie_csrf()
        headers = _header_csrf
        response = self._client.post(path=self._path, data='',
                content_type=self._json_mimetype, **headers)
        self.assertEqual(int(response.status_code) , 200)
        sessid_cookie = self._client.cookies.get('sessionid', None)
        self.assertEqual(sessid_cookie.value , '')
        jwt_access_token  = self._client.cookies.get('jwt_access_token', None)
        jwt_refresh_token = self._client.cookies.get('jwt_refresh_token', None)
        self.assertEqual(type(jwt_access_token ) , Morsel)
        self.assertEqual(type(jwt_refresh_token) , Morsel)
        self.assertIn(jwt_access_token.value   , ('', None))
        self.assertIn(jwt_refresh_token.value  , ('', None))


class PrivRevProxyTestCase(BaseAuthenticationTestFixture, TransactionTestCase):
    def _login(self):
        login_response = self._send_req(headers=_header_csrf, path='/login',
                body=self._default_login_body, enable_cookie_csrf=True)
        self.assertEqual(int(login_response.status_code) , 200)

    def _logout(self):
        logout_response = self._send_req(headers=_header_csrf, path='/logout',
                body={}, enable_cookie_csrf=True)
        self.assertEqual(int(logout_response.status_code) , 200)

    def setUp(self): # note this test case class doesn't cover authorization validation
        super().setUp()
        self._usr.is_active = True
        self._usr.is_staff = True
        self._usr.save()
        self._login()

    def tearDown(self):
        self._logout()
        super().tearDown()

    def test_nonexist_endpoint(self):
        pxy_response = self._send_req(headers=_header_csrf, path='/non_exist_path',
                body={}, method='GET', enable_cookie_csrf=True)
        self.assertEqual(int(pxy_response.status_code) , 404)
        self.assertEqual(pxy_response.exc_info , None)

    def test_endpoint_unavailable(self):
        # downstream services will NOT turn on during the tests, in this case it is normal
        # to receive bad gateway status (502) within the backend's response.
        pxy_response = self._send_req(headers=_header_csrf, path='/usermgt/usrprofs',
                body={}, method='GET', enable_cookie_csrf=True)
        self.assertEqual(int(pxy_response.status_code) , 502) # service available

    @patch('common.views.proxy.mixins.DjangoProxyRequestMixin._get_send_fn')
    def test_endpoint_timeout(self, mock_get_send_fn):
        def mock_send_fn_timeout(**kwargs):
            from requests.exceptions import Timeout
            raise Timeout()
        mock_get_send_fn.return_value = mock_send_fn_timeout
        pxy_response = self._send_req(headers=_header_csrf, path='/usermgt/usrprofs',
                body={}, method='GET', enable_cookie_csrf=True)
        self.assertEqual(int(pxy_response.status_code) , 504) # service timeout

    def test_endpoint_available(self):
        from api.views.constants import SERVICE_HOSTS
        data = [
            ('GET', '/usermgt/quota',    SERVICE_HOSTS['usermgt'][0]),
            ('GET', '/usermgt/roles',    SERVICE_HOSTS['usermgt'][0]),
            ('GET', '/usermgt/usrgrps',  SERVICE_HOSTS['usermgt'][0]),
            ('GET', '/usermgt/usrprofs', SERVICE_HOSTS['usermgt'][0]),
            ('GET', '/usermgt/role_applied/12', SERVICE_HOSTS['usermgt'][0]),
            ('GET', '/usermgt/grps_applied/34', SERVICE_HOSTS['usermgt'][0]),
            ('POST', '/usermgt/account/activate' ,  SERVICE_HOSTS['usermgt'][0]),
            ('POST', '/usermgt/account/deactivate', SERVICE_HOSTS['usermgt'][0]),
            ('PATCH', '/usermgt/username/edit', SERVICE_HOSTS['usermgt'][0]),
            ('PATCH', '/usermgt/password/edit', SERVICE_HOSTS['usermgt'][0]),
            ('POST', '/usermgt/remote_auth',   SERVICE_HOSTS['usermgt'][0]),
            ('GET', '/product/tags',          SERVICE_HOSTS['productmgt'][0]),
            ('GET', '/product/attrtypes',     SERVICE_HOSTS['productmgt'][0]),
            ('GET', '/product/ingredients',   SERVICE_HOSTS['productmgt'][0]),
            ('GET', '/product/saleableitems', SERVICE_HOSTS['productmgt'][0]),
        ]
        with patch('common.views.proxy.mixins.requests.request') as mock_request:
            expect_call_cnt = 0
            for http_mthd, uri, expect_pxy_host in data:
                expect_call_cnt += 1
                self._test_endpoint_available(mock_request, http_mthd, uri,
                        expect_pxy_host, expect_call_cnt)

    def _test_endpoint_available(self, mock_request, http_mthd, uri, expect_pxy_host, expect_call_cnt):
        expect_content_type = 'application/json'
        expect_status_code = 200
        expect_response_body = '{"http_medhod": "%s", "uri": "%s"}' % (http_mthd, uri)
        expect_response_body = expect_response_body.encode()
        mock_request.return_value.status_code = expect_status_code
        mock_request.return_value.ok = True
        mock_request.return_value.content = expect_response_body
        mock_request.return_value.headers = {'content-type': expect_content_type,}
        pxy_response = self._send_req(headers=_header_csrf, path=uri,
                body={}, method=http_mthd, enable_cookie_csrf=True)
        self.assertEqual(pxy_response.status_code, expect_status_code)
        self.assertEqual(mock_request.call_count, expect_call_cnt)
        call_args  = mock_request.call_args
        auth_fwd = call_args.kwargs['headers']['forwarded']
        auth_fwd = auth_fwd.split(';')
        auth_fwd = dict(map(lambda x: x.split('='), auth_fwd))
        self.assertEqual(self._uname, auth_fwd['for'])
        self.assertTrue(call_args.kwargs['url'].startswith(expect_pxy_host))
        self.assertEqual(expect_content_type, call_args.kwargs['headers']['accept'])
        self.assertEqual(expect_response_body, pxy_response.content)
    # end of test_endpoint_available()
## end of class PrivRevProxyTestCase



class UnprivRevProxyTestCase(BaseAuthenticationTestFixture, TransactionTestCase):
    def test_nonexist_resource(self):
        pxy_response = self._send_req(headers=_header_csrf, path='/usermgt/username/recovery',
                body={}, method='GET', enable_cookie_csrf=True)
        self.assertEqual(int(pxy_response.status_code) , 405)

    def test_service_unavailable(self):
        pxy_response = self._send_req(headers=_header_csrf, path='/usermgt/username/recovery',
                body={}, method='POST', enable_cookie_csrf=True)
        self.assertEqual(int(pxy_response.status_code) , 502)

    def test_unauth_endpoint_access(self):
        pxy_response = self._send_req(headers=_header_csrf, path='/usermgt/usrprofs',
                body={}, method='POST', enable_cookie_csrf=True)
        self.assertEqual(int(pxy_response.status_code) , 401)

    def test_jwks_endpoint(self):
        pxy_response = self._send_req(headers=_header_csrf, path='/jwks',
                body={}, method='GET', enable_cookie_csrf=True)
        self.assertEqual(int(pxy_response.status_code) , 200)
        stream_gen = pxy_response.streaming_content
        tmp_file = SpooledTemporaryFile(max_size=200)
        for chunk in stream_gen:
            tmp_file.write(chunk)
        file_size = tmp_file.tell()
        tmp_file.seek(0)
        jwks = json.load(tmp_file)
        tmp_file.close()
        self.assertGreater(file_size , 1)
        self.assertNotEqual(jwks.get('keys'), None)
        self.assertGreater(len(jwks['keys']), 0)
        expect_keys = ('exp', 'alg', 'kty', 'use', 'kid')
        for jwk in jwks['keys']:
            jwk_item_keys = jwk.keys()
            for expect_key in expect_keys:
                self.assertIn(expect_key, jwk_item_keys)
## end of class UnprivRevProxyTestCase

