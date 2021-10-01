import random
import json

from django.test import TransactionTestCase
from django.utils import timezone as django_timezone
from django.contrib.auth.models import Permission
from django.contrib.contenttypes.models  import ContentType
from rest_framework.settings    import api_settings as drf_settings

from common.util.python import sort_nested_object
from common.models.constants     import ROLE_ID_SUPERUSER, ROLE_ID_STAFF
from user_management.serializers import PermissionSerializer
from user_management.models.base import GenericUserProfile, GenericUserAppliedRole
from user_management.models.auth import LoginAccount, Role

from tests.python.common import HttpRequestDataGen
from tests.python.common.django import _BaseMockTestClientInfoMixin
from user_management.tests.common import _fixtures, client_req_csrf_setup, AuthenticateUserMixin

non_fd_err_key = drf_settings.NON_FIELD_ERRORS_KEY

_srlz_fn = lambda role: {'id': role.id, 'name':role.name, 'permissions': list(role.permissions.values_list('id', flat=True))}


def _setup_user_roles(profile, approved_by, extra_role_data=None):
    role_data = [
        {'id':ROLE_ID_STAFF, 'name':'base staff',  },
        {'id':6, 'name':'role manager',},
    ]
    extra_role_data = extra_role_data or []
    role_data.extend(extra_role_data)
    roles = tuple(map(lambda d:Role.objects.create(**d) , role_data))
    for role in roles:
        data = {'last_updated':django_timezone.now(), 'approved_by': approved_by, 'role': role}
        applied_role = GenericUserAppliedRole(**data)
        profile.roles.add(applied_role, bulk=False)
    return roles


class PermissionTestCase(TransactionTestCase, _BaseMockTestClientInfoMixin, AuthenticateUserMixin):
    path = '/permissions'

    def setUp(self):
        self._profile, _ = self._auth_setup(testcase=self, is_superuser=False)
        profile_2nd_data = {'id': 5, 'first_name':'Bigbrother', 'last_name':'Iswatching'}
        self._profile_2nd = GenericUserProfile.objects.create(**profile_2nd_data)
        self._roles = _setup_user_roles(profile=self._profile, approved_by=self._profile_2nd)
        self.api_call_kwargs = client_req_csrf_setup()
        self.api_call_kwargs.update({'path': self.path, 'method':'get'})

    def tearDown(self):
        self._client.cookies.clear()

    def test_no_permission(self):
        qset = Permission.objects.filter(content_type__app_label='user_management', codename='view_quotamaterial')
        self._roles[1].permissions.set(qset)
        acs_tok_resp = self._refresh_access_token(testcase=self, audience=['user_management'])
        access_token = acs_tok_resp['access_token']
        response = self._send_request_to_backend(**self.api_call_kwargs)
        self.assertEqual(int(response.status_code), 401)
        self.api_call_kwargs['headers']['HTTP_AUTHORIZATION'] = ' '.join(['Bearer', access_token])
        response = self._send_request_to_backend(**self.api_call_kwargs)
        self.assertEqual(int(response.status_code), 403)
        # post, put, delete methods are not allowed
        for method in ('post','put','patch', 'delete'):
            self.api_call_kwargs['method'] = method
            response = self._send_request_to_backend(**self.api_call_kwargs)
            self.assertEqual(int(response.status_code), 405)

    def test_read_ok(self):
        qset = Permission.objects.filter(content_type__app_label='user_management', codename='view_role')
        self._roles[1].permissions.set(qset)
        acs_tok_resp = self._refresh_access_token(testcase=self, audience=['user_management'])
        access_token = acs_tok_resp['access_token']
        self.api_call_kwargs['headers']['HTTP_AUTHORIZATION'] = ' '.join(['Bearer', access_token])
        response = self._send_request_to_backend(**self.api_call_kwargs)
        self.assertEqual(int(response.status_code), 200)
        actual_perms = response.json()
        expect_perms = PermissionSerializer.get_default_queryset().values('id','name')
        self.assertGreater(expect_perms.count(), 0)
        actual_perms = sorted(actual_perms, key=lambda d:d['id'])
        expect_perms = sorted(expect_perms, key=lambda d:d['id'])
        self.assertListEqual(actual_perms, expect_perms)
## end of class PermissionTestCase


class RoleCreationTestCase(TransactionTestCase, _BaseMockTestClientInfoMixin, AuthenticateUserMixin):
    path = '/roles'

    def setUp(self):
        self._profile, _ = self._auth_setup(testcase=self, is_superuser=False)
        profile_2nd_data = {'id': 6, 'first_name':'Stan', 'last_name':'Marsh'}
        self._profile_2nd = GenericUserProfile.objects.create(**profile_2nd_data)
        self._roles = _setup_user_roles(profile=self._profile, approved_by=self._profile_2nd)
        self._permissions = Permission.objects.all()
        self.api_call_kwargs = client_req_csrf_setup()
        self.api_call_kwargs.update({'path': self.path, 'method':'post'})

    def tearDown(self):
        self._client.cookies.clear()

    def _prepare_access_token(self, new_perms_info):
        qset = Permission.objects.filter(content_type__app_label='user_management',
                codename__in=new_perms_info)
        self._roles[1].permissions.set(qset)
        acs_tok_resp = self._refresh_access_token(testcase=self, audience=['user_management'])
        access_token = acs_tok_resp['access_token']
        self.api_call_kwargs['headers']['HTTP_AUTHORIZATION'] = ' '.join(['Bearer', access_token])

    def test_no_permission(self):
        self._prepare_access_token(new_perms_info=['view_quotamaterial', 'view_role'])
        response = self._send_request_to_backend(**self.api_call_kwargs)
        self.assertEqual(int(response.status_code), 403)

    def test_input_error(self):
        self._prepare_access_token(new_perms_info=['view_role','add_role'])
        # subcase #1, empty request data
        self.api_call_kwargs['body'] = []
        response = self._send_request_to_backend(**self.api_call_kwargs)
        self.assertEqual(int(response.status_code), 400)
        err_info = response.json()
        self.assertEqual('request data should not be empty', err_info['detail'])
        # subcase #2, lacking essential fields
        body = [{'name':'security vendor',} , {}]
        self.api_call_kwargs['body'] = body
        response = self._send_request_to_backend(**self.api_call_kwargs)
        self.assertEqual(int(response.status_code), 400)
        err_info = response.json()
        self.assertIn('This field is required.', err_info[0].get('permissions', []))
        self.assertIn('This field is required.', err_info[1].get('permissions', []))
        self.assertIn('This field is required.', err_info[1].get('name', []))
        # subcase #3, name field, permissions field  are empty, another permissions field
        # includes low-level permissions which are NOT allowed to use in application
        perms = self._permissions.filter(content_type__app_label='contenttypes')
        perms = perms.values_list('id', flat=True)
        perms = list(perms)
        body = [{'name':'', 'permissions':perms}, {'name':'security vend\x00r', 'permissions':[]}]
        self.api_call_kwargs['body'] = body
        response = self._send_request_to_backend(**self.api_call_kwargs)
        self.assertEqual(int(response.status_code), 400)
        err_info = response.json()
        self.assertIn('This field may not be blank.',     err_info[0].get('name', []))
        self.assertIn('Null characters are not allowed.', err_info[1].get('name', []))
        self.assertIn('This list may not be empty.',      err_info[1].get('permissions', []))
        expect_err_msg = 'Invalid pk "%s" - object does not exist.' % perms[0]
        self.assertIn(expect_err_msg, err_info[0].get('permissions', []))
        # subcase #4, invalid permission ID
        invalid_perm_id = '-123'
        body = [{'name':'Museu\x02m', 'permissions':[invalid_perm_id]}, {'name':'security vendor', 'permissions':['1o4']}]
        self.api_call_kwargs['body'] = body
        response = self._send_request_to_backend(**self.api_call_kwargs)
        self.assertEqual(int(response.status_code), 400)
        err_info = response.json()
        expect_err_msg = 'Invalid pk "%s" - object does not exist.' % invalid_perm_id
        self.assertIn(expect_err_msg, err_info[0].get('permissions', []))
        self.assertIn('Incorrect type. Expected pk value, received str.', err_info[1].get('permissions', []))


    def test_bulk_ok(self):
        self._prepare_access_token(new_perms_info=['view_role','add_role'])
        perms = self._permissions.filter(codename__contains='generic')
        perms = perms.values_list('id', flat=True)
        perms = list(perms)
        body = [
                {'name':'security vendor', 'permissions':perms[0:2]},
                {'name':'SoC emulator',    'permissions':perms[2:4]},
                {'name':'Human Resource team', 'permissions':perms[4:6]},
            ]
        self.api_call_kwargs['body'] = body
        # subcase #1: add roles without specifying id
        response = self._send_request_to_backend(**self.api_call_kwargs)
        self.assertEqual(int(response.status_code), 201)
        result = response.json()
        expect_result = body
        actual_result = list(map(lambda d:{key:d[key] for key in ('name','permissions')}, result))
        expect_result = sort_nested_object(expect_result)
        actual_result = sort_nested_object(actual_result)
        self.assertListEqual(expect_result, actual_result)
        # subcase #2: add roles with specified id, attempts to overwrite reserved roles , but ignore at backend
        body[0].update({'id':ROLE_ID_SUPERUSER, 'name':'malicious superuser'})
        body[1].update({'id':ROLE_ID_STAFF,     'name':'fake staff'})
        body[2].update({'id':5566 ,             'name':'internship'})
        response = self._send_request_to_backend(**self.api_call_kwargs)
        self.assertEqual(int(response.status_code), 201)
        result = response.json()
        for idx in range(len(body)):
            self.assertEqual(result[idx]['name'], body[idx]['name'])
            self.assertNotEqual(result[idx]['id'], body[idx]['id'])
## end of class RoleCreationTestCase



class _RoleBaseUpdateTestCase(TransactionTestCase, _BaseMockTestClientInfoMixin, AuthenticateUserMixin):
    def setUp(self):
        self._profile, _ = self._auth_setup(testcase=self, is_superuser=False)
        profile_2nd_data = {'id': 5, 'first_name':'Kyo', 'last_name':'Direnger'}
        self._profile_2nd = GenericUserProfile.objects.create(**profile_2nd_data)
        extra_role_data = [
            {'id':7, 'name':'my role 001',},
            {'id':8, 'name':'my role 002',},
            {'id':9, 'name':'my role 003',},
        ]
        self._roles = _setup_user_roles(profile=self._profile, approved_by=self._profile_2nd,
                extra_role_data=extra_role_data)
        all_perm_objs = PermissionSerializer.get_default_queryset()
        for role in self._roles[2:]:
            perm_objs = all_perm_objs[:3]
            all_perm_objs = all_perm_objs[3:]
            role.permissions.set(perm_objs)
        self.api_call_kwargs = client_req_csrf_setup()
        self._permissions = Permission.objects.all()

    def tearDown(self):
        self._client.cookies.clear()

    def _prepare_access_token(self, new_perms_info, app_labels=None):
        app_labels = app_labels or ['user_management']
        qset = Permission.objects.filter(content_type__app_label__in=app_labels,
                codename__in=new_perms_info)
        if qset.exists():
            self._roles[1].permissions.set(qset)
        acs_tok_resp = self._refresh_access_token(testcase=self, audience=['user_management'])
        access_token = acs_tok_resp['access_token']
        self.api_call_kwargs['headers']['HTTP_AUTHORIZATION'] = ' '.join(['Bearer', access_token])



class RoleUpdateTestCase(_RoleBaseUpdateTestCase):
    path = '/roles'
    def setUp(self):
        super().setUp()
        self.api_call_kwargs.update({'path': self.path, 'method':'put'})

    def test_no_permission(self):
        response = self._send_request_to_backend(**self.api_call_kwargs)
        self.assertEqual(int(response.status_code), 401)
        self._prepare_access_token(new_perms_info=['add_role', 'view_role'])
        response = self._send_request_to_backend(**self.api_call_kwargs)
        self.assertEqual(int(response.status_code), 403)

    def test_input_error(self):
        self._prepare_access_token(new_perms_info=['view_role','change_role'])
        # subcase #1 : empty request data
        body = []
        self.api_call_kwargs['body'] = body
        response = self._send_request_to_backend(**self.api_call_kwargs)
        self.assertEqual(int(response.status_code), 400)
        err_info = response.json()
        self.assertIn('no instance found in update operation', err_info[non_fd_err_key])
        # subcase #2, lacking essential fields
        body = [{fd_name: getattr(role, fd_name) for fd_name in ('id','name')} for role in self._roles[2:]]
        body[0].pop('id', None)
        body[1]['id'] = None
        body[2]['permissions'] = []
        body[1]['permissions'] = []
        body[2]['name'] = ''
        self.api_call_kwargs['body'] = body
        response = self._send_request_to_backend(**self.api_call_kwargs)
        self.assertEqual(int(response.status_code), 400)
        err_info = response.json()
        expect_errmsg_pattern = 'request data cannot be mapped to existing instance'
        pos_errmsg = err_info[0][non_fd_err_key].find(expect_errmsg_pattern)
        self.assertGreater(pos_errmsg, 0)
        pos_errmsg = err_info[1][non_fd_err_key].find(expect_errmsg_pattern)
        self.assertGreater(pos_errmsg, 0)
        self.assertIn('This field may not be blank.', err_info[2]['name'])
        self.assertIn('This list may not be empty.',  err_info[2]['permissions'])
        # subcase #3, attempts to overwrite reserved roles
        body = list(map(lambda role: {'permissions': list(role.permissions.values_list('id', flat=True))}, self._roles[2:]))
        body[0]['id'] = ROLE_ID_SUPERUSER
        body[1]['id'] = ROLE_ID_STAFF
        body[0]['name'] = 'malicious superuser'
        body[1]['name'] = 'fake staff'
        self.api_call_kwargs['body'] = body
        response = self._send_request_to_backend(**self.api_call_kwargs)
        self.assertEqual(int(response.status_code), 400)
        err_info = response.json()
        self.assertIn('no instance found in update operation', err_info[non_fd_err_key])
        # subcase #4, attempts to overwrite reserved roles
        body[2]['id'] = self._roles[2].id
        body[2]['name'] = 'fish story'
        response = self._send_request_to_backend(**self.api_call_kwargs)
        self.assertEqual(int(response.status_code), 400)
        err_info = response.json()
        pos_errmsg = err_info[0][non_fd_err_key].find(expect_errmsg_pattern)
        self.assertGreater(pos_errmsg, 0)
        pos_errmsg = err_info[1][non_fd_err_key].find(expect_errmsg_pattern)
        self.assertGreater(pos_errmsg, 0)

    def test_bulk_ok(self):
        self._prepare_access_token(new_perms_info=['view_role','change_role'])
        body = list(map(_srlz_fn, self._roles[2:]))
        all_perm_ids = self._permissions.filter(codename__contains='generic').values_list('id', flat=True)
        for item in body:
            perm_ids = all_perm_ids[:3]
            all_perm_ids = all_perm_ids[3:]
            item['permissions'].extend(perm_ids)
        body[0]['name'] = 'b plus tree'
        body[1]['name'] = 'quick sort'
        body[2]['name'] = 'Dijkstra'
        self.api_call_kwargs['body'] = body
        response = self._send_request_to_backend(**self.api_call_kwargs)
        self.assertEqual(int(response.status_code), 200)
        tuple(map(lambda role: role.refresh_from_db(), self._roles))
        expect_result = body
        actual_result = list(map(_srlz_fn, self._roles[2:]))
        expect_result = sort_nested_object(expect_result)
        actual_result = sort_nested_object(actual_result)
        self.assertListEqual(expect_result, actual_result)
## end of class RoleUpdateTestCase



class RoleDeletionTestCase(_RoleBaseUpdateTestCase):
    path = '/roles'
    def setUp(self):
        super().setUp()
        self.api_call_kwargs.update({'path': self.path, 'method':'delete'})

    def test_no_permission(self):
        response = self._send_request_to_backend(**self.api_call_kwargs)
        self.assertEqual(int(response.status_code), 401)
        self._prepare_access_token(new_perms_info=['view_emailaddress', 'view_role'])
        response = self._send_request_to_backend(**self.api_call_kwargs)
        self.assertEqual(int(response.status_code), 403)

    def test_invalid_ids(self):
        self._prepare_access_token(new_perms_info=['view_role', 'delete_role'])
        # subcase #1 : attempt to delete reserved role(s)
        delete_ids = list(map(lambda idx: self._roles[idx].id , [0,2,4]))
        self.assertIn(ROLE_ID_STAFF, delete_ids)
        self.api_call_kwargs['ids'] = delete_ids
        response = self._send_request_to_backend(**self.api_call_kwargs)
        self.assertEqual(int(response.status_code), 409)
        err_info = response.json()
        expect_err_msg = 'not allowed to delete preserved role ID = {%s}' % ROLE_ID_STAFF
        self.assertIn(expect_err_msg , err_info[non_fd_err_key])
        # subcase #2 : text id which contains English letter
        delete_ids.append('32s8')
        response = self._send_request_to_backend(**self.api_call_kwargs)
        self.assertEqual(int(response.status_code), 400)
        err_info = response.json()
        self.assertEqual('ids field has to be a list of number', err_info['detail'])

    def test_bulk_ok(self):
        self._prepare_access_token(new_perms_info=['view_role', 'delete_role'])
        delete_ids = list(map(lambda idx: self._roles[idx].id , [2,4]))
        self.api_call_kwargs['ids'] = delete_ids
        response = self._send_request_to_backend(**self.api_call_kwargs)
        self.assertEqual(int(response.status_code), 204)
        role_cnt = Role.objects.filter(id__in=delete_ids).count()
        self.assertEqual(role_cnt , 0)


class RoleQueryTestCase(_RoleBaseUpdateTestCase):
    path = ['/roles', '/role/%s']

    def setUp(self):
        super().setUp()
        # create extra roles which are NOT applied to an user, user
        # can all the existing roles including those unapplied only if
        # the user has `view_role` low-level permission
        extra_role_data = [
            {'id':10, 'name':'unapplied role 001',},
            {'id':11, 'name':'unapplied role 002',},
            {'id':12, 'name':'unapplied role 003',},
        ]
        self._unapplied_roles  = tuple(map(lambda d:Role.objects.create(**d) , extra_role_data))
        perms = self._permissions.filter(codename__contains='phone')
        perms_iter = iter(perms)
        for ur in self._unapplied_roles:
            perm = next(perms_iter)
            ur.permissions.set([perm])

    def test_read_roles_applied_only(self):
        self.api_call_kwargs.update({'path': self.path[0], 'method':'get'})
        response = self._send_request_to_backend(**self.api_call_kwargs)
        self.assertEqual(int(response.status_code), 401)
        self._prepare_access_token(new_perms_info=[])
        response = self._send_request_to_backend(**self.api_call_kwargs)
        self.assertEqual(int(response.status_code), 200)
        expect_result = list(map(_srlz_fn, self._roles))
        actual_result = response.json()
        expect_result = sort_nested_object(expect_result)
        actual_result = sort_nested_object(actual_result)
        self.assertListEqual(expect_result, actual_result)


    def test_read_all_roles(self):
        self.api_call_kwargs.update({'path': self.path[0], 'method':'get'})
        self._prepare_access_token(new_perms_info=['view_role'])
        response = self._send_request_to_backend(**self.api_call_kwargs)
        self.assertEqual(int(response.status_code), 200)
        applied_role_data   = list(map(_srlz_fn, self._roles))
        unapplied_role_data = list(map(_srlz_fn, self._unapplied_roles))
        expect_result = []
        expect_result.extend(applied_role_data)
        expect_result.extend(unapplied_role_data)
        actual_result = response.json()
        expect_result = sort_nested_object(expect_result)
        actual_result = sort_nested_object(actual_result)
        self.assertListEqual(expect_result, actual_result)

    def test_read_single_role(self):
        chosen_role = self._unapplied_roles[0]
        path = self.path[1] % chosen_role.id
        self.api_call_kwargs.update({'path': path, 'method':'get'})
        # subcase #1: read unapplied role without `view_role` permission
        self._prepare_access_token(new_perms_info=[])
        response = self._send_request_to_backend(**self.api_call_kwargs)
        self.assertEqual(int(response.status_code), 403)
        # subcase #2: read unapplied role with `view_role` permission
        self._prepare_access_token(new_perms_info=['view_role'])
        response = self._send_request_to_backend(**self.api_call_kwargs)
        self.assertEqual(int(response.status_code), 200)
        expect_result = _srlz_fn(chosen_role)
        actual_result = response.json()
        self.assertDictEqual(expect_result, actual_result)


