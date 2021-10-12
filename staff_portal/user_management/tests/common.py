import string
import random

from django.conf import settings as django_settings
from django.middleware.csrf import _get_new_csrf_token
from django.utils import timezone as django_timezone

from common.cors.middleware import conf as cors_conf

from user_management.models.common import AppCodeOptions
from user_management.models.auth import LoginAccount, Role
from user_management.models.base import GenericUserProfile, GenericUserGroup, QuotaMaterial, EmailAddress, PhoneNumber, GeoLocation

_fixtures = {
    LoginAccount: [
        {'is_superuser':False, 'is_staff':True,  'is_active':True,  'username': 'AltinGun','password': '93rutGrPt'} ,
        {'is_superuser':False, 'is_staff':False, 'is_active':True,  'username': 'KingGizz','password': '39rjfR@et'} ,
        {'is_superuser':False, 'is_staff':True,  'is_active':False, 'username': 'Imarhan', 'password': 'if74w#gfy'} ,
        {'is_superuser':True,  'is_staff':False, 'is_active':True,  'username': 'yuk0p1ano', 'password': 'anti@s0cia1'} ,
        {'is_superuser':True,  'is_staff':False, 'is_active':False, 'username': 'remoteCtrl','password': '9rJ3yf740fM'} ,
    ],
    GenericUserProfile: [
        {'id':3, 'first_name':'Jon', 'last_name':'Snow'},
        {'id':4, 'first_name':'Shelton', 'last_name':'Cooper'},
        {'id':5, 'first_name':'Kenny',  'last_name':'McCormick'},
        {'id':6, 'first_name':'Shaun',  'last_name':'Merphy'},
    ],
    Role: [
        {'id':idx, 'name':'my role %s' % ''.join(random.choices(string.ascii_letters, k=8)) } for idx in range(4, 14)
    ],
    QuotaMaterial: [
        {"id": 1, "app_code": AppCodeOptions.user_management, "mat_code": QuotaMaterial._MatCodeOptions.MAX_NUM_EMAILS.value} ,
        {"id": 2, "app_code": AppCodeOptions.user_management, "mat_code": QuotaMaterial._MatCodeOptions.MAX_NUM_GEO_LOCATIONS.value} ,
        {"id": 3, "app_code": AppCodeOptions.user_management, "mat_code": QuotaMaterial._MatCodeOptions.MAX_NUM_PHONE_NUMBERS.value} ,
        {"id": 4, "app_code": AppCodeOptions.product,    "mat_code": 1} ,
        {"id": 5, "app_code": AppCodeOptions.product,    "mat_code": 2} ,
        {"id": 6, "app_code": AppCodeOptions.fileupload, "mat_code": 3} ,
        {"id": 7, "app_code": AppCodeOptions.fileupload, "mat_code": 5} ,
    ],
    GenericUserGroup:[
        {'id':3 , 'name':'rest of my career'},
        {'id':4 , 'name':'avoid code smell'},
        {'id':5 , 'name':'improve refacting ability'},
        {'id':6 , 'name':'never be more than one reason'},
        {'id':7 , 'name':'whats being done in the func'},
        {'id':8 , 'name':'keep coupling and cohesion'},
        {'id':9 , 'name':'range of coincidence'},
        {'id':10, 'name':'coding block'},
        {'id':11, 'name':'big class controling everything'},
        {'id':12, 'name':'loosing related'},
        {'id':13, 'name':'thats whole discussions'},
        {'id':14, 'name':'problem of human design'},
    ],
    EmailAddress: [
        {'id':idx, 'addr':'%s@%s.%s' % (
            ''.join(random.choices(string.ascii_letters, k=8)), \
            ''.join(random.choices(string.ascii_letters, k=10)), \
            ''.join(random.choices(string.ascii_letters, k=3)) \
            )
        } for idx in range(3, 12)
    ],
    PhoneNumber: [
        {'id':idx, 'country_code':str(random.randrange(1,999)),
            'line_number': str(random.randrange(0x10000000, 0xffffffff)) }  for idx in range(3, 10)
    ],
    GeoLocation: [
        {'id':3, 'country':'DE', 'province':'Hamburg', 'locality':'Heiderburg', 'street':'Generic Size', 'detail':'PitaProfession House', 'floor': 3, 'description':'AutoFarm101'},
        {'id':4, 'country':'CZ', 'province':'Buno', 'locality':'Gurrigashee', 'street':'Old castle ave', 'detail':'Agile Lane, 4-8-1', 'floor':-1, 'description':'Smart connected Handle Bar studio'},
        {'id':5, 'country':'SG', 'province':'cannotBeEmpty', 'locality':'Chang-i', 'street':'Tok-Hua rd.', 'detail':'Tyson mansion', 'floor':8, 'description':'contexturize marshall language'},
        {'id':6, 'country':'PT', 'province':'Leewisky', 'locality':'Lisbon', 'street':'green straw st', 'detail':'Booming Lane 13-6', 'floor':7, 'description':'Herb nursery'},
        {'id':7, 'country':'ID', 'province':'Gunung Surawesi', 'locality':'Gyrueoq0', 'street':'Steamer road 199', 'detail':'Broken Bay', 'floor':1, 'description':'human resource agency'},
        {'id':8, 'country':'IN', 'province':'Udaipur', 'locality':'Jenkistania', 'street':'Taiwan Independent ave. 426', 'detail':'be independent', 'floor':1, 'description':'Human Right NGO'},
    ],
} ## end of _fixtures


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


