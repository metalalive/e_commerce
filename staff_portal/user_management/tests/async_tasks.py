import string
import random
from datetime import time
from unittest.mock import patch

from MySQLdb.constants.ER import NO_SUCH_TABLE
from celery import states as CeleryStates

from django.conf     import settings as django_settings
from django.db.utils import ProgrammingError
from django.test     import TransactionTestCase
from django.contrib.contenttypes.models  import ContentType
from django.contrib.auth.models import Permission as ModelLevelPermission

from common.models.enums.django import AppCodeOptions
from user_management.models.auth import Role
from user_management.models.base import QuotaMaterial, GenericUserProfile, GenericUserGroup
from user_management.serializers.common import serialize_profile_quota, serialize_profile_permissions
from user_management.async_tasks import get_profile

from .common import _fixtures, UserNestedFieldSetupMixin, UserNestedFieldVerificationMixin


class GetProfileCase(TransactionTestCase, UserNestedFieldSetupMixin, UserNestedFieldVerificationMixin):
    num_profiles = 8

    _permission_info = {
        'store':['storeprofile', 'storestaff', 'storeproductavail'],
        'order':['orderinvoice', 'orderreceipt', 'orderreturn']
    }

    def _setup_extra_permission_info(self):
        ct_objs = []
        perm_objs = []
        actions = ('add','change','delete','view')
        for app_label, model_name_list in self._permission_info.items():
            data = map(lambda name:{'model':name, 'app_label':app_label}, model_name_list)
            objs = map(lambda d:ContentType.objects.create(**d), data)
            ct_objs.extend(list(objs))
        for action in actions:
            data = map(lambda ct:{'codename':'%s_%s' % (action, ct.model),
                'name': ct.model,  'content_type':ct}, ct_objs)
            objs = map(lambda d:ModelLevelPermission.objects.create(**d), data)
            perm_objs.extend(list(objs))
        return ct_objs, perm_objs

    def _teardown_extra_permission_info(self, ct_objs, perm_objs):
        try:
            tuple(map(lambda obj:obj.delete(), perm_objs))
            tuple(map(lambda obj:obj.delete(), ct_objs))
        except ProgrammingError as e:
            if e.args[0] == NO_SUCH_TABLE and e.args[1].find('auth_group_permissions') > 0:
                pass
            else:
                raise

    def init_primitive(self):
        keys = (Role, QuotaMaterial, GenericUserGroup)
        data_map = dict(map(lambda cls: (cls, _fixtures[cls]), keys))
        objs = {k_cls: list(map(lambda d: k_cls(**d), data)) for k_cls, data in data_map.items()}
        for cls in keys:
            cls.objects.bulk_create(objs[cls])
        # the profiles that were already created, will be used to create another new profiles
        # through serializer or API endpoint
        default_profile_data = _fixtures[GenericUserProfile][:self.num_profiles]
        objs[GenericUserProfile] = list(map(lambda d: GenericUserProfile(**d), default_profile_data))
        GenericUserProfile.objects.bulk_create(objs[GenericUserProfile])
        return objs

    def _assign_permissions_to_roles(self, roles, perms):
        for role in roles:
            num_perms = random.randrange(1, len(perms) - 2)
            chosen_perms = random.choices(perms, k=num_perms)
            role.permissions.set(chosen_perms)


    def setUp(self):
        ct_objs, perm_objs = self._setup_extra_permission_info()
        self._extra_apps_perms = (ct_objs, perm_objs)
        self._primitives = self.init_primitive()
        self._assign_permissions_to_roles(roles=self._primitives[Role], perms=perm_objs)
        for profile in self._primitives[GenericUserProfile]:
            applied_role_data  = self._gen_roles(role_objs=self._primitives[Role], num=3, serializable=False)
            applied_quota_data = self._gen_quota(quota_mat_objs=self._primitives[QuotaMaterial], num=3, serializable=False)
            applied_email_data = self._gen_emails(num=4)
            applied_phone_data = self._gen_phones(num=4)
            for data in applied_role_data:
                data['approved_by'] = self._primitives[GenericUserProfile][0]
            for data in applied_email_data:
                data.pop('id', None)
            for data in applied_phone_data:
                data.pop('id', None)
            # TODO, randomly create login account
            tuple(map(lambda d: profile.roles.create(**d), applied_role_data ))
            tuple(map(lambda d: profile.quota.create(**d), applied_quota_data))
            tuple(map(lambda d: profile.emails.create(**d), applied_email_data))
            tuple(map(lambda d: profile.phones.create(**d), applied_phone_data))
        get_profile.app.conf.task_always_eager = True


    def tearDown(self):
        get_profile.app.conf.task_always_eager = False
        self._teardown_extra_permission_info(*self._extra_apps_perms)


    def test_success(self):
        num_rounds = 30
        for _ in range(num_rounds):
            chosen_profiles = random.choices(self._primitives[GenericUserProfile], k=3)
            prof_ids = list(map(lambda obj:obj.id, chosen_profiles))
            app_labels = list(self._permission_info.keys())
            chosen_app_label = random.choice(app_labels)
            input_kwargs = {'ids':prof_ids, 'fields':['id', 'roles', 'quota']}
            eager_result = get_profile.apply_async(kwargs=input_kwargs, headers={'src_app':chosen_app_label})
            self.assertEqual(eager_result.state, CeleryStates.SUCCESS)
            for data in eager_result.result:
                actual_perms = data.get('perms')
                actual_quota = data.get('quota')
                self.assertIsNotNone(actual_perms)
                self.assertIsNotNone(actual_quota)
                expect_profile = next(filter(lambda obj:obj.id == data['id'], chosen_profiles))
                expect_perms   = serialize_profile_permissions(expect_profile, app_labels=[chosen_app_label])
                expect_quota   = serialize_profile_quota(expect_profile, app_labels=[chosen_app_label])
                actual_perms = sorted(actual_perms, key=lambda d:d['codename'])
                expect_perms = sorted(expect_perms, key=lambda d:d['codename'])
                actual_quota = sorted(actual_quota, key=lambda d:d['mat_code'])
                expect_quota = sorted(expect_quota, key=lambda d:d['mat_code'])
                self.assertListEqual(actual_perms , expect_perms)
                self.assertListEqual(actual_quota , expect_quota)
            # ----------------------------
            input_kwargs = {'ids':prof_ids, 'fields':['id', 'emails', 'phones']}
            eager_result = get_profile.apply_async(kwargs=input_kwargs, headers={'src_app':chosen_app_label})
            self.assertEqual(eager_result.state, CeleryStates.SUCCESS)
            for data in eager_result.result:
                actual_emails = data.get('emails')
                actual_phones = data.get('phones')
                self.assertIsNotNone(actual_emails)
                self.assertIsNotNone(actual_phones)
                expect_profile = next(filter(lambda obj:obj.id == data['id'], chosen_profiles))
                expect_emails = expect_profile.emails.values('id', 'addr')
                expect_phones = expect_profile.phones.values('id', 'country_code', 'line_number')
                actual_emails = sorted(actual_emails, key=lambda d:d['id'])
                actual_phones = sorted(actual_phones, key=lambda d:d['id'])
                expect_emails = sorted(expect_emails, key=lambda d:d['id'])
                expect_phones = sorted(expect_phones, key=lambda d:d['id'])
                self.assertListEqual(actual_emails , expect_emails)
                self.assertListEqual(actual_phones , expect_phones)
        ## end of loop
    ## end of test_success()


    def test_invalid_id(self):
        chosen_profiles = random.choices(self._primitives[GenericUserProfile], k=3)
        prof_ids = list(map(lambda obj:obj.id, chosen_profiles))
        prof_ids.append('1xx')
        chosen_app_label = 'non_existent_service'
        input_kwargs = {'ids':prof_ids, 'fields':['id', 'roles', 'emails']}
        eager_result = get_profile.apply_async(kwargs=input_kwargs, headers={'src_app':chosen_app_label})
        self.assertEqual(eager_result.state, CeleryStates.FAILURE)
        self.assertTrue(isinstance(eager_result.result, ValueError))
        pos = eager_result.result.args[0].find('1xx')
        self.assertGreater(pos, 0)

    def test_nonexist_app_label(self):
        chosen_profiles = random.choices(self._primitives[GenericUserProfile], k=3)
        prof_ids = list(map(lambda obj:obj.id, chosen_profiles))
        headers = {}
        input_kwargs = {'ids':prof_ids, 'fields':['id', 'roles', 'quota']}
        eager_result = get_profile.apply_async(kwargs=input_kwargs, headers=headers)
        self.assertEqual(eager_result.state, CeleryStates.FAILURE)
        self.assertTrue(isinstance(eager_result.result, KeyError))
        chosen_app_label = 'non_existent_service'
        headers = {'src_app':chosen_app_label}
        eager_result = get_profile.apply_async(kwargs=input_kwargs, headers=headers)
        self.assertEqual(eager_result.state, CeleryStates.FAILURE)
        self.assertTrue(isinstance(eager_result.result, ValueError))
        expect_err_msg = 'receive invalid app_label %s' % chosen_app_label
        actual_err_msg = eager_result.result.args[0]
        self.assertEqual(expect_err_msg, actual_err_msg)

## from common.util.python.messaging.rpc import RPCproxy
## auth_app_rpc = RPCproxy(dst_app_name='user_management', src_app_name='store')
## reply_evt = auth_app_rpc.get_profile(ids=[2,3,4] , fields=['id', 'roles', 'quota'])
## reply_evt.refresh(retry=False, timeout=0.6)
## reply_evt.result['result']
