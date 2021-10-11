import string
import random
import copy
import json
from functools import partial
from datetime import timedelta

from django.test import TransactionTestCase
from django.utils import timezone as django_timezone
from rest_framework.exceptions import ValidationError as DRFValidationError
from rest_framework.settings import DEFAULTS as drf_default_settings

from common.util.python import sort_nested_object
from tests.python.common import TreeNodeMixin
from user_management.models.common import AppCodeOptions
from user_management.models.base import GenericUserProfile, QuotaMaterial
from user_management.models.auth import LoginAccount, Role

from ..common import  _fixtures
from  .common import  HttpRequestDataGenGroup, GroupVerificationMixin, _nested_field_names

non_field_err_key = drf_default_settings['NON_FIELD_ERRORS_KEY']

class GroupCommonTestCase(TransactionTestCase, HttpRequestDataGenGroup, GroupVerificationMixin):
    usermgt_material_data = tuple(filter(lambda d:d['app_code'] == AppCodeOptions.user_management, _fixtures[QuotaMaterial]))
    num_roles = 2
    num_quota = 3

    def _setup_login_account(self, account_data, profile_obj):
        account_data = account_data.copy()
        login_user_profile = profile_obj
        account_data['profile'] = login_user_profile
        account_data['password_last_updated'] = django_timezone.now()
        LoginAccount.objects.create_user(**account_data)
        # assume that the logged-in user has access to assign all the roles to groups
        for role in self._primitives[Role]:
            data_kwargs = {'expiry': self._gen_expiry_time(), 'role':role, 'approved_by':login_user_profile,}
            login_user_profile.roles.create(**data_kwargs)
        login_user_profile.refresh_from_db()
        return login_user_profile

    def setUp(self):
        self.init_primitive()
        self._login_user_profile = self._setup_login_account(account_data=_fixtures[LoginAccount][0],
                profile_obj=self._primitives[GenericUserProfile][1] )

    def tearDown(self):
        pass
## end of class GroupCommonTestCase


class GroupCreationTestCase(GroupCommonTestCase):
    def _init_new_trees(self, num_trees=3, min_num_nodes=2, max_num_nodes=10, min_num_siblings=1,
            max_num_siblings=3, write_value_fn=None, value_compare_fn=None):
        write_value_fn = write_value_fn or self._write_value_fn
        value_compare_fn = value_compare_fn or self._value_compare_fn
        origin_trees = TreeNodeMixin.rand_gen_trees(
                num_trees=num_trees, min_num_nodes=min_num_nodes,
                max_num_nodes=max_num_nodes, min_num_siblings=min_num_siblings,
                max_num_siblings=max_num_siblings, write_value_fn=write_value_fn)
        req_data = self.trees_to_req_data(trees=origin_trees)
        account  = self._login_user_profile.account
        serializer = self.serializer_class(many=True, data=req_data, account=account)
        serializer.is_valid(raise_exception=True)
        validated_data = serializer.validated_data
        actual_instances = serializer.save()
        obj_ids = tuple(map(lambda obj: obj.pk, actual_instances))
        entity_data, closure_data = self.load_closure_data(node_ids=obj_ids)
        saved_trees = TreeNodeMixin.gen_from_closure_data(entity_data=entity_data, closure_data=closure_data,
                custom_value_setup_fn=self._closure_node_value_setup )
        matched, not_matched = TreeNodeMixin.compare_trees(trees_a=origin_trees, trees_b=saved_trees,
                value_compare_fn=value_compare_fn)
        self.assertListEqual(not_matched, [])
        self.assertEqual(len(matched), len(origin_trees))
        return saved_trees

    def test_create_new_trees(self):
        num_rounds = 3
        for _ in range(num_rounds):
            self._init_new_trees()

    def _append_new_trees_to_existing_nodes(self, existing_trees, appending_trees):
        appending_trees_iter = iter(appending_trees)
        for existing_tree in existing_trees:
            try:
                appending_tree = next(appending_trees_iter)
                exist_parent = existing_tree
                while any(exist_parent.children):
                    exist_parent = exist_parent.children[0]
                appending_tree.value['exist_parent'] = exist_parent.value['id']
                appending_tree.parent = exist_parent
            except StopIteration as e:
                break
        out = existing_trees.copy() # shallow copy should be enough
        out.extend([t for t in appending_trees_iter])
        return out

    def test_append_new_trees_to_existing_nodes(self):
        num_rounds = 5
        num_trees = 3
        existing_trees = self._init_new_trees(num_trees=num_trees, max_num_nodes=7)
        for _ in range(num_rounds):
            appending_trees = TreeNodeMixin.rand_gen_trees(num_trees=random.randrange(2, num_trees + 2),
                    min_num_nodes=7, max_num_nodes=10, write_value_fn=self._write_value_fn)
            trees_before_save = self._append_new_trees_to_existing_nodes(
                    existing_trees=existing_trees, appending_trees=appending_trees)
            req_data = self.trees_to_req_data(trees=appending_trees)
            serializer = self.serializer_class(many=True, data=req_data, account=self._login_user_profile.account)
            serializer.is_valid(raise_exception=True)
            actual_instances = serializer.save()
            obj_ids = tuple(map(lambda obj: obj.pk, actual_instances))
            obj_ids = obj_ids + tuple(existing_trees.entity_data.values_list('id', flat=True))
            entity_data, closure_data = self.load_closure_data(node_ids=obj_ids) # serializer.data
            trees_after_save = TreeNodeMixin.gen_from_closure_data(entity_data=entity_data, closure_data=closure_data,
                    custom_value_setup_fn=self._closure_node_value_setup )
            matched, not_matched = TreeNodeMixin.compare_trees(trees_a=trees_before_save, trees_b=trees_after_save,
                value_compare_fn=self._value_compare_fn)
            self.assertListEqual(not_matched, [])
            self.assertEqual(len(matched), len(trees_before_save))
            existing_trees = trees_after_save


    def test_loop_detection_rand_gen_trees(self):
        self.num_quota = 2
        appending_trees = TreeNodeMixin.rand_gen_trees(num_trees=1,  min_num_nodes=7, max_num_nodes=15,
                max_num_siblings=2, min_num_siblings=1, write_value_fn=self._write_value_fn)
        req_data = self.trees_to_req_data(trees=appending_trees)
        non_root_data = map(lambda idx: {'idx': idx, 'data':req_data[idx]}, range(len(req_data)))
        non_root_data = list(filter(lambda d: d['data']['new_parent'] is not None, non_root_data))
        idx = random.randrange(0, len(non_root_data))
        loop_data_start = non_root_data[idx]
        loop_data_end = req_data[0]
        origin_new_parent = loop_data_end['new_parent']
        loop_data_end['new_parent'] = loop_data_start['idx']
        error_caught = None
        serializer = self.serializer_class(many=True, data=req_data, account=self._login_user_profile.account)
        with self.assertRaises(DRFValidationError):
            try:
                serializer.is_valid(raise_exception=True)
            except DRFValidationError as e:
                error_caught = e
                raise
        self.assertIsNotNone(error_caught)
        err_info = error_caught.detail[non_field_err_key]
        pattern_pos = err_info[0].find(self.err_msg_loop_detected)
        self.assertGreater(pattern_pos, 0)
        ancestor_indexes = [loop_data_end['new_parent']]
        curr_ancestor = req_data[loop_data_end['new_parent']]
        while curr_ancestor is not loop_data_end:
            parent_idx = curr_ancestor['new_parent']
            ancestor_indexes.append(parent_idx)
            curr_ancestor = req_data[parent_idx]
        form_label_pattern = 'form #%s'
        for asc_idx in ancestor_indexes:
            pattern_pos = err_info[0].find(form_label_pattern % asc_idx)
            self.assertGreaterEqual(pattern_pos, 0)

    def test_non_support_roles(self):
        self.num_roles = 3
        self.num_quota = 0
        trees = TreeNodeMixin.rand_gen_trees(num_trees=1,  min_num_nodes=2, max_num_nodes=4,
                max_num_siblings=2, min_num_siblings=1, write_value_fn=self._write_value_fn)
        missing_role_id = trees[0].value['roles'][0]['role']
        self._login_user_profile.roles.filter(role__id=missing_role_id).delete(hard=True)
        req_data = self.trees_to_req_data(trees=trees)
        serializer = self.serializer_class(many=True, data=req_data, account=self._login_user_profile.account)
        error_caught = None
        with self.assertRaises(DRFValidationError):
            try:
                serializer.is_valid(raise_exception=True)
            except DRFValidationError as e:
                error_caught = e
                raise
        self.assertIsNotNone(error_caught)
        err_info = error_caught.detail
        expect_value = 'Role is NOT assigned to current login user: %s' % (missing_role_id)
        actual_value = err_info[0]['roles'][0]['role'][0]
        actual_value = str(actual_value)
        self.assertEqual(expect_value, actual_value)


    def test_duplicate_roles(self):
        self.num_roles = 3
        self.num_quota = 0
        trees = TreeNodeMixin.rand_gen_trees(num_trees=1,  min_num_nodes=2, max_num_nodes=3,
                max_num_siblings=2, min_num_siblings=1, write_value_fn=self._write_value_fn)
        duplicate_role = trees[0].value['roles'][1]
        trees[0].value['roles'].append(duplicate_role)
        req_data = self.trees_to_req_data(trees=trees)
        serializer = self.serializer_class(many=True, data=req_data, account=self._login_user_profile.account)
        error_caught = None
        with self.assertRaises(DRFValidationError):
            try:
                serializer.is_valid(raise_exception=True)
            except DRFValidationError as e:
                error_caught = e
                raise
        self.assertIsNotNone(error_caught)
        err_info = error_caught.detail
        expect_errmsg = str( err_info[0]['roles'][non_field_err_key][0] )
        reason_pattern = 'duplicate item found in the list'
        self.assertGreater(expect_errmsg.find(reason_pattern), 0)


    def test_nonexist_quota_materials(self):
        self.num_roles = 0
        self.num_quota = 3
        trees = TreeNodeMixin.rand_gen_trees(num_trees=1,  min_num_nodes=2, max_num_nodes=3,
                max_num_siblings=2, min_num_siblings=1, write_value_fn=self._write_value_fn)
        invalid_material_id = -123
        edit_quota_data = trees[0].value['quota'][0]
        edit_quota_data['material'] = invalid_material_id
        req_data = self.trees_to_req_data(trees=trees)
        serializer = self.serializer_class(many=True, data=req_data, account=self._login_user_profile.account)
        error_caught = None
        with self.assertRaises(DRFValidationError):
            try:
                serializer.is_valid(raise_exception=True)
            except DRFValidationError as e:
                error_caught = e
                raise
        self.assertIsNotNone(error_caught)
        err_info = error_caught.detail
        expect_errmsg = str( err_info[0]['quota'][0]['material'][0] )
        reason_pattern = 'object does not exist'
        self.assertGreater(expect_errmsg.find(reason_pattern), 0)


    def test_duplicate_quota_materials(self):
        self.num_roles = 0
        self.num_quota = 3
        trees = TreeNodeMixin.rand_gen_trees(num_trees=1,  min_num_nodes=2, max_num_nodes=3,
                max_num_siblings=2, min_num_siblings=1, write_value_fn=self._write_value_fn)
        duplicate_role = trees[0].value['quota'][1].copy()
        duplicate_role['maxnum'] = random.randrange(2, 50)
        trees[0].value['quota'].append(duplicate_role)
        req_data = self.trees_to_req_data(trees=trees)
        serializer = self.serializer_class(many=True, data=req_data, account=self._login_user_profile.account)
        error_caught = None
        with self.assertRaises(DRFValidationError):
            try:
                serializer.is_valid(raise_exception=True)
            except DRFValidationError as e:
                error_caught = e
                raise
        self.assertIsNotNone(error_caught)
        err_info = error_caught.detail
        expect_errmsg = str( err_info[0]['quota'][non_field_err_key][0] )
        reason_pattern = 'duplicate item found in the list'
        self.assertGreater(expect_errmsg.find(reason_pattern), 0)


    def test_exceeds_quota_limit(self):
        self.num_roles = 0
        self.num_quota = 0
        trees = TreeNodeMixin.rand_gen_trees(num_trees=3,  min_num_nodes=1, max_num_nodes=1,
                max_num_siblings=2, min_num_siblings=1, write_value_fn=self._write_value_fn)
        for root in trees:
            self.assertFalse(any(root.value['quota']))
            self.assertFalse(any(root.value['emails']))
            self.assertFalse(any(root.value['phones']))
            self.assertFalse(any(root.value['locations']))
        _info_map = {
                QuotaMaterial._MatCodeOptions.MAX_NUM_EMAILS.value: \
                        {'maxnum': 3, 'node':trees[0], 'field':'emails', 'data':self._gen_emails(num=4)},
                QuotaMaterial._MatCodeOptions.MAX_NUM_PHONE_NUMBERS.value: \
                        {'maxnum': 2, 'node':trees[1], 'field':'phones', 'data':self._gen_phones(num=3)},
                QuotaMaterial._MatCodeOptions.MAX_NUM_GEO_LOCATIONS.value: \
                        {'maxnum': 1, 'node':trees[2], 'field':'locations', 'data':self._gen_locations(num=2)},
        }
        for mat_dataitem in self.usermgt_material_data:
            item = _info_map.get(mat_dataitem['mat_code'])
            if not item:
                continue
            data = {'expiry':self._gen_expiry_time(), 'material': mat_dataitem['id'], 'maxnum':item['maxnum']}
            item['node'].value['quota'].append(data)
            item['node'].value[item['field']].extend(item['data'])
        req_data = self.trees_to_req_data(trees=trees)
        serializer = self.serializer_class(many=True, data=req_data, account=self._login_user_profile.account)
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
    ## end of test_exceeds_quota_limit


    def test_skip_id(self):
        self.num_quota = 0
        contact_quota_maxnum = 2
        invalid_id_nested_field = -234
        invalid_id = -235
        treenode = TreeNodeMixin()
        self._write_value_fn(treenode)
        _info_map = {
            QuotaMaterial._MatCodeOptions.MAX_NUM_EMAILS.value: {'field':'emails', 'data':self._gen_emails(num=contact_quota_maxnum)},
            QuotaMaterial._MatCodeOptions.MAX_NUM_PHONE_NUMBERS.value: {'field':'phones', 'data':self._gen_phones(num=contact_quota_maxnum)},
            QuotaMaterial._MatCodeOptions.MAX_NUM_GEO_LOCATIONS.value: {'field':'locations', 'data':self._gen_locations(num=contact_quota_maxnum)},
        }
        for mat_dataitem in self.usermgt_material_data:
            item = _info_map.get(mat_dataitem['mat_code'])
            if not item:
                continue
            data = {'expiry':self._gen_expiry_time(), 'material': mat_dataitem['id'], 'maxnum':contact_quota_maxnum}
            treenode.value['quota'].append(data)
            for nested_dataitem in item['data']:
                nested_dataitem['id'] = invalid_id_nested_field
            treenode.value[item['field']].extend(item['data'])
        treenode.value['id'] = invalid_id
        req_data = self.trees_to_req_data(trees=[treenode])
        serializer = self.serializer_class(many=True, data=req_data, account=self._login_user_profile.account)
        serializer.is_valid(raise_exception=True)
        actual_instances = serializer.save()
        group = actual_instances[0]
        self.assertNotEqual(group.id, invalid_id)
        for info_item in _info_map.values():
            related_field_name = info_item['field']
            manager = getattr(group, related_field_name)
            invalid_nested_id_exists = manager.filter(id=invalid_id_nested_field).exists()
            self.assertFalse(invalid_nested_id_exists)
## end of class GroupCreationTestCase


class GroupUpdateTestCase(GroupCommonTestCase):
    num_roles = 3
    num_quota = 3

    def _init_new_trees(self, num_trees=3, min_num_nodes=2, max_num_nodes=10, min_num_siblings=1,  max_num_siblings=3 ):
        origin_num_quota = self.num_quota
        self.num_quota = 0
        origin_trees = TreeNodeMixin.rand_gen_trees(
                num_trees=num_trees, min_num_nodes=min_num_nodes, max_num_nodes=max_num_nodes,
                min_num_siblings=min_num_siblings, max_num_siblings=max_num_siblings,
                write_value_fn=self._write_value_fn )
        self.num_quota = origin_num_quota
        contact_quota_maxnum = 3
        other_apps_material_data = filter(lambda d:d['app_code'] != AppCodeOptions.user_management, _fixtures[QuotaMaterial])
        other_apps_material_data = next(other_apps_material_data)
        for root in origin_trees:
            quota_data = list(map(lambda d: {'expiry':self._gen_expiry_time(), 'material': d['id'], \
                    'maxnum':contact_quota_maxnum } , self.usermgt_material_data))
            quota_data.append({'expiry':self._gen_expiry_time(), 'maxnum':random.randrange(3,30), \
                    'material': other_apps_material_data['id'],})
            root.value['quota'].extend(quota_data)
            root.value['emails'].extend(self._gen_emails(num=contact_quota_maxnum))
            root.value['phones'].extend(self._gen_phones(num=contact_quota_maxnum))
            root.value['locations'].extend(self._gen_locations(num=contact_quota_maxnum))
        req_data = self.trees_to_req_data(trees=origin_trees)
        serializer = self.serializer_class(many=True, data=req_data, account=self._login_user_profile.account)
        serializer.is_valid(raise_exception=True)
        validated_data = serializer.validated_data
        actual_instances = serializer.save()
        obj_ids = tuple(map(lambda obj: obj.pk, actual_instances))
        entity_data, closure_data = self.load_closure_data(node_ids=obj_ids)
        saved_trees = TreeNodeMixin.gen_from_closure_data(entity_data=entity_data, closure_data=closure_data,
                custom_value_setup_fn=self._closure_node_value_setup )
        return saved_trees


    def setUp(self):
        super().setUp()
        self.existing_trees = self._init_new_trees(num_trees=3, min_num_nodes=3, max_num_nodes=4,
                min_num_siblings=1,  max_num_siblings=2)


    def _moving_nodes_to_req_data(self, moving_nodes):
        field_names = tuple(_nested_field_names.keys()) + ('id', 'name',)
        req_data = []
        for node in moving_nodes:
            data = {fname: node.value[fname] for fname in field_names}
            data['exist_parent'] = node.parent.value['id'] if node.parent else None
            data['new_parent'] = None
            req_data.append(data)
        random.shuffle(req_data) # `id` field should be unique value in each data item
        return req_data

    def _perform_update(self, moving_nodes, account):
        req_data = self._moving_nodes_to_req_data(moving_nodes)
        grp_ids = list(map(lambda node:node.value['id'] ,moving_nodes))
        grp_objs = self.serializer_class.Meta.model.objects.filter(id__in=grp_ids)
        serializer = self.serializer_class(many=True, data=req_data, instance=grp_objs,
                account=account )
        serializer.is_valid(raise_exception=True)
        actual_instances = serializer.save()
        obj_ids = self.existing_trees.entity_data.values_list('id', flat=True)
        entity_data, closure_data = self.load_closure_data(node_ids=obj_ids)
        edited_trees = TreeNodeMixin.gen_from_closure_data(entity_data=entity_data, closure_data=closure_data,
                custom_value_setup_fn=self._closure_node_value_setup )
        return edited_trees


    def test_bulk_with_hierarchy_change(self):
        for root in self.existing_trees:
            new_grp_name = 'my group %s' % ''.join(random.choices(string.ascii_letters, k=8))
            root.value['name'] = new_grp_name
            # --- role ---
            applied_roles = tuple(map(lambda d:d['role'], root.value['roles']))
            available_roles = filter(lambda role: role.id not in applied_roles, self._primitives[Role])
            new_role = next(available_roles)
            new_data = {'expiry': self._gen_expiry_time(), 'role':new_role.id}
            root.value['roles'][0]['expiry'] = self._gen_expiry_time()
            evicted = root.value['roles'].pop()
            root.value['roles'].append(new_data)
            # --- quota ---
            applied_quota_mats = tuple(map(lambda d:d['material'], root.value['quota']))
            available_quota_mats = filter(lambda material: material.id not in applied_quota_mats, self._primitives[QuotaMaterial])
            new_quo_mat = next(available_quota_mats)
            new_data = {'expiry':self._gen_expiry_time(), 'material': new_quo_mat.id, 'maxnum':random.randrange(2,19)}
            root.value['quota'][0]['expiry'] = self._gen_expiry_time()
            root.value['quota'][0]['maxnum'] = random.randrange(3,19)
            evicted =  root.value['quota'].pop()
            root.value['quota'].append(new_data)
            # --- emails ---
            new_data = self._gen_emails(num=1)
            root.value['emails'][0]['addr'] = '%s@t0ward.c10k' % ''.join(random.choices(string.ascii_letters, k=8))
            evicted =  root.value['emails'].pop()
            root.value['emails'].extend(new_data)
            # --- phones ---
            new_data = self._gen_phones(num=1)
            root.value['phones'][0]['line_number'] = str(random.randrange(0x10000000, 0xffffffff))
            evicted =  root.value['phones'].pop()
            root.value['phones'].extend(new_data)
            # --- locations ---
            new_data = self._gen_locations(num=1)
            root.value['locations'][0]['detail'] = ''.join(random.choices(string.ascii_letters, k=12))
            evicted =  root.value['locations'].pop()
            root.value['locations'].extend(new_data)
        new_parent_node = self.existing_trees[0].children[-1]
        self.existing_trees[1].parent = new_parent_node
        self.existing_trees[2].parent = new_parent_node
        moving_nodes = self.existing_trees.copy()
        edited_tree  = self._perform_update(moving_nodes, account=self._login_user_profile.account)
        matched, not_matched = TreeNodeMixin.compare_trees(trees_a=[self.existing_trees[0]],
                trees_b=edited_tree,  value_compare_fn=self._value_compare_fn)
        self.assertListEqual(not_matched, [])
    ## end of test_bulk_with_hierarchy_change


    def test_another_user_edits_role(self):
        root_node = self.existing_trees[0]
        # -----------------------------------------------
        applied_roles = tuple(map(lambda d:d['role'], root_node.value['roles']))
        available_roles = filter(lambda role: role.id not in applied_roles, self._primitives[Role])
        new_role = next(available_roles)
        new_data = {'expiry': self._gen_expiry_time(), 'role':new_role.id}
        root_node.value['roles'][0]['expiry'] = self._gen_expiry_time()
        evicted = root_node.value['roles'].pop()
        root_node.value['roles'].append(new_data)
        # -----------------------------------------------
        another_login_profile = self._setup_login_account(account_data=_fixtures[LoginAccount][2],
                profile_obj=self._primitives[GenericUserProfile][2] )
        edited_tree  = self._perform_update([root_node], account=another_login_profile.account)
        expect_value = root_node.value
        actual_value = edited_tree[0].value
        compare_result = self._value_compare_roles_fn(expect_value, actual_value)
        self.assertTrue(compare_result)
        expect_value = list(map(lambda d:{'role': d['role'], 'approved_by':another_login_profile.id} , root_node.value['roles']))
        expect_value[1]['approved_by'] = self._login_user_profile.id
        group = edited_tree.entity_data.get(pk=root_node.value['id'])
        actual_value = list(group.roles.values('role', 'approved_by'))
        expect_value = sort_nested_object(expect_value)
        actual_value = sort_nested_object(actual_value)
        self.assertListEqual(expect_value, actual_value)


    def test_another_user_edits_quota(self):
        root_node = self.existing_trees[0]
        # -----------------------------------------------
        applied_quota_mats = tuple(map(lambda d:d['material'], root_node.value['quota']))
        available_quota_mats = filter(lambda material: material.id not in applied_quota_mats, self._primitives[QuotaMaterial])
        new_quo_mat = next(available_quota_mats)
        new_data = {'expiry':self._gen_expiry_time(), 'material': new_quo_mat.id, 'maxnum':random.randrange(2,19)}
        root_node.value['quota'][0]['expiry'] = self._gen_expiry_time()
        root_node.value['quota'][0]['maxnum'] = random.randrange(4,19)
        evicted =  root_node.value['quota'].pop()
        root_node.value['quota'].append(new_data)
        # -----------------------------------------------
        another_login_profile = self._setup_login_account(account_data=_fixtures[LoginAccount][2],
                profile_obj=self._primitives[GenericUserProfile][2] )
        edited_tree  = self._perform_update([root_node], account=another_login_profile.account)
        expect_value = root_node.value
        actual_value = edited_tree[0].value
        compare_result = self._value_compare_quota_fn(expect_value, actual_value)
        self.assertTrue(compare_result)


    def test_invalid_role_quota_expiry(self):
        root_node = self.existing_trees[0]
        invalid_expiry = django_timezone.now() - timedelta(minutes=10)
        invalid_expiry = invalid_expiry.isoformat()
        root_node.value['roles'][0]['expiry'] = invalid_expiry
        root_node.value['quota'][0]['expiry'] = invalid_expiry
        error_caught = None
        with self.assertRaises(DRFValidationError):
            try:
                edited_tree  = self._perform_update([root_node], account=self._login_user_profile.account)
            except DRFValidationError as e:
                error_caught = e
                raise
        self.assertIsNotNone(error_caught)
        err_info = error_caught.detail
        actual_errmsg = err_info[0]['roles'][0]['expiry'][0]
        self.assertEqual('min_value', actual_errmsg.code)
        actual_errmsg = err_info[0]['quota'][0]['expiry'][0]
        self.assertEqual('min_value', actual_errmsg.code)


    def test_duplicate_nested_field_id(self):
        root_node = self.existing_trees[0]
        dup_role_id = root_node.value['roles'][0]['role']
        root_node.value['roles'][1]['role'] = dup_role_id
        dup_quota_mat_id = root_node.value['quota'][0]['material']
        root_node.value['quota'][1]['material'] = dup_quota_mat_id
        error_caught = None
        with self.assertRaises(DRFValidationError):
            try:
                edited_tree  = self._perform_update([root_node], account=self._login_user_profile.account)
            except DRFValidationError as e:
                error_caught = e
                raise
        self.assertIsNotNone(error_caught)
        err_info = error_caught.detail
        expect_errmsg_pattern = 'duplicate item found in the list'
        pos = err_info[0]['roles'][non_field_err_key][0].find( expect_errmsg_pattern )
        self.assertGreater(pos, 0)
        pos = err_info[0]['quota'][non_field_err_key][0].find( expect_errmsg_pattern )
        self.assertGreater(pos, 0)


    def test_exceeds_quota_limit(self):
        expect_new_limits = {'emails':3, 'phones':2, 'locations': 1}
        root_node = self.existing_trees[0]
        for data in root_node.value['quota']:
            material = filter(lambda obj:obj.id == data['material'], self._primitives[QuotaMaterial])
            material = next(material)
            if material.mat_code == QuotaMaterial._MatCodeOptions.MAX_NUM_EMAILS.value:
                data['maxnum'] = expect_new_limits['emails']
            elif material.mat_code == QuotaMaterial._MatCodeOptions.MAX_NUM_PHONE_NUMBERS.value:
                data['maxnum'] = expect_new_limits['phones']
            elif material.mat_code == QuotaMaterial._MatCodeOptions.MAX_NUM_GEO_LOCATIONS.value:
                data['maxnum'] = expect_new_limits['locations']
        root_node.value['emails'].extend(self._gen_emails(num=1))
        root_node.value['locations'].pop()
        error_caught = None
        with self.assertRaises(DRFValidationError):
            try:
                edited_tree  = self._perform_update([root_node], account=self._login_user_profile.account)
            except DRFValidationError as e:
                error_caught = e
                raise
        self.assertIsNotNone(error_caught)
        err_info = error_caught.detail
        expect_errmsg_pattern = 'number of items provided exceeds the limit: %s'
        expect_value = expect_errmsg_pattern % expect_new_limits['emails']
        actual_value = error_caught.detail[0]['emails'][non_field_err_key][0]
        self.assertEqual(expect_value, actual_value)
        expect_value = expect_errmsg_pattern % expect_new_limits['phones']
        actual_value = error_caught.detail[0]['phones'][non_field_err_key][0]
        self.assertEqual(expect_value, actual_value)
        expect_value = expect_errmsg_pattern % expect_new_limits['locations']
        actual_value = error_caught.detail[0]['locations'][non_field_err_key][0]
        self.assertEqual(expect_value, actual_value)


    def test_tree_chains(self):
        curr_node = self.existing_trees[0]
        new_parent = curr_node.children[-1]
        curr_node = self.existing_trees[1]
        curr_node.parent = new_parent
        new_parent = curr_node.children[-1]
        curr_node = self.existing_trees[2]
        curr_node.parent = new_parent
        edited_tree  = self._perform_update( self.existing_trees[1:], account=self._login_user_profile.account )
        matched, not_matched = TreeNodeMixin.compare_trees(trees_a=[self.existing_trees[0]],
                trees_b=edited_tree,  value_compare_fn=self._value_compare_fn)
        self.assertListEqual(not_matched, [])


    def test_loop_detection_across_3_trees(self):
        curr_node = self.existing_trees[0]
        new_parent = curr_node.children[-1]
        curr_node = self.existing_trees[1]
        curr_node.parent = new_parent
        new_parent = curr_node.children[-1]
        curr_node = self.existing_trees[2]
        curr_node.parent = new_parent
        new_parent = curr_node.children[-1]
        curr_node = self.existing_trees[0]
        curr_node.parent = new_parent
        with self.assertRaises(DRFValidationError):
            try:
                edited_tree  = self._perform_update( self.existing_trees, account=self._login_user_profile.account )
            except DRFValidationError as e:
                error_caught = e
                raise
        self.assertIsNotNone(error_caught)
        err_info = error_caught.detail
        expect_errmsg_pattern = 'will form a loop, which is NOT allowed in closure table'
        pos = err_info[non_field_err_key][0].find(expect_errmsg_pattern)
        self.assertGreater(pos, 0)
## end of class GroupUpdateTestCase


