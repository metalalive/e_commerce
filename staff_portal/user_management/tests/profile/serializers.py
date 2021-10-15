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
from user_management.models.base import QuotaMaterial, GenericUserProfile, GenericUserGroup, GenericUserGroupClosure
from user_management.serializers.nested import GroupAssignValidator

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
        account_data =  _fixtures[LoginAccount][0]
        self._default_login_profile = _setup_login_account(account_data=account_data,
                profile_obj=self._primitives[GenericUserProfile][0] , roles=roles_without_superuser )
        self.assertEqual(self._default_login_profile.privilege_status , GenericUserProfile.STAFF)
        top_grps = (self._grp_map[3], self._grp_map[8], self._grp_map[11])
        self._refresh_applied_groups(profile=self._default_login_profile, groups=top_grps)

    def tearDown(self):
        pass

    def _refresh_applied_groups(self, profile, groups):
        approved_by = self._primitives[GenericUserProfile][1]
        profile.groups.all(with_deleted=True).delete(hard=True)
        for grp_obj in groups:
            profile.groups.create(group=grp_obj, approved_by=approved_by)
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
        self.verify_data(actual_data=actual_instances, expect_data=self.request_data,
                profile=self._default_login_profile)

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
        req_data = self.request_data
        dup_grp_id = req_data[0]['groups'][0]['group']
        req_data[0]['groups'][1]['group'] = dup_grp_id
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
        expect_errmsg  = err_info[0]['groups'][non_field_err_key][0]
        reason_pattern = 'duplicate item found in the list'
        self.assertGreater(expect_errmsg.find(reason_pattern), 0)


    def test_create_new_profile_without_groups(self):
        req_data = self.request_data
        req_data[0]['groups'].clear()
        # subcase #1: if current logged-in user is NOT superuser
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
        serializer = self.serializer_class(many=True, data=req_data, account=self._default_login_profile.account)
        validate_result = serializer.is_valid(raise_exception=True)
        self.assertTrue(validate_result)


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



class ProfileUpdateTestCase(ProfileCommonTestCase):
    def test_bulk_ok(self):
        pass

    def test_user_edits_her_own_profile(self):
        pass

    def test_higher_priv_user_edits_other_profiles(self):
        pass

    def test_invalid_role_quota_expiry(self):
        pass

    def test_duplicate_nested_field_id(self):
        pass

    def test_exceeds_quota_limit(self):
        pass


class ProfileRepresentationTestCase(ProfileCommonTestCase):
    def test_full_representation(self):
        pass

    def test_partial_representation(self):
        pass


