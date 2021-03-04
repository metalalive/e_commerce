from http.cookies import Morsel
import jwt
import pdb

from django.conf import settings
from django.test import TestCase as DjangoTestCase, Client as DjangoTestClient
from django.db   import DEFAULT_DB_ALIAS
from django.core.cache       import caches as DjangoBuiltinCaches
from django.contrib.auth.models import User as AuthAccount

from common.util.python import get_header_name

# in this project, all test cases that require database accesses will make
# Django create separate / empty database only for testing purpose.

_csrf_value = 'aNvypJVzkTUUBqs9G8I41RWvSAagIQktQ3fn91WyyAiHvIcEoWE0HFD96gz028ol'
_cookie_csrf = {
    'anticsrftok': _csrf_value,
    'csrf_header_name': get_header_name(name=settings.CSRF_HEADER_NAME),
    'csrf_cookie_name': settings.CSRF_COOKIE_NAME,
}

_header_csrf = {settings.CSRF_HEADER_NAME: _csrf_value}

cache_jwt_secret = DjangoBuiltinCaches['jwt_secret']


class BaseAuthenticationTestFixture:
    databases = {DEFAULT_DB_ALIAS, 'usermgt_service'}
    _uname = 'YOUR_USERNAME'
    _passwd = 'YOUR_PASSWORD'
    _json_mimetype = 'application/json'
    _body = {'username': _uname, 'password': _passwd,}
    _user_attributes = {'username':_uname, 'password': _passwd,
            'is_active':False, 'is_superuser':False, 'is_staff':False, }
    _client = DjangoTestClient(enforce_csrf_checks=True, HTTP_ACCEPT=_json_mimetype)

    def __init__(self, *args, **kwargs):
        """
        Note that Django creates several instances of the same testcase class
        depending on number of test case functions declared in the class.
        """
        self._usr = None
        self._session = None
        super().__init__(*args, **kwargs)

    def __del__(self):
        """
        note that destructor is invoked every time after tearDown() completes,
        which implicitly means the class will run this destructor multiple times
        as number of test case functions (the functions beginning with `test_xxx`)
        """

    def setUp(self):
        pass

    def tearDown(self):
        if self._usr:
            result = cache_jwt_secret.delete(self._usr.pk)
            ##print('tear down, jwt secret deleted ? %s' % result)
        if self._client.session:
            ##print('staff session: %s' % self._client.session)
            self._client.session.delete()
        self._client.cookies.clear()
        AuthAccount.objects.filter(username=self._uname).delete()
        self._usr = None

    def _send_req(self, headers:dict, usr_attr:dict, path:str, body=None, method='post',
            enable_cookie_csrf=False):
        if enable_cookie_csrf:
            self._update_cookie_csrf()
        if usr_attr:
            self._usr = AuthAccount.objects.create_user(**usr_attr)
        #pdb.set_trace()
        if body is None:
            body = self._body
        fn = getattr(self._client, method.lower())
        response = fn(path=path, data=body, content_type=self._json_mimetype, **headers)
        return response

    def _update_cookie_csrf(self):
        for k,v in _cookie_csrf.items():
            self._client.cookies[k] = v
        #self._client.cookies.update(_cookie_csrf)


class LoginTestCase(BaseAuthenticationTestFixture, DjangoTestCase):
    _path = '/login'

    def test_no_csrf_failure(self):
        response = self._send_req(headers={}, usr_attr={}, path=self._path)
        self.assertEqual(int(response.status_code) , 403)
        self.assertIn(b'CSRF cookie', response.content)

    def test_inactive_failure(self):
        response = self._send_req(headers=_header_csrf, path=self._path,
                usr_attr=self._user_attributes, enable_cookie_csrf=True)
        self.assertIn(b'auth', response.content)
        self.assertIn(b'fail', response.content)
        self._verify_failure(response)

    def test_nonstaff_failure(self):
        usr_attr_cpy = self._user_attributes.copy()
        usr_attr_cpy['is_active'] = True
        response = self._send_req(headers=_header_csrf, path=self._path,
                usr_attr=usr_attr_cpy, enable_cookie_csrf=True)
        self._verify_failure(response)

    def test_staff_succeed(self):
        usr_attr_cpy = self._user_attributes.copy()
        usr_attr_cpy['is_active'] = True
        usr_attr_cpy['is_staff'] = True
        response = self._send_req(headers=_header_csrf, path=self._path,
                usr_attr=usr_attr_cpy, enable_cookie_csrf=True)
        self._verify_succeed(response)

    def test_superuser_succeed(self):
        usr_attr_cpy = self._user_attributes.copy()
        usr_attr_cpy['is_active'] = True
        usr_attr_cpy['is_staff'] = True
        usr_attr_cpy['is_superuser'] = True
        response = self._send_req(headers=_header_csrf, path=self._path,
                usr_attr=usr_attr_cpy, enable_cookie_csrf=True)
        self._verify_succeed(response)

    def _verify_failure(self, response):
        self.assertEqual(int(response.status_code) , 401)
        jwt_cookie = self._client.cookies.get('jwt', None)
        sessid_cookie = self._client.cookies.get('sessionid', None)
        self.assertEqual(jwt_cookie , None)
        self.assertEqual(sessid_cookie , None)

    def _verify_succeed(self, response):
        self.assertEqual(int(response.status_code) , 200)
        self.assertEqual(response.exc_info , None)
        jwt_cookie = self._client.cookies.get('jwt', None)
        sessid_cookie = self._client.cookies.get('sessionid', None)
        self.assertNotEqual(jwt_cookie , None)
        self.assertNotEqual(sessid_cookie , None)
        self.assertEqual(type(jwt_cookie) , Morsel)
        # verify jwt
        self._verify_recv_jwt(encoded=jwt_cookie.value)
        # the session comes from Django built-in function, no need to test

    def _verify_recv_jwt(self, encoded):
        #print('staff cookie, jwt : %s' % encoded)
        secret = cache_jwt_secret.get(self._usr.pk , None)
        header = jwt.get_unverified_header( encoded )
        payld_unverified = jwt.decode(encoded, options={'verify_signature':False})
        payld_verified   = jwt.decode(encoded, secret,  algorithms=header['alg'])
        self.assertNotEqual(payld_verified , None)
        self.assertEqual(payld_verified , payld_unverified)



class LogoutTestCase(BaseAuthenticationTestFixture, DjangoTestCase):
    _path = '/logout'

    def test_no_login_failure(self):
        headers = _header_csrf
        response = self._client.post(path=self._path, data='',
                content_type=self._json_mimetype, **headers)
        self.assertEqual(int(response.status_code) , 401)

    def test_succeed(self):
        usr_attr_cpy = self._user_attributes.copy()
        usr_attr_cpy['is_active'] = True
        usr_attr_cpy['is_staff'] = True
        login_response = self._send_req(headers=_header_csrf, path='/login',
                usr_attr=usr_attr_cpy, enable_cookie_csrf=True)
        jwt_cookie = self._client.cookies.get('jwt', None)
        sessid_cookie = self._client.cookies.get('sessionid', None)
        self.assertNotEqual(jwt_cookie.value , '')
        self.assertNotEqual(sessid_cookie.value , '')

        self._update_cookie_csrf()
        headers = _header_csrf
        response = self._client.post(path=self._path, data='',
                content_type=self._json_mimetype, **headers)
        self.assertEqual(int(response.status_code) , 200)
        jwt_cookie = self._client.cookies.get('jwt', None)
        sessid_cookie = self._client.cookies.get('sessionid', None)
        self.assertEqual(jwt_cookie.value , '')
        self.assertEqual(sessid_cookie.value , '')


class RevProxyTestCase(BaseAuthenticationTestFixture, DjangoTestCase):

    def _login(self):
        usr_attr_cpy = self._user_attributes.copy()
        usr_attr_cpy['is_active'] = True
        usr_attr_cpy['is_staff'] = True
        login_response = self._send_req(headers=_header_csrf, path='/login',
                usr_attr=usr_attr_cpy, enable_cookie_csrf=True)
        self.assertEqual(int(login_response.status_code) , 200)

    def _logout(self):
        logout_response = self._send_req(headers=_header_csrf, path='/logout',
                usr_attr=None, body={}, enable_cookie_csrf=True)
        self.assertEqual(int(logout_response.status_code) , 200)

    def test_nonexist_priv_res(self):
        self._login()
        pxy_response = self._send_req(headers=_header_csrf, path='/non_exist_path',
                usr_attr=None, body={}, method='GET', enable_cookie_csrf=True)
        self._logout()
        self.assertEqual(int(pxy_response.status_code) , 404)
        self.assertEqual(pxy_response.exc_info , None)

    # downstream services will NOT turn on for integration test, in this case it is normal
    # to receive bad gateway status (502) within the backend's response.

    def test_exist_priv_res(self):
        self._login()
        pxy_response = self._send_req(headers=_header_csrf, path='/usermgt/usrprofs',
                usr_attr=None, body={}, method='GET', enable_cookie_csrf=True)
        self._logout()
        self.assertEqual(int(pxy_response.status_code) , 502)
        self.assertEqual(pxy_response.exc_info , None)

    def test_exist_unpriv_res(self):
        pxy_response = self._send_req(headers=_header_csrf, path='/usermgt/username/recovery',
                usr_attr=None, body={}, method='GET', enable_cookie_csrf=True)
        self.assertEqual(int(pxy_response.status_code) , 405)
        pxy_response = self._send_req(headers=_header_csrf, path='/usermgt/username/recovery',
                usr_attr=None, body={}, method='POST', enable_cookie_csrf=True)
        self.assertEqual(int(pxy_response.status_code) , 502)


