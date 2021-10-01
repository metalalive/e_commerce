
from django.conf import settings as django_settings
from django.middleware.csrf import _get_new_csrf_token
from django.utils import timezone as django_timezone

from common.cors.middleware import conf as cors_conf

from user_management.models.auth import LoginAccount, Role
from user_management.models.base import GenericUserProfile

_fixtures = {
    'LoginAccount': [
        {'id':14, 'is_superuser':False, 'is_staff':True,  'is_active':True,  'username': 'AltinGun','password': '93rutGrPt'} ,
        {'id':19, 'is_superuser':False, 'is_staff':False, 'is_active':True,  'username': 'KingGizz','password': '39rjfR@et'} ,
        {'id':10, 'is_superuser':False, 'is_staff':True,  'is_active':False, 'username': 'Imarhan', 'password': 'if74w#gfy'} ,
        {'id':7,  'is_superuser':True,  'is_staff':False, 'is_active':True,  'username': 'yuk0p1ano', 'password': 'anti@s0cia1'} ,
        {'id':8,  'is_superuser':True,  'is_staff':False, 'is_active':False, 'username': 'remoteCtrl','password': '9rJ3yf740fM'} ,
    ],
    'GenericUserProfile': [
        {'id':2, 'first_name':'Jon', 'last_name':'Snow'},
        {'id':3, 'first_name':'Shelton', 'last_name':'Cooper'},
        {'id':4, 'first_name':'Kenny',  'last_name':'McCormick'},
    ],
}


def client_req_csrf_setup():
    usermgt_host_url = cors_conf.ALLOWED_ORIGIN['user_management']
    scheme_end_pos = usermgt_host_url.find('://') + 3
    valid_csrf_token = _get_new_csrf_token()
    base_headers = {
        'SERVER_NAME': usermgt_host_url[scheme_end_pos:],
        'HTTP_ORIGIN': cors_conf.ALLOWED_ORIGIN['web'],
        django_settings.CSRF_HEADER_NAME: valid_csrf_token,
    }
    base_cookies = {
        django_settings.CSRF_COOKIE_NAME: valid_csrf_token,
    } # mock CSRF token previously received from web app
    return { 'headers': base_headers, 'cookies': base_cookies, 'enforce_csrf_checks':True }


class AuthenticateUserMixin:
    def _auth_setup(self, testcase, is_staff=True, is_active=True, is_superuser=False):
        api_login_kwargs = client_req_csrf_setup()
        api_login_kwargs['path'] = '/login'
        api_login_kwargs['method'] = 'post'
        profile_data = {'id': 3, 'first_name':'Brooklynn', 'last_name':'Jenkins'}
        profile = GenericUserProfile.objects.create(**profile_data)
        account_data = {'username':'ImStaff', 'password':'dontexpose', 'is_active':is_active,
                'is_staff':is_staff, 'is_superuser':is_superuser, 'profile':profile,
                'password_last_updated':django_timezone.now(), }
        account = LoginAccount.objects.create_user(**account_data)
        api_login_kwargs['body'] = {key: account_data[key] for key in ('username','password')}
        # first login request
        response = testcase._send_request_to_backend(**api_login_kwargs)
        testcase.assertEqual(int(response.status_code), 200)
        return profile, response

    def _refresh_access_token(self, testcase, audience):
        testcase.assertIn(django_settings.JWT_NAME_REFRESH_TOKEN, testcase._client.cookies.keys())
        api_call_kwargs = {'path':'/refresh_access_token', 'method':'get',
                'extra_query_params':{'audience': ','.join(audience)}}
        api_call_kwargs.update(client_req_csrf_setup())
        response = testcase._send_request_to_backend(**api_call_kwargs)
        testcase.assertEqual(int(response.status_code), 200)
        return response.json()


