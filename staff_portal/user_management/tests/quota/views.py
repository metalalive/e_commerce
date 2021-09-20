import random
import json

from django.test import TransactionTestCase
from django.contrib.auth.models import Permission as AuthPermission, User as AuthUser, Group as AuthRole, LoginAccountRoleRelation
from django.contrib.contenttypes.models  import ContentType
from rest_framework.settings    import api_settings as drf_settings

from common.util.python import sort_nested_object
from user_management.models import QuotaUsageType, GenericUserProfile, GenericUserAuthRelation

from tests.python.common import HttpRequestDataGen
from tests.python.common.django import _BaseMockTestClientInfoMixin
from user_management.tests.common import _fixtures, AuthCheckMixin


class QuotaMaterialQueryTestCase(TransactionTestCase, _BaseMockTestClientInfoMixin, AuthCheckMixin):
    path = '/quota_material'

    def test_unauthenticated_access(self):
        response = self._send_request_to_backend(path=self.path, method='get')
        self.assertEqual(int(response.status_code), 403)

    def test_invalid_account_without_profile(self):
        super().test_invalid_account_without_profile(testcase=self, path=self.path, methods=('get',))

    def test_inactive_staff(self):
        super().test_inactive_staff(testcase=self, path=self.path, methods=('get',))

    def test_unauthorized_user(self):
        super().test_unauthorized_user(testcase=self, path=self.path, methods=('get',))

    def test_unauthorized_staff(self):
        super().test_unauthorized_staff(testcase=self, path=self.path, methods=('get',))

    def test_superuser(self):
        _account_info = _fixtures['AuthUser'][0].copy()
        _account_info.update({'is_active':True, 'is_superuser':True, 'is_staff':True})
        account = AuthUser.objects.create_superuser(**_account_info)
        profile = GenericUserProfile(**_fixtures['GenericUserProfile'][0])
        profile.save()
        auth_rel = GenericUserAuthRelation(profile=profile, login=account)
        auth_rel.save()
        http_forwarded = self._forwarded_pattern % _account_info['username']
        headers = {'HTTP_FORWARDED': http_forwarded,}
        response = self._send_request_to_backend(path=self.path, method='get', headers=headers)
        self._validate_materials(response)
        for method in ('post','put','patch','delete'):
            response = self._send_request_to_backend(path=self.path, method=method, headers=headers)
            self.assertEqual(int(response.status_code), 405)

    def test_authorized_staff(self):
        _account_info = _fixtures['AuthUser'][0].copy()
        _account_info.update({'is_active':True, 'is_superuser':False, 'is_staff':True})
        account = AuthUser.objects.create_user(**_account_info)
        profile = GenericUserProfile.objects.create(**_fixtures['GenericUserProfile'][0])
        auth_rel = GenericUserAuthRelation.objects.create(profile=profile, login=account)
        min_perms = AuthPermission.objects.filter(codename='view_contenttype')
        role = AuthRole.objects.create(id=5, name='can view quota materials')
        role.permissions.set(min_perms)
        LoginAccountRoleRelation.objects.create(id=5, role=role, account=account)
        http_forwarded = self._forwarded_pattern % _account_info['username']
        headers = {'HTTP_FORWARDED': http_forwarded,}
        response = self._send_request_to_backend(path=self.path, method='get', headers=headers)
        self._validate_materials(response)


    def _validate_materials(self, response):
        self.assertEqual(int(response.status_code), 200)
        materials = response.json()
        for key, value in materials.items():
            expect_value = ContentType.objects.filter(app_label=key).values('id','model')
            expect_value = sort_nested_object(list(expect_value))
            actual_value = sort_nested_object(value)
            expect_value = json.dumps(expect_value)
            actual_value = json.dumps(actual_value)
            self.assertEqual(expect_value, actual_value)
## end of class QuotaMaterialQueryTestCase


class QuotaDenyAccessTestCase(TransactionTestCase, _BaseMockTestClientInfoMixin, AuthCheckMixin):
    path = '/quota'

    def test_unauthenticated_access(self):
        response = self._send_request_to_backend(path=self.path, method='post', body=[])
        self.assertEqual(int(response.status_code), 403)

    def test_invalid_account_without_profile(self):
        super().test_invalid_account_without_profile(testcase=self, path=self.path, methods=('post','put','delete'))

    def test_inactive_staff(self):
        super().test_inactive_staff(testcase=self, path=self.path, methods=('post','put','delete'))

    def test_unauthorized_user(self):
        super().test_unauthorized_user(testcase=self, path=self.path, methods=('post','put','delete'))

    def test_unauthorized_staff(self):
        super().test_unauthorized_staff(testcase=self, path=self.path, methods=('post','put','delete'))
## end of class QuotaDenyAccessTestCase


class _PermissionSetupMixin:
    def _setup_account(self):
        _account_info = _fixtures['AuthUser'][0].copy()
        _account_info.update({'is_active':True, 'is_superuser':False, 'is_staff':True})
        account = AuthUser.objects.create_user(**_account_info)
        profile = GenericUserProfile.objects.create(**_fixtures['GenericUserProfile'][0])
        auth_rel = GenericUserAuthRelation.objects.create(profile=profile, login=account)
        self._account_info = _account_info
        return account

    def _setup_role(self, account, perms_cond, role_name):
        min_perms = AuthPermission.objects.filter(**perms_cond)
        role = AuthRole.objects.create(id=5, name=role_name)
        role.permissions.set(min_perms)
        LoginAccountRoleRelation.objects.create(id=6, role=role, account=account)
        return role


class QuotaBaseViewTestCase(TransactionTestCase, _BaseMockTestClientInfoMixin, _PermissionSetupMixin):
    path = '/quota'

    def setUp(self):
        app_label = 'user_management'
        quota_info = [
                ('useremailaddress', 'max number of email addresses'),
                ('userlocation'    , 'max number of geographic locations'),
                ('userphonenumber' , 'max number of phone numbers'),
                ('genericuserprofile','max # new users created by current user'),
            ]
        def _gen_req_dataitem(info):
            model_ct = ContentType.objects.filter(app_label=app_label, model=info[0]).first()
            return {'material': model_ct.pk, 'label':info[1]}
        self.request_data = list(map(_gen_req_dataitem, quota_info))



class QuotaCreationTestCase(QuotaBaseViewTestCase):
    def setUp(self):
        super().setUp()
        account = self._setup_account()
        self._setup_role(account, perms_cond={'codename':'add_quotausagetype'},
                role_name='can add quota usage type')

    def test_bulk_ok(self):
        http_forwarded = self._forwarded_pattern % self._account_info['username']
        headers = {'HTTP_FORWARDED': http_forwarded,}
        response = self._send_request_to_backend(path=self.path, method='post', headers=headers,
                body=self.request_data, expect_shown_fields=['id','label'])
        self.assertEqual(int(response.status_code), 201)
        created_items = response.json()
        self.assertEqual(len(created_items), len(self.request_data))


    def test_invalid_label(self):
        http_forwarded = self._forwarded_pattern % self._account_info['username']
        headers = {'HTTP_FORWARDED': http_forwarded,}
        # empty
        self.request_data[-1].pop('label',None)
        response = self._send_request_to_backend(path=self.path, method='post', headers=headers,
                body=self.request_data, expect_shown_fields=['id','label'])
        self.assertEqual(int(response.status_code), 400)
        err_info = response.json()
        self.assertEqual(err_info[-1]['label'][0] , 'This field is required.')
        # label too long
        self.request_data[-1]['label'] = 'maximum number of new users created by current authenticated user'
        response = self._send_request_to_backend(path=self.path, method='post', headers=headers,
                body=self.request_data, expect_shown_fields=['id','label'])
        self.assertEqual(int(response.status_code), 400)
        err_info = response.json()
        self.assertEqual(err_info[-1]['label'][0] , 'Ensure this field has no more than 50 characters.')


    def test_bulk_material_conflict(self):
        http_forwarded = self._forwarded_pattern % self._account_info['username']
        headers = {'HTTP_FORWARDED': http_forwarded,}
        dup_material = self.request_data[0]['material']
        self.request_data[-1]['material'] = self.request_data[0]['material']
        response = self._send_request_to_backend(path=self.path, method='post', headers=headers,
                body=self.request_data, expect_shown_fields=['id','label'])
        self.assertEqual(int(response.status_code), 400)
        err_info = response.json()
        err_msg = err_info[-1]['material'][0]
        pos = err_msg.find('duplicate entry')
        self.assertGreater(pos,0)
        pos = err_msg.find(str(dup_material))
        self.assertGreater(pos,0)
## end of class QuotaCreationTestCase


class QuotaUpdateTestCase(QuotaBaseViewTestCase):
    def setUp(self):
        super().setUp()
        account = self._setup_account()
        self._setup_role(account, perms_cond={'codename__in':['change_quotausagetype',
            'add_quotausagetype']},  role_name='can add & edit quota usage type in bulk')
        http_forwarded = self._forwarded_pattern % self._account_info['username']
        headers = {'HTTP_FORWARDED': http_forwarded,}
        response = self._send_request_to_backend(path=self.path, method='post', headers=headers,
                body=self.request_data, expect_shown_fields=['id','label','material'])
        self.assertEqual(int(response.status_code), 201)
        self.request_data = response.json()

    def test_bulk_ok(self):
        http_forwarded = self._forwarded_pattern % self._account_info['username']
        headers = {'HTTP_FORWARDED': http_forwarded,}
        self.request_data[0]['label'] = 'max # emails'
        model_ct = ContentType.objects.filter(app_label='user_management', model='userquotarelation').first()
        self.request_data[1]['material'] = model_ct.pk
        self.request_data[1]['label'] = 'max # quota types applied'
        response = self._send_request_to_backend(path=self.path, method='put', headers=headers,
                body=self.request_data[:2], expect_shown_fields=['id','label','material'])
        self.assertEqual(int(response.status_code), 200)
        edited_items = response.json()
        self.assertEqual(len(edited_items), 2)
        expect_value = sort_nested_object(self.request_data[:2])
        actual_value = sort_nested_object(edited_items)
        expect_value = json.dumps(expect_value)
        actual_value = json.dumps(actual_value)
        self.assertEqual(expect_value, actual_value)

    def test_bulk_material_conflict(self):
        http_forwarded = self._forwarded_pattern % self._account_info['username']
        headers = {'HTTP_FORWARDED': http_forwarded,}
        dup_material = self.request_data[0]['material']
        self.request_data[-1]['material'] = self.request_data[0]['material']
        response = self._send_request_to_backend(path=self.path, method='put', headers=headers,
                body=self.request_data[1:],)
        self.assertEqual(int(response.status_code), 400)
        err_info = response.json()
        err_msg = err_info[-1]['material'][0]
        self.assertEqual(err_msg, 'This field must be unique.')


class QuotaDeletionTestCase(QuotaBaseViewTestCase):
    def setUp(self):
        super().setUp()
        account = self._setup_account()
        self._setup_role(account, perms_cond={'codename__in':['delete_quotausagetype',
            'add_quotausagetype']},  role_name='can add & delete quota usage type in bulk')
        http_forwarded = self._forwarded_pattern % self._account_info['username']
        headers = {'HTTP_FORWARDED': http_forwarded,}
        response = self._send_request_to_backend(path=self.path, method='post', headers=headers,
                body=self.request_data, expect_shown_fields=['id','label'])
        self.assertEqual(int(response.status_code), 201)
        self.request_data = response.json()

    def test_bulk_ok(self):
        http_forwarded = self._forwarded_pattern % self._account_info['username']
        headers = {'HTTP_FORWARDED': http_forwarded,}
        all_ids    = tuple(map(lambda d:d['id'], self.request_data))
        remain_ids = tuple(map(lambda d:d['id'], self.request_data[2:]))
        delete_ids = tuple(map(lambda d:d['id'], self.request_data[:2]))
        response = self._send_request_to_backend(path=self.path, method='delete',
                headers=headers, ids=delete_ids,)
        self.assertEqual(int(response.status_code), 204)
        actual_ids = QuotaUsageType.objects.filter(id__in=all_ids).values_list('id', flat=True)
        self.assertSetEqual(set(remain_ids),set(actual_ids))

    def test_without_specifying_id(self):
        http_forwarded = self._forwarded_pattern % self._account_info['username']
        headers = {'HTTP_FORWARDED': http_forwarded,}
        response = self._send_request_to_backend(path=self.path, method='delete',
                headers=headers,)
        self.assertEqual(int(response.status_code), 400)
        err_info = response.json()
        non_field_err_key = drf_settings.NON_FIELD_ERRORS_KEY
        err_msg = err_info[non_field_err_key][0]
        self.assertEqual(err_msg, 'not specify any ID, no object is deleted')


