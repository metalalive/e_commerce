import string
import random
from datetime import timedelta

from django.conf import settings as django_settings
from django.middleware.csrf import _get_new_csrf_token
from django.core.exceptions import  ObjectDoesNotExist
from django.utils import timezone as django_timezone

from common.cors.middleware import conf as cors_conf

from user_management.models.common import AppCodeOptions
from user_management.models.auth import LoginAccount, Role
from user_management.models.base import GenericUserProfile, GenericUserGroup, QuotaMaterial, EmailAddress, PhoneNumber, GeoLocation

from tests.python.common import listitem_rand_assigner, KeystoreMixin

_curr_timezone = django_timezone.get_current_timezone()

num_login_profiles = 29


def gen_expiry_time(minutes_valid=None, serializable=True):
    minutes_valid = minutes_valid or random.randrange(0,60)
    if minutes_valid > 5:
        expiry_time = django_timezone.now() + timedelta(minutes=minutes_valid)
        # timezone has to be consistent
        expiry_time = expiry_time.astimezone(_curr_timezone)
        if serializable:
            expiry_time = expiry_time.isoformat()
    else:
        expiry_time = None
    return expiry_time


_fixtures = {
    LoginAccount: [
        {'is_superuser':False, 'is_staff':False,  'is_active':False, 'profile':None, \
                'password_last_updated': gen_expiry_time(minutes_valid=8), \
                'username': ''.join(random.choices(string.ascii_letters, k=10)), \
                'password': ''.join(random.choices(string.ascii_letters, k=16) + ['@']) \
        } for _ in range(num_login_profiles)
    ],
    GenericUserProfile: [
        {'id':idx, 'first_name':''.join(random.choices(string.ascii_letters, k=5)),
            'last_name':''.join(random.choices(string.ascii_letters, k=8)) }  for idx in range(1, 1 + num_login_profiles)
    ],
    Role: [ # including superuser role and staff role
        {'id':idx, 'name':'my role %s' % ''.join(random.choices(string.ascii_letters, k=8)) } \
                for idx in range(GenericUserProfile.STAFF, 15)
    ],
    QuotaMaterial: [
        {"id": 1, "app_code": AppCodeOptions.user_management, "mat_code": QuotaMaterial._MatCodeOptions.MAX_NUM_EMAILS.value} ,
        {"id": 2, "app_code": AppCodeOptions.user_management, "mat_code": QuotaMaterial._MatCodeOptions.MAX_NUM_GEO_LOCATIONS.value} ,
        {"id": 3, "app_code": AppCodeOptions.user_management, "mat_code": QuotaMaterial._MatCodeOptions.MAX_NUM_PHONE_NUMBERS.value} ,
        {"id": 4, "app_code": AppCodeOptions.product,    "mat_code": 1} ,
        {"id": 5, "app_code": AppCodeOptions.product,    "mat_code": 2} ,
        {"id": 6, "app_code": AppCodeOptions.media, "mat_code": 3} ,
        {"id": 7, "app_code": AppCodeOptions.media, "mat_code": 5} ,
        {"id": 8, "app_code": AppCodeOptions.store,  "mat_code": 1} ,
        {"id": 9, "app_code": AppCodeOptions.store,  "mat_code": 2} ,
        {"id":10, "app_code": AppCodeOptions.store,  "mat_code": 3} ,
        {"id":11, "app_code": AppCodeOptions.store,  "mat_code": 4} ,
        {"id":12, "app_code": AppCodeOptions.order,  "mat_code": 2} ,
        {"id":13, "app_code": AppCodeOptions.order,  "mat_code": 3} ,
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
        } for idx in range(3, 25)
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


def _setup_login_account(account_data, profile_obj, roles=None, expiry=None):
    account_data = account_data.copy()
    login_user_profile = profile_obj
    account_data['profile'] = login_user_profile
    account_data['password_last_updated'] = django_timezone.now()
    LoginAccount.objects.create_user(**account_data)
    # assume that the logged-in user has access to assign all the roles to groups
    roles = roles or []
    for role in roles:
        data_kwargs = {'expiry': gen_expiry_time(minutes_valid=expiry),
                'role':role, 'approved_by':login_user_profile,}
        login_user_profile.roles.create(**data_kwargs)
    login_user_profile.refresh_from_db()
    return login_user_profile


def client_req_csrf_setup():
    usermgt_host_url = cors_conf.ALLOWED_ORIGIN['user_management']
    scheme_end_pos = usermgt_host_url.find('://') + 3
    valid_csrf_token = _get_new_csrf_token()
    # (1) assume every request from this application is cross-origin reference.
    # (2) Django's test client sets `testserver` to host name of each reqeust
    #     , which cause error in CORS middleware, I fixed the problem by adding
    #    SERVER_NAME header directly passing in Django's test client (it is only
    #    for testing purpose)
    base_headers = {
        'SERVER_NAME': usermgt_host_url[scheme_end_pos:],
        'HTTP_ORIGIN': cors_conf.ALLOWED_ORIGIN['web'],
        django_settings.CSRF_HEADER_NAME: valid_csrf_token,
    }
    base_cookies = {
        django_settings.CSRF_COOKIE_NAME: valid_csrf_token,
    } # mock CSRF token previously received from web app
    return { 'headers': base_headers, 'cookies': base_cookies, 'enforce_csrf_checks':True }


class AuthenticateUserMixin(KeystoreMixin):
    _keystore_init_config = django_settings.AUTH_KEYSTORE

    def _auth_setup(self, testcase, profile=None, login_password=None, new_account_data=None,
            is_staff=True, is_active=True, is_superuser=False):
        api_login_kwargs = client_req_csrf_setup()
        api_login_kwargs['path'] = '/login'
        api_login_kwargs['method'] = 'post'
        if profile is None:
            profile_data = {'id': 3, 'first_name':'Brooklynn', 'last_name':'Jenkins'}
            profile = GenericUserProfile.objects.create(**profile_data)
        try:
            account = profile.account
            assert login_password, 'login_password has to be given'
        except ObjectDoesNotExist as e:
            if not new_account_data:
                new_account_data = {'username':'ImStaff', 'password':'dontexpose',
                    'is_superuser':is_superuser}
            new_account_data.update({'profile': profile, 'is_active':is_active, 'is_staff':is_staff,})
            ## account = LoginAccount.objects.create_user(**new_account_data)
            profile =  _setup_login_account(account_data=new_account_data, profile_obj=profile,
                   roles=None, expiry=None)
            account = profile.account
            login_password = new_account_data['password']
        api_login_kwargs['body'] = {'username': account.username, 'password':login_password }
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
## end of class AuthenticateUserMixin


class UserNestedFieldSetupMixin:
    num_roles = 0
    num_quota = 0

    def _gen_roles(self, role_objs, num=None, serializable=True):
        if num is None:
            num = self.num_roles
        out = []
        if num > 0:
            roles_gen = listitem_rand_assigner(list_=role_objs, min_num_chosen=num,
                    max_num_chosen=(num + 1))
            for role in roles_gen:
                if serializable:
                    role = role.id
                data = {'expiry': gen_expiry_time(serializable=serializable), 'role':role,
                        'approved_by': random.randrange(3,1000), # will NOT write this field to model
                        }
                out.append(data)
        return out

    def _gen_quota(self, quota_mat_objs, num=None, serializable=True):
        if num is None:
            num = self.num_quota
        self.num_locations = 0
        self.num_emails = 0
        self.num_phones = 0
        out = []
        if num > 0:
            materials_gen = listitem_rand_assigner(list_=quota_mat_objs, min_num_chosen=num,
                    max_num_chosen=(num + 1))
            for material in materials_gen:
                maxnum = random.randrange(1,10)
                if material.app_code == AppCodeOptions.user_management:
                    if material.mat_code == QuotaMaterial._MatCodeOptions.MAX_NUM_PHONE_NUMBERS.value:
                        self.num_phones = maxnum
                    elif material.mat_code == QuotaMaterial._MatCodeOptions.MAX_NUM_EMAILS.value:
                        self.num_emails = maxnum
                    elif material.mat_code == QuotaMaterial._MatCodeOptions.MAX_NUM_GEO_LOCATIONS.value:
                        self.num_locations = maxnum
                if serializable:
                    material = material.id
                data = {'expiry':gen_expiry_time(serializable=serializable), 'material':material, 'maxnum':maxnum}
                out.append(data)
        return out

    def _gen_locations(self, num=None):
        if num is None:
            num = min(self.num_locations, len(_fixtures[GeoLocation]))
        out = []
        if num > 0:
            data_gen = listitem_rand_assigner(list_=_fixtures[GeoLocation], min_num_chosen=num,
                    max_num_chosen=(num + 1))
            out = list(data_gen)
        return out

    def _gen_emails(self, num=None):
        if num is None:
            num = min(self.num_emails, len(_fixtures[EmailAddress]))
        out = []
        if num > 0:
            data_gen = listitem_rand_assigner(list_=_fixtures[EmailAddress], min_num_chosen=num,
                    max_num_chosen=(num + 1))
            out = list(data_gen)
        return out

    def _gen_phones(self, num=None):
        if num is None:
            num = min(self.num_phones, len(_fixtures[PhoneNumber]))
        out = []
        if num > 0:
            data_gen = listitem_rand_assigner(list_=_fixtures[PhoneNumber], min_num_chosen=num,
                    max_num_chosen=(num + 1))
            out = list(data_gen)
        return out



class UserNestedFieldVerificationMixin:
    _nested_field_names = None

    def load_group_from_instance(self, obj):
        value = {'id': obj.id,}
        _fields_compare = self._nested_field_names['roles']
        value['roles'] = list(obj.roles.values(*_fields_compare))
        for d in value['roles']:
            if not d['expiry']:
                continue
            d['expiry'] = d['expiry'].astimezone(_curr_timezone)
            d['expiry'] = d['expiry'].isoformat()
        _fields_compare = self._nested_field_names['quota']
        value['quota'] = list(obj.quota.values(*_fields_compare))
        for d in value['quota']:
            if not d['expiry']:
                continue
            d['expiry'] = d['expiry'].astimezone(_curr_timezone)
            d['expiry'] = d['expiry'].isoformat()
        _fields_compare = self._nested_field_names['emails']
        value['emails'] = list(obj.emails.values(*_fields_compare))
        _fields_compare = self._nested_field_names['phones']
        value['phones'] = list(obj.phones.values(*_fields_compare))
        _fields_compare = self._nested_field_names['locations']
        value['locations'] = list(obj.locations.values(*_fields_compare))
        return value

    def _value_compare_fn(self, val_a, val_b):
        fields_eq = {}
        fields_eq['roles']  = self._value_compare_roles_fn(val_a=val_a, val_b=val_b)
        fields_eq['quota']  = self._value_compare_quota_fn(val_a=val_a, val_b=val_b)
        for k in ('emails', 'phones', 'locations'):
            fields_eq[k] = self._value_compare_contact_fn(val_a=val_a[k], val_b=val_b[k],
                    _fields_compare=self._nested_field_names[k])
        return fields_eq

    def _value_compare_roles_fn(self, val_a, val_b):
        _fields_compare = self._nested_field_names['roles']
        expect_val = list(map(lambda d: {fname:d[fname] for fname in _fields_compare}, val_a['roles']))
        actual_val = list(map(lambda d: {fname:d[fname] for fname in _fields_compare}, val_b['roles']))
        expect_val = sorted(expect_val, key=lambda d:d['role'])
        actual_val = sorted(actual_val, key=lambda d:d['role'])
        return actual_val == expect_val

    def _value_compare_quota_fn(self, val_a, val_b):
        _fields_compare = self._nested_field_names['quota']
        expect_val = list(map(lambda d: {fname:d[fname] for fname in _fields_compare}, val_a['quota']))
        actual_val = list(map(lambda d: {fname:d[fname] for fname in _fields_compare}, val_b['quota']))
        expect_val = sorted(expect_val, key=lambda d:d['material'])
        actual_val = sorted(actual_val, key=lambda d:d['material'])
        return actual_val == expect_val

    def _value_compare_contact_fn(self, val_a, val_b, _fields_compare, compare_id=False):
        if not compare_id:
            _fields_compare = _fields_compare.copy()
            _fields_compare.remove('id')
        expect_val = list(map(lambda d: tuple([d[fname] for fname in _fields_compare]), val_a))
        actual_val = list(map(lambda d: tuple([d[fname] for fname in _fields_compare]), val_b))
        expect_val = sorted(expect_val)
        actual_val = sorted(actual_val)
        return actual_val == expect_val


