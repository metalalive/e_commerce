import string
import random
import copy
import json
from datetime import timedelta
from unittest.mock import Mock, patch

from django.test import TransactionTestCase
from django.utils import timezone as django_timezone
from rest_framework.exceptions import ValidationError as DRFValidationError
from rest_framework.settings import DEFAULTS as drf_default_settings

from user_management.models.common import AppCodeOptions
from user_management.models.auth import Role, LoginAccount
from user_management.models.base import QuotaMaterial, GenericUserProfile, GenericUserGroup, GenericUserGroupClosure, UserQuotaRelation
from user_management.serializers.nested import GroupAssignValidator
from user_management.serializers        import LoginAccountExistField

from tests.python.common import listitem_rand_assigner, rand_gen_request_body
from ..common import  _fixtures, gen_expiry_time, _setup_login_account
from  .common import  HttpRequestDataGenProfile, ProfileVerificationMixin, _nested_field_names

non_field_err_key = drf_default_settings['NON_FIELD_ERRORS_KEY']

class ProfileCommonTestCase(TransactionTestCase, HttpRequestDataGenProfile, ProfileVerificationMixin):
    usermgt_material_data = tuple(filter(lambda d:d['app_code'] == AppCodeOptions.user_management, _fixtures[QuotaMaterial]))

    def _setup_groups(self):
        """
        the tree structure of the group hierarchy in this test file

                   3             8         11
                 /    \        /  \       /  \
                4      5      9   10     12   13
               / \                           /
              6   7                         14

        """
        grp_obj_map = dict(map(lambda obj: (obj.id, obj), self._primitives[GenericUserGroup]))
        group_closure_data = [
            {'id':1,  'depth':0, 'ancestor':grp_obj_map[3], 'descendant':grp_obj_map[3]},
            {'id':2,  'depth':0, 'ancestor':grp_obj_map[4], 'descendant':grp_obj_map[4]},
            {'id':3,  'depth':0, 'ancestor':grp_obj_map[5], 'descendant':grp_obj_map[5]},
            {'id':4,  'depth':0, 'ancestor':grp_obj_map[6], 'descendant':grp_obj_map[6]},
            {'id':5,  'depth':0, 'ancestor':grp_obj_map[7], 'descendant':grp_obj_map[7]},
            {'id':6,  'depth':1, 'ancestor':grp_obj_map[3], 'descendant':grp_obj_map[4]},
            {'id':7,  'depth':1, 'ancestor':grp_obj_map[3], 'descendant':grp_obj_map[5]},
            {'id':8,  'depth':1, 'ancestor':grp_obj_map[4], 'descendant':grp_obj_map[6]},
            {'id':9,  'depth':1, 'ancestor':grp_obj_map[4], 'descendant':grp_obj_map[7]},
            {'id':10, 'depth':2, 'ancestor':grp_obj_map[3], 'descendant':grp_obj_map[6]},
            {'id':11, 'depth':2, 'ancestor':grp_obj_map[3], 'descendant':grp_obj_map[7]},
            # ---------
            {'id':12, 'depth':0, 'ancestor':grp_obj_map[8],  'descendant':grp_obj_map[8]},
            {'id':13, 'depth':0, 'ancestor':grp_obj_map[9],  'descendant':grp_obj_map[9]},
            {'id':14, 'depth':0, 'ancestor':grp_obj_map[10], 'descendant':grp_obj_map[10]},
            {'id':15, 'depth':1, 'ancestor':grp_obj_map[8],  'descendant':grp_obj_map[9]},
            {'id':16, 'depth':1, 'ancestor':grp_obj_map[8],  'descendant':grp_obj_map[10]},
            # ---------
            {'id':17, 'depth':0, 'ancestor':grp_obj_map[11],  'descendant':grp_obj_map[11]},
            {'id':18, 'depth':0, 'ancestor':grp_obj_map[12],  'descendant':grp_obj_map[12]},
            {'id':19, 'depth':0, 'ancestor':grp_obj_map[13],  'descendant':grp_obj_map[13]},
            {'id':20, 'depth':0, 'ancestor':grp_obj_map[14],  'descendant':grp_obj_map[14]},
            {'id':21, 'depth':1, 'ancestor':grp_obj_map[11],  'descendant':grp_obj_map[12]},
            {'id':22, 'depth':1, 'ancestor':grp_obj_map[11],  'descendant':grp_obj_map[13]},
            {'id':23, 'depth':1, 'ancestor':grp_obj_map[13],  'descendant':grp_obj_map[14]},
            {'id':24, 'depth':2, 'ancestor':grp_obj_map[11],  'descendant':grp_obj_map[14]},
        ]
        list(map(lambda d: GenericUserGroupClosure.objects.create(**d) , group_closure_data))
        return grp_obj_map

    def setUp(self):
        self.init_primitive()
        self._grp_map = self._setup_groups()
        roles_without_superuser = self._primitives[Role]
        self._default_login_profile = _setup_login_account(account_data=_fixtures[LoginAccount][0],
                profile_obj=self._primitives[GenericUserProfile][0] , roles=roles_without_superuser )
        self.assertEqual(self._default_login_profile.privilege_status , GenericUserProfile.STAFF)
        top_grps = (self._grp_map[3], self._grp_map[8], self._grp_map[11])
        self._refresh_applied_groups(profile=self._default_login_profile, groups=top_grps)
        # the default login user can have at most 3 emails
        quota_data = {'expiry':gen_expiry_time(), 'maxnum':3, 'material':self._primitives[QuotaMaterial][0] }
        self._default_login_profile.quota.create(**quota_data)

    def tearDown(self):
        pass

    def _refresh_applied_groups(self, profile, groups):
        approved_by = self._primitives[GenericUserProfile][1]
        profile.groups.all(with_deleted=True).delete(hard=True)
        for grp_obj in groups:
            profile.groups.create(group=grp_obj, approved_by=approved_by)

    def _test_edit_profile_without_groups(self, instance=None):
        req_data = self.request_data
        req_data[0]['groups'].clear()
        # subcase #1: if current logged-in user is NOT superuser
        kwargs = {'many':True, 'data':req_data, 'instance': instance, 'account':self._default_login_profile.account}
        serializer = self.serializer_class(**kwargs)
        error_caught = None
        with self.assertRaises(DRFValidationError):
            try:
                serializer.is_valid(raise_exception=True)
            except DRFValidationError as e:
                error_caught = e
                raise
        self.assertIsNotNone(error_caught)
        err_info = error_caught.detail
        expect_errmsg = 'non-admin user has to select at least one group for the new profile'
        actual_errmsg = err_info[0]['groups'][0]
        self.assertEqual(expect_errmsg, actual_errmsg)
        # subcase #2: if current logged-in user is superuser
        su_role = Role.objects.get_or_create(id=GenericUserProfile.SUPERUSER , name='super_user_role')
        role_rel_data = {'role':su_role[0], 'expiry': gen_expiry_time(), 'approved_by':self._primitives[GenericUserProfile][1] }
        self._default_login_profile.roles.create(**role_rel_data)
        self._default_login_profile.account.is_superuser = True
        self._default_login_profile.account.save(update_fields=['is_superuser'])
        self.assertEqual(self._default_login_profile.privilege_status , GenericUserProfile.SUPERUSER)
        serializer = self.serializer_class(**kwargs)
        validate_result = serializer.is_valid(raise_exception=True)
        self.assertTrue(validate_result)

    def _test_duplicate_groups(self, instance=None):
        req_data = self.request_data
        dup_grp_id = req_data[0]['groups'][0]['group']
        req_data[0]['groups'][1]['group'] = dup_grp_id
        kwargs = {'many': True, 'data':req_data, 'instance':instance, 'account':self._default_login_profile.account}
        serializer = self.serializer_class(**kwargs)
        error_caught = None
        with self.assertRaises(DRFValidationError):
            try:
                serializer.is_valid(raise_exception=True)
            except DRFValidationError as e:
                error_caught = e
                raise
        self.assertIsNotNone(error_caught)
        err_info = error_caught.detail
        expect_errmsg  = err_info[0]['groups'][non_field_err_key][0]
        reason_pattern = 'duplicate item found in the list'
        self.assertGreater(expect_errmsg.find(reason_pattern), 0)
## end of class ProfileCommonTestCase


class ProfileCreationTestCase(ProfileCommonTestCase):
    num_roles = 2
    num_quota = 3
    num_groups = 3

    def setUp(self):
        super().setUp()
        num_profiles = 3
        profile_data_for_test = _fixtures[GenericUserProfile][self.num_default_profiles : ]
        profs_data_gen = listitem_rand_assigner(list_=profile_data_for_test,
                min_num_chosen=num_profiles, max_num_chosen=(num_profiles+1))
        request_data = rand_gen_request_body(customize_item_fn=self.customize_req_data_item, data_gen=profs_data_gen)
        self.request_data = list(request_data)

    def test_bulk_ok(self):
        req_data = self.request_data
        serializer = self.serializer_class(many=True, data=req_data, account=self._default_login_profile.account)
        serializer.is_valid(raise_exception=True)
        actual_instances = serializer.save()
        self.verify_data(actual_data=actual_instances, expect_data=self.request_data)

    def test_non_support_groups(self):
        # a user A at higher management position attempts to assign groups she doesn't have
        non_top_grps = (self._grp_map[4], self._grp_map[10], self._grp_map[13])
        top_grp_ids = (3, 8, 11)
        self._refresh_applied_groups(profile=self._default_login_profile, groups=non_top_grps)
        req_data = self.request_data
        for idx in range(len(top_grp_ids)):
            req_data[idx]['groups'][idx]['group'] = top_grp_ids[idx]
        serializer = self.serializer_class(many=True, data=req_data, account=self._default_login_profile.account)
        error_caught = None
        with self.assertRaises(DRFValidationError):
            try:
                serializer.is_valid(raise_exception=True)
            except DRFValidationError as e:
                error_caught = e
                raise
        self.assertIsNotNone(error_caught)
        err_info = error_caught.detail
        for idx in range(len(top_grp_ids)):
            expect_errmsg = GroupAssignValidator.err_msg_pattern % top_grp_ids[idx]
            actual_errmsg = err_info[idx]['groups'][idx]['group'][0]
            self.assertEqual(expect_errmsg, actual_errmsg)

    def test_duplicate_groups(self):
        self._test_duplicate_groups()

    def test_create_new_profile_without_groups(self):
        self._test_edit_profile_without_groups()


    def test_exceeds_quota_limit(self):
        req_data = self.request_data
        for data_item in req_data:
            data_item['quota'].clear()
        _info_map = {
            QuotaMaterial._MatCodeOptions.MAX_NUM_EMAILS.value: \
                    {'maxnum': 3, 'data_item':req_data[0], 'field':'emails', 'data':self._gen_emails(num=4)},
            QuotaMaterial._MatCodeOptions.MAX_NUM_PHONE_NUMBERS.value: \
                    {'maxnum': 2, 'data_item':req_data[1], 'field':'phones', 'data':self._gen_phones(num=3)},
            QuotaMaterial._MatCodeOptions.MAX_NUM_GEO_LOCATIONS.value: \
                    {'maxnum': 1, 'data_item':req_data[2], 'field':'locations', 'data':self._gen_locations(num=2)},
        }
        for mat_dataitem in self.usermgt_material_data:
            info = _info_map.get(mat_dataitem['mat_code'])
            data_item = info['data_item']
            quota_data = {'expiry':gen_expiry_time(), 'material': mat_dataitem['id'],
                    'maxnum': info['maxnum']}
            data_item['quota'].append(quota_data)
            data_item[info['field']].clear()
            data_item[info['field']].extend(info['data'])
        serializer = self.serializer_class(many=True, data=req_data, account=self._default_login_profile.account)
        error_caught = None
        with self.assertRaises(DRFValidationError):
            try:
                serializer.is_valid(raise_exception=True)
            except DRFValidationError as e:
                error_caught = e
                raise
        self.assertIsNotNone(error_caught)
        err_info = error_caught.detail
        expect_errmsg_pattern = 'number of items provided exceeds the limit: %s'
        expect_errmsg = expect_errmsg_pattern % _info_map[QuotaMaterial._MatCodeOptions.MAX_NUM_EMAILS.value]['maxnum']
        actual_errmsg = str(err_info[0]['emails'][non_field_err_key][0])
        self.assertEqual(expect_errmsg, actual_errmsg)
        expect_errmsg = expect_errmsg_pattern % _info_map[QuotaMaterial._MatCodeOptions.MAX_NUM_PHONE_NUMBERS.value]['maxnum']
        actual_errmsg = str(err_info[1]['phones'][non_field_err_key][0])
        self.assertEqual(expect_errmsg, actual_errmsg)
        expect_errmsg = expect_errmsg_pattern % _info_map[QuotaMaterial._MatCodeOptions.MAX_NUM_GEO_LOCATIONS.value]['maxnum']
        actual_errmsg = str(err_info[2]['locations'][non_field_err_key][0])
        self.assertEqual(expect_errmsg, actual_errmsg)


    def test_quota_inherited_from_applied_groups(self):
        self.assertEqual(self.num_quota, self.num_groups)
        req_data = self.request_data
        # configure quota arrangements to the applying groups in advance, by moving the same
        # quota arrangments from the request data 
        quota_arrangements = req_data[0]['quota']
        req_data[0]['quota'] = []
        quota_arrangements_iter = iter(quota_arrangements)
        for grp_data in req_data[0]['groups']:
            quota_data = next(quota_arrangements_iter)
            filtered = filter(lambda obj:obj.id == quota_data['material'], self._primitives[QuotaMaterial])
            quota_mat_obj = next(filtered)
            quota_rel_data = {'material':quota_mat_obj, 'maxnum':quota_data['maxnum']}
            filtered = filter(lambda obj:obj.id == grp_data['group'], self._primitives[GenericUserGroup])
            grp_obj = next(filtered)
            grp_obj.quota.create(**quota_rel_data)
        serializer = self.serializer_class(many=True, data=req_data[:1], account=self._default_login_profile.account)
        validate_result = serializer.is_valid(raise_exception=True)
        self.assertTrue(validate_result)
        expect_final_quota = {item['material']:item['maxnum'] for item in quota_arrangements}
        actual_final_quota =  serializer.child._final_quota_list[0]
        self.assertDictEqual(expect_final_quota, actual_final_quota)
        validated_data = serializer.validated_data[0]
        self.assertFalse(any(validated_data['quota']))
## end of class ProfileCreationTestCase


class ProfileUpdateBaseTestCase(ProfileCommonTestCase):
    num_roles = 3
    num_groups = 3

    def _setup_new_profiles_req_data(self, num_profiles = 2, contact_quota_maxnum = 3):
        self.num_quota = 0
        profile_data_for_test = _fixtures[GenericUserProfile][self.num_default_profiles : ]
        profs_data_gen = listitem_rand_assigner(list_=profile_data_for_test,
                min_num_chosen=num_profiles, max_num_chosen=(num_profiles+1))
        request_data = rand_gen_request_body(customize_item_fn=self.customize_req_data_item, data_gen=profs_data_gen)
        request_data = list(request_data)
        other_apps_material_data = filter(lambda d:d['app_code'] != AppCodeOptions.user_management, _fixtures[QuotaMaterial])
        other_apps_material_data = next(other_apps_material_data)
        for req_data_item in request_data:
            quota_data = list(map(lambda d: {'expiry':gen_expiry_time(), 'material': d['id'], \
                    'maxnum':contact_quota_maxnum } , self.usermgt_material_data))
            quota_data.append({'expiry':gen_expiry_time(), 'maxnum':random.randrange(3,30), \
                    'material': other_apps_material_data['id'],})
            req_data_item['quota'].extend(quota_data)
            req_data_item['emails'].extend(self._gen_emails(num=contact_quota_maxnum))
            req_data_item['phones'].extend(self._gen_phones(num=contact_quota_maxnum))
            req_data_item['locations'].extend(self._gen_locations(num=contact_quota_maxnum))
        self.num_quota = contact_quota_maxnum
        return request_data

    def setUp(self):
        super().setUp()
        request_data = self._setup_new_profiles_req_data()
        serializer = self.serializer_class(many=True, data=request_data, account=self._default_login_profile.account)
        serializer.is_valid(raise_exception=True)
        self.created_profiles = serializer.save()
        self.request_data = self._load_profiles_from_instances(objs=self.created_profiles)


class ProfileUpdateTestCase(ProfileUpdateBaseTestCase):
    def test_bulk_ok(self):
        for req_data_item in self.request_data:
            req_data_item['first_name'] = ''.join(random.choices(string.ascii_letters, k=6))
            # --- role ---
            applied_roles = tuple(map(lambda d:d['role'], req_data_item['roles']))
            available_roles = filter(lambda role: role.id not in applied_roles, self._primitives[Role])
            new_role = next(available_roles)
            new_data = {'expiry': gen_expiry_time(), 'role':new_role.id}
            req_data_item['roles'][0]['expiry'] = gen_expiry_time()
            evicted = req_data_item['roles'].pop()
            req_data_item['roles'].append(new_data)
            # --- group ---
            applied_grps = tuple(map(lambda d:d['group'], req_data_item['groups']))
            available_grps = filter(lambda obj: obj.id not in applied_grps, self._primitives[GenericUserGroup])
            new_grp = next(available_grps)
            req_data_item['groups'][-1]['group'] = new_grp.id
            # --- quota ---
            applied_quota_mats = tuple(map(lambda d:d['material'], req_data_item['quota']))
            available_quota_mats = filter(lambda material: material.id not in applied_quota_mats, self._primitives[QuotaMaterial])
            new_quo_mat = next(available_quota_mats)
            new_data = {'expiry':gen_expiry_time(), 'material': new_quo_mat.id, 'maxnum':random.randrange(2,19)}
            req_data_item['quota'][0]['expiry'] = gen_expiry_time()
            req_data_item['quota'][0]['maxnum'] = random.randrange(3,19)
            evicted =  req_data_item['quota'].pop()
            req_data_item['quota'].append(new_data)
            # --- emails ---
            req_data_item['emails'][0]['addr'] = '%s@t0ward.c10k' % ''.join(random.choices(string.ascii_letters, k=8))
            evicted =  req_data_item['emails'].pop()
            req_data_item['emails'].extend(self._gen_emails(num=1))
            # --- phones ---
            req_data_item['phones'][0]['line_number'] = str(random.randrange(0x10000000, 0xffffffff))
            evicted =  req_data_item['phones'].pop()
            req_data_item['phones'].extend(self._gen_phones(num=1))
            # --- locations ---
            req_data_item['locations'][0]['detail'] = ''.join(random.choices(string.ascii_letters, k=12))
            evicted = req_data_item['locations'].pop()
            req_data_item['locations'].extend(self._gen_locations(num=1))
        prof_ids = list(map(lambda obj:obj.id, self.created_profiles))
        qset = GenericUserProfile.objects.filter(id__in=prof_ids)
        # another loggedin user with higher privilege edits the profiles
        serializer = self.serializer_class(many=True, data=self.request_data, instance=qset,
                account=self._default_login_profile.account)
        serializer.is_valid(raise_exception=True)
        edited_profiles = serializer.save()
        self.verify_data(actual_data=edited_profiles, expect_data=self.request_data)


    def test_user_edits_her_own_profile(self):
        login_profile_2 = self.created_profiles[0]
        _setup_login_account(account_data=_fixtures[LoginAccount][1], profile_obj=login_profile_2)
        su_role = Role.objects.create(id=GenericUserProfile.SUPERUSER , name='super_user_role')
        req_data = self.request_data
        req_data[0]['first_name'] = 'Jose'
        # --- emails ---
        req_data[0]['emails'][0]['addr'] = '%s@t0ward.c10k' % ''.join(random.choices(string.ascii_letters, k=8))
        evicted =  req_data[0]['emails'].pop()
        req_data[0]['emails'].extend(self._gen_emails(num=1))
        req_data = copy.deepcopy(req_data)
        # following modification is NOT allowed and will be ignored in serializer validation process
        malicious_role_data  = {'expiry': None, 'role':su_role.id}
        malicious_quota_data = {'expiry': None, 'maxnum':9999,}
        req_data[0]['roles'][0].update(malicious_role_data )
        req_data[0]['quota'][0].update(malicious_quota_data)
        serializer = self.serializer_class(many=False, data=req_data[0], instance=login_profile_2,
                account=login_profile_2.account)
        serializer.is_valid(raise_exception=True)
        validated_data = serializer.validated_data
        with self.assertRaises(KeyError):
            validated_data['quota']
        with self.assertRaises(KeyError):
            validated_data['roles']
        with self.assertRaises(KeyError):
            validated_data['groups']
        edited_profile = serializer.save()
        self.verify_data(actual_data=[edited_profile], expect_data=self.request_data[:1])


    def test_bulk_ok_2(self):
        # user edits several profiles including her own profile
        req_data = self._load_profiles_from_instances(objs=[self._default_login_profile])
        req_data.extend(self.request_data)
        req_data[0]['emails'].extend(self._gen_emails(num=1))
        req_data[0]['first_name'] = 'Jimmy'
        req_data[1]['first_name'] = 'Haam'
        req_data[2]['first_name'] = 'Drex'
        req_data[1]['quota'][0]['maxnum'] = 345
        req_data[2]['quota'][0]['maxnum'] = 456
        req_data[1]['roles'][0]['expiry'] = gen_expiry_time(minutes_valid=20)
        req_data[2]['roles'][0]['expiry'] = gen_expiry_time(minutes_valid=21)
        req_data[1]['groups'].pop()
        req_data[2]['groups'].pop()
        self.request_data = req_data
        req_data = copy.deepcopy(req_data)
        # following modification is NOT allowed and will be ignored in serializer validation process
        quota_data = {'expiry':gen_expiry_time(), 'maxnum':30, 'material':-345 }
        req_data[0]['quota'].append(quota_data)
        role_data = {'expiry': gen_expiry_time(minutes_valid=144000), 'role': -123}
        req_data[0]['roles'].append(role_data)
        req_data[0]['groups'].append({'group':-234})
        # ----------------------------------
        prof_ids = list(map(lambda obj:obj.id, self.created_profiles))
        prof_ids.append(self._default_login_profile.id)
        qset = GenericUserProfile.objects.filter(id__in=prof_ids)
        serializer = self.serializer_class(many=True, data=req_data, instance=qset,
                account=self._default_login_profile.account)
        serializer.is_valid(raise_exception=True)
        validated_data = serializer.validated_data
        with self.assertRaises(KeyError):
            validated_data[0]['quota']
        with self.assertRaises(KeyError):
            validated_data[0]['roles']
        with self.assertRaises(KeyError):
            validated_data[0]['groups']
        edited_profiles = serializer.save()
        self.verify_data(actual_data=edited_profiles, expect_data=self.request_data)


    def test_edit_profile_without_groups(self):
        prof_ids = list(map(lambda obj:obj.id, self.created_profiles))
        qset = GenericUserProfile.objects.filter(id__in=prof_ids)
        self._test_edit_profile_without_groups(instance=qset)

    def test_duplicate_groups(self):
        prof_ids = list(map(lambda obj:obj.id, self.created_profiles))
        qset = GenericUserProfile.objects.filter(id__in=prof_ids)
        self._test_duplicate_groups(instance=qset)

    def test_exceeds_quota_limit(self):
        expect_new_limits = {'emails':3, 'phones':2, 'locations': 1}
        req_data = self.request_data[0]
        instance = self.created_profiles[0]
        for data in req_data['quota']:
            material = filter(lambda obj:obj.id == data['material'], self._primitives[QuotaMaterial])
            material = next(material)
            if material.mat_code == QuotaMaterial._MatCodeOptions.MAX_NUM_EMAILS.value:
                data['maxnum'] = expect_new_limits['emails']
            elif material.mat_code == QuotaMaterial._MatCodeOptions.MAX_NUM_PHONE_NUMBERS.value:
                data['maxnum'] = expect_new_limits['phones']
            elif material.mat_code == QuotaMaterial._MatCodeOptions.MAX_NUM_GEO_LOCATIONS.value:
                data['maxnum'] = expect_new_limits['locations']
        req_data['emails'].extend(self._gen_emails(num=1))
        req_data['locations'].pop()
        error_caught = None
        serializer = self.serializer_class(many=False, data=req_data, instance=instance,
                account=self._default_login_profile.account)
        with self.assertRaises(DRFValidationError):
            try:
                serializer.is_valid(raise_exception=True)
            except DRFValidationError as e:
                error_caught = e
                raise
        self.assertIsNotNone(error_caught)
        err_info = error_caught.detail
        expect_errmsg_pattern = 'number of items provided exceeds the limit: %s'
        for field_name, expect_limit in expect_new_limits.items():
            expect_value = expect_errmsg_pattern % expect_limit
            actual_value = error_caught.detail[field_name][non_field_err_key][0]
            self.assertEqual(expect_value, actual_value)
## end of class ProfileUpdateTestCase


class UpdateAccountPrivilegeTestCase(ProfileCommonTestCase):
    num_roles = 0
    num_quota = 0

    def _setup_default_user_roles(self, role_data):
        _other_login_profiles = self._primitives[GenericUserProfile][1:]
        tuple(map(lambda profile:profile.roles.create(**role_data[GenericUserProfile.NONE]), _other_login_profiles[0:5]))
        tuple(map(lambda profile:profile.roles.create(**role_data[GenericUserProfile.NONE]), _other_login_profiles[5:10]))
        tuple(map(lambda profile:profile.roles.create(**role_data[GenericUserProfile.STAFF]), _other_login_profiles[10:15]))
        tuple(map(lambda profile:profile.roles.create(**role_data[GenericUserProfile.STAFF]), _other_login_profiles[15:20]))
        tuple(map(lambda profile:profile.roles.create(**role_data[GenericUserProfile.SUPERUSER]), _other_login_profiles[20:25]))
        tuple(map(lambda profile:profile.roles.create(**role_data[GenericUserProfile.NONE]), _other_login_profiles[25:30]))
        accounts_data = iter(_fixtures[LoginAccount][1:])
        for profile in _other_login_profiles:
            account_data = next(accounts_data)
            _setup_login_account( account_data=account_data, profile_obj=profile )
        return _other_login_profiles

    def setUp(self):
        super().setUp()
        # gain superuser role to the default login user
        su_role = Role.objects.create(id=GenericUserProfile.SUPERUSER, name='mock superuser role')
        self._default_login_profile.roles.create(role=su_role, approved_by=self._default_login_profile)
        self._default_login_profile.account.is_superuser = True
        # data for role change
        self._role_data = {
            GenericUserProfile.SUPERUSER: {'role': su_role, 'approved_by': self._default_login_profile},
            GenericUserProfile.STAFF: {'role': self._primitives[Role][0], 'approved_by': self._default_login_profile},
            GenericUserProfile.NONE:  {'role': self._primitives[Role][1], 'approved_by': self._default_login_profile},
        }
        # generate other login users
        self.created_profiles = self._setup_default_user_roles(self._role_data)
        self.request_data = self._load_profiles_from_instances(objs=self.created_profiles)
        self.su_role = su_role

    def tearDown(self):
        super().tearDown()

    def test_change_by_higher_priv_user(self):
        profiles_guest2su    = self.created_profiles[0:5]
        profiles_guest2staff = self.created_profiles[5:10]
        profiles_staff2su    = self.created_profiles[10:15]
        profiles_staff2guest = self.created_profiles[15:20]
        profiles_su2staff    = self.created_profiles[20:25]
        profiles_guest_unchanged = self.created_profiles[25:30]
        self._perform_update(instance=self.created_profiles, data=self.request_data)
        for profile in tuple(profiles_guest2su) + tuple(profiles_guest2staff) + tuple(profiles_guest_unchanged):
            self.assertEqual(profile.privilege_status, GenericUserProfile.NONE)
            self.assertFalse(profile.account.is_superuser)
            self.assertFalse(profile.account.is_staff)
        for profile in tuple(profiles_su2staff):
            self.assertEqual(profile.privilege_status, GenericUserProfile.SUPERUSER)
            self.assertTrue(profile.account.is_superuser)
            self.assertTrue(profile.account.is_staff)
        for profile in tuple(profiles_staff2su) + tuple(profiles_staff2guest):
            self.assertEqual(profile.privilege_status, GenericUserProfile.STAFF)
            self.assertFalse(profile.account.is_superuser)
            self.assertTrue(profile.account.is_staff)
        self.request_data = self._load_profiles_from_instances(objs=self.created_profiles)
        def _inner_change_role(data_iter, new_role):
            for data in data_iter:
                data['roles'][0]['role'] = new_role.id
        _inner_change_role( new_role=self.su_role, data_iter=self.request_data[0:5] )
        _inner_change_role( new_role=self._primitives[Role][0], data_iter=self.request_data[5:10] )
        _inner_change_role( new_role=self.su_role, data_iter=self.request_data[10:15] )
        _inner_change_role( new_role=self._primitives[Role][1], data_iter=self.request_data[15:20] )
        _inner_change_role( new_role=self._primitives[Role][0], data_iter=self.request_data[20:25] )
        self._perform_update(instance=self.created_profiles, data=self.request_data)
        for profile in tuple(profiles_guest2su) + tuple(profiles_staff2su):
            self.assertEqual(profile.privilege_status, GenericUserProfile.SUPERUSER)
            self.assertTrue(profile.account.is_superuser)
            self.assertTrue(profile.account.is_staff)
        for profile in tuple(profiles_guest2staff) + tuple(profiles_su2staff):
            self.assertEqual(profile.privilege_status, GenericUserProfile.STAFF)
            self.assertFalse(profile.account.is_superuser)
            self.assertTrue(profile.account.is_staff)
        for profile in tuple(profiles_staff2guest) + tuple(profiles_guest_unchanged):
            self.assertEqual(profile.privilege_status, GenericUserProfile.NONE)
            self.assertFalse(profile.account.is_superuser)
            self.assertFalse(profile.account.is_staff)


    def _perform_update(self, instance, data):
        prof_ids = list(map(lambda obj:obj.id, instance))
        qset = GenericUserProfile.objects.filter(id__in=prof_ids)
        serializer = self.serializer_class(many=True, instance=qset, data=data,
                account=self._default_login_profile.account)
        serializer.is_valid(raise_exception=True)
        serializer.save()
        for profile in instance:
            profile.account.refresh_from_db()




class ProfileRepresentationTestCase(ProfileUpdateBaseTestCase):
    def test_full_representation(self):
        prof_ids = list(map(lambda obj:obj.id, self.created_profiles))
        qset = GenericUserProfile.objects.filter(id__in=prof_ids)
        serializer = self.serializer_class(many=True, instance=qset, account=self._default_login_profile.account)
        actual_data = serializer.data
        self.verify_data(actual_data=self.created_profiles , expect_data=actual_data)

    def test_partial_representation(self):
        hidden_fields = ('first_name', 'time_created', 'roles', 'emails', 'locations',)
        expect_fields = ('id', 'last_name', 'last_updated', 'auth', 'groups', 'quota', 'phones')
        mocked_request = Mock()
        mocked_request.query_params = {'fields':','.join(expect_fields)}
        prof_ids = list(map(lambda obj:obj.id, self.created_profiles))
        qset = GenericUserProfile.objects.filter(id__in=prof_ids)
        serializer = self.serializer_class(many=True, instance=qset, account=self._default_login_profile.account)
        serializer.context['request'] = mocked_request
        expect_data = self._load_profiles_from_instances(objs=self.created_profiles)
        actual_data = serializer.data
        expect_data_iter = iter(expect_data)
        for actual_d in actual_data:
            for field in hidden_fields:
                with self.assertRaises(KeyError):
                    actual_d[field]
            expect_d = next(expect_data_iter)
            is_equal = self._value_compare_groups_fn(val_a=actual_d, val_b=expect_d)
            self.assertTrue(is_equal)
            is_equal = self._value_compare_quota_fn(val_a=actual_d, val_b=expect_d)
            self.assertTrue(is_equal)
            is_equal = self._value_compare_contact_fn(val_a=actual_d['phones'], compare_id=True,
                    val_b=expect_d['phones'], _fields_compare=_nested_field_names['phones'])
            self.assertTrue(is_equal)
            expect_active_status = LoginAccountExistField.activation_status.ACCOUNT_NON_EXISTENT.value
            actual_active_status = actual_d['auth']
            self.assertEqual(expect_active_status, actual_active_status)


