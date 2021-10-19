import string
import random
import time
from unittest.mock import patch

from django.test import TransactionTestCase
from django.contrib.auth.models import Permission
from rest_framework.settings    import api_settings as drf_settings

from user_management.models.common import AppCodeOptions
from user_management.models.base import GenericUserProfile, QuotaMaterial
from user_management.models.auth import Role, LoginAccount
from user_management.serializers.nested import RoleAssignValidator
from user_management.async_tasks import update_accounts_privilege

from tests.python.common import TreeNodeMixin
from tests.python.common.django import _BaseMockTestClientInfoMixin

from ..common import _fixtures, client_req_csrf_setup, AuthenticateUserMixin, UserNestedFieldSetupMixin, gen_expiry_time
from  .common import _nested_field_names, HttpRequestDataGenGroup, GroupVerificationMixin


class BaseViewTestCase(TransactionTestCase, _BaseMockTestClientInfoMixin, AuthenticateUserMixin, HttpRequestDataGenGroup, GroupVerificationMixin):
    usermgt_material_data = tuple(filter(lambda d:d['app_code'] == AppCodeOptions.user_management, _fixtures[QuotaMaterial]))
    def setUp(self):
        self.init_primitive()
        self._profile = self._primitives[GenericUserProfile][0]
        self._profile_2nd = self._primitives[GenericUserProfile][1]
        self._setup_user_roles(profile=self._profile, approved_by=self._profile_2nd,
                roles=self._primitives[Role][:])
        self._auth_setup(testcase=self, profile=self._profile, is_superuser=False)

    def tearDown(self):
        self._client.cookies.clear()

    def _setup_user_roles(self, profile, approved_by, roles=None):
        roles = roles or []
        role_rel_data = {'expiry':gen_expiry_time(minutes_valid=10), 'approved_by': approved_by,}
        tuple(map(lambda role: profile.roles.create(role=role, **role_rel_data), roles))

    def _prepare_access_token(self, new_perms_info):
        qset = Permission.objects.filter(content_type__app_label='user_management',
                codename__in=new_perms_info)
        self._primitives[Role][1].permissions.set(qset)
        acs_tok_resp = self._refresh_access_token(testcase=self, audience=['user_management'])
        return acs_tok_resp['access_token']


class GroupCreationTestCase(BaseViewTestCase):
    path = '/groups'
    num_roles = 2
    num_quota = 3

    def setUp(self):
        super().setUp()
        self.api_call_kwargs = client_req_csrf_setup()
        self.api_call_kwargs.update({'path': self.path, 'method':'post'})


    def test_no_permission(self):
        access_token = self._prepare_access_token(new_perms_info=['view_role', 'view_genericusergroup'])
        self.api_call_kwargs['headers']['HTTP_AUTHORIZATION'] = ' '.join(['Bearer', access_token])
        response = self._send_request_to_backend(**self.api_call_kwargs)
        self.assertEqual(int(response.status_code), 403)

    def test_new_trees_ok(self):
        access_token =  self._prepare_access_token(new_perms_info=['view_genericusergroup','add_genericusergroup'])
        self.api_call_kwargs['headers']['HTTP_AUTHORIZATION'] = ' '.join(['Bearer', access_token])
        # subcase #1, create new trees
        origin_trees = TreeNodeMixin.rand_gen_trees(
                num_trees=2, min_num_nodes=3, max_num_nodes=3, min_num_siblings=2,
                max_num_siblings=2, write_value_fn=self._write_value_fn)
        req_data = self.trees_to_req_data(trees=origin_trees)
        self.api_call_kwargs['body'] = req_data
        self.api_call_kwargs['expect_shown_fields'] = ['id','name']
        response = self._send_request_to_backend(**self.api_call_kwargs)
        self.assertEqual(int(response.status_code), 201)
        response_body = response.json()
        grp_ids = list(map(lambda data: data['id'], response_body))
        entity_data, closure_data = self.load_closure_data(node_ids=grp_ids)
        saved_trees = TreeNodeMixin.gen_from_closure_data(entity_data=entity_data, closure_data=closure_data,
                custom_value_setup_fn=self._closure_node_value_setup )
        matched, not_matched = TreeNodeMixin.compare_trees(trees_a=origin_trees, trees_b=saved_trees,
                value_compare_fn=self._value_compare_fn)
        self.assertListEqual(not_matched, [])
        self.assertEqual(len(matched), len(origin_trees))
        origin_trees = saved_trees
        # subcase #2, new trees appending to existing trees
        another_trees = TreeNodeMixin.rand_gen_trees(
                num_trees=2, min_num_nodes=3, max_num_nodes=3, min_num_siblings=2,
                max_num_siblings=2, write_value_fn=self._write_value_fn)
        exist_parent = origin_trees[0].children[0]
        for root in another_trees:
            root.value['exist_parent'] = exist_parent.value['id']
            root.parent = exist_parent
        req_data = self.trees_to_req_data(trees=another_trees)
        self.api_call_kwargs['body'] = req_data
        self.api_call_kwargs['expect_shown_fields'] = ['id','name']
        response = self._send_request_to_backend(**self.api_call_kwargs)
        self.assertEqual(int(response.status_code), 201)
        response_body = response.json()
        grp_ids.extend( list(map(lambda data: data['id'], response_body)) )
        entity_data, closure_data = self.load_closure_data(node_ids=grp_ids)
        saved_trees = TreeNodeMixin.gen_from_closure_data(entity_data=entity_data, closure_data=closure_data,
                custom_value_setup_fn=self._closure_node_value_setup )
        matched, not_matched = TreeNodeMixin.compare_trees(trees_a=origin_trees, trees_b=saved_trees,
                value_compare_fn=self._value_compare_fn)
        self.assertListEqual(not_matched, [])
        self.assertEqual(len(matched), len(origin_trees))
        applied_grp_ids = self._profile.groups.values_list('group', flat=True)
        self.assertSetEqual(set(grp_ids), set(applied_grp_ids))

    def test_non_support_roles(self):
        access_token =  self._prepare_access_token(new_perms_info=['view_genericusergroup','add_genericusergroup'])
        self.api_call_kwargs['headers']['HTTP_AUTHORIZATION'] = ' '.join(['Bearer', access_token])
        trees = TreeNodeMixin.rand_gen_trees(num_trees=1,  min_num_nodes=3, max_num_nodes=3,
                max_num_siblings=2, min_num_siblings=1, write_value_fn=self._write_value_fn)
        missing_role_id = trees[0].value['roles'][0]['role']
        self._profile.roles.filter(role__id=missing_role_id).delete(hard=True)
        req_data = self.trees_to_req_data(trees=trees)
        self.api_call_kwargs['body'] = req_data
        response = self._send_request_to_backend(**self.api_call_kwargs)
        self.assertEqual(int(response.status_code), 400)
        err_info = response.json()
        expect_errmsg = RoleAssignValidator.err_msg_pattern % missing_role_id
        actual_errmsg = err_info[0]['roles'][0]['role'][0]
        self.assertEqual(expect_errmsg, actual_errmsg)
## end of class GroupCreationTestCase


class GroupBaseUpdateTestCase(BaseViewTestCase):
    num_roles = 2
    num_quota = 2

    def setUp(self):
        super().setUp()
        self.saved_trees = self._init_new_trees()

    # override parent class function
    def _init_new_trees(self):
        origin_trees = TreeNodeMixin.rand_gen_trees(
                num_trees=3, min_num_nodes=5, max_num_nodes=5, min_num_siblings=2,
                max_num_siblings=2, write_value_fn=self._write_value_fn)
        contact_quota_maxnum = 3
        other_apps_material_data = filter(lambda d:d['app_code'] != AppCodeOptions.user_management, _fixtures[QuotaMaterial])
        other_apps_material_data = next(other_apps_material_data)
        for root in origin_trees:
            quota_data = list(map(lambda d: {'expiry':gen_expiry_time(), 'material': d['id'], \
                    'maxnum':contact_quota_maxnum } , self.usermgt_material_data))
            quota_data.append({'expiry':gen_expiry_time(), 'maxnum':random.randrange(3,30), \
                    'material': other_apps_material_data['id'],})
            root.value['quota'] = quota_data
            root.value['emails'] = self._gen_emails(num=contact_quota_maxnum)
            root.value['phones'] = self._gen_phones(num=contact_quota_maxnum)
            root.value['locations'] = self._gen_locations(num=contact_quota_maxnum)
        req_data = self.trees_to_req_data(trees=origin_trees)
        api_call_kwargs = client_req_csrf_setup()
        api_call_kwargs.update({'path': '/groups', 'method':'post', 'body':req_data,
            'expect_shown_fields':['id','name'],})
        access_token =  self._prepare_access_token(new_perms_info=['view_genericusergroup','add_genericusergroup'])
        api_call_kwargs['headers']['HTTP_AUTHORIZATION'] = ' '.join(['Bearer', access_token])
        response = self._send_request_to_backend(**api_call_kwargs)
        self.assertEqual(int(response.status_code), 201)
        response_body = response.json()
        grp_ids = list(map(lambda data: data['id'], response_body))
        entity_data, closure_data = self.load_closure_data(node_ids=grp_ids)
        saved_trees = TreeNodeMixin.gen_from_closure_data(entity_data=entity_data, closure_data=closure_data,
                custom_value_setup_fn=self._closure_node_value_setup )
        return saved_trees
## end of class GroupBaseUpdateTestCase


class GroupUpdateTestCase(GroupBaseUpdateTestCase):
    path = '/groups'

    def setUp(self):
        super().setUp()
        self.api_call_kwargs = client_req_csrf_setup()
        self.api_call_kwargs.update({'path': self.path, 'method':'put'})

    def test_no_permission(self):
        # subcase #1, the user does not have sufficient roles
        access_token = self._prepare_access_token(new_perms_info=['view_role', 'view_genericusergroup'])
        self.api_call_kwargs['headers']['HTTP_AUTHORIZATION'] = ' '.join(['Bearer', access_token])
        response = self._send_request_to_backend(**self.api_call_kwargs)
        self.assertEqual(int(response.status_code), 403)
        # subcase #2, user has sufficient roles but doesn't have access to specific group
        access_token = self._prepare_access_token(new_perms_info=['view_genericusergroup','change_genericusergroup'])
        self.api_call_kwargs['headers']['HTTP_AUTHORIZATION'] = ' '.join(['Bearer', access_token])
        # assume the user is missing the access control to the first group
        self._profile.groups.filter(group__id=self.saved_trees[0].value['id']).delete(hard=True)
        req_data = self.trees_to_req_data(trees=self.saved_trees[:1])
        self.api_call_kwargs.update({'body': req_data,})
        response = self._send_request_to_backend(**self.api_call_kwargs)
        self.assertEqual(int(response.status_code), 403)

    def test_bulk_ok(self):
        for root in self.saved_trees:
            new_grp_name = 'my group %s' % ''.join(random.choices(string.ascii_letters, k=8))
            root.value['name'] = new_grp_name
            # --- role ---
            applied_roles = tuple(map(lambda d:d['role'], root.value['roles']))
            available_roles = filter(lambda role: role.id not in applied_roles, self._primitives[Role])
            new_role = next(available_roles)
            new_data = {'expiry': gen_expiry_time(), 'role':new_role.id}
            root.value['roles'][0]['expiry'] = gen_expiry_time()
            evicted = root.value['roles'].pop()
            root.value['roles'].append(new_data)
            # --- quota ---
            applied_quota_mats = tuple(map(lambda d:d['material'], root.value['quota']))
            available_quota_mats = filter(lambda material: material.id not in applied_quota_mats, self._primitives[QuotaMaterial])
            new_quo_mat = next(available_quota_mats)
            new_data = {'expiry':gen_expiry_time(), 'material': new_quo_mat.id, 'maxnum':random.randrange(2,19)}
            root.value['quota'][0]['expiry'] = gen_expiry_time()
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
        new_parent_node = self.saved_trees[0].children[-1]
        self.saved_trees[1].parent = new_parent_node
        self.saved_trees[2].parent = new_parent_node
        moving_nodes = self.saved_trees.copy()
        req_data = self._moving_nodes_to_req_data(moving_nodes)
        self.api_call_kwargs.update({'body': req_data,})
        access_token = self._prepare_access_token(new_perms_info=['view_genericusergroup','change_genericusergroup'])
        self.api_call_kwargs['headers']['HTTP_AUTHORIZATION'] = ' '.join(['Bearer', access_token])
        with patch('user_management.async_tasks.update_accounts_privilege.apply_async') as mocked_async_task:
            response = self._send_request_to_backend(**self.api_call_kwargs)
            self.assertEqual(mocked_async_task.call_count , 1)
        self.assertEqual(int(response.status_code), 200)
        grp_ids = self.saved_trees.entity_data.values_list('id', flat=True)
        entity_data, closure_data = self.load_closure_data(node_ids=grp_ids)
        trees_before_edit = self.saved_trees[:1]
        trees_after_edit = TreeNodeMixin.gen_from_closure_data(entity_data=entity_data, closure_data=closure_data,
                custom_value_setup_fn=self._closure_node_value_setup )
        matched, not_matched = TreeNodeMixin.compare_trees(trees_a=trees_before_edit, trees_b=trees_after_edit,
                value_compare_fn=self._value_compare_fn)
        self.assertListEqual(not_matched, [])
        self.assertEqual(len(matched), len(trees_before_edit))
## end of class GroupUpdateTestCase


class GroupDeletionTestCase(GroupBaseUpdateTestCase):
    path = '/groups'

    def _gen_roles(self, num=None):
        # avoid to add staff role when creating groups
        return UserNestedFieldSetupMixin._gen_roles(self, num=num, role_objs=self._primitives[Role][1:])

    def setUp(self):
        update_accounts_privilege.app.conf.task_always_eager = True
        super().setUp()
        self.deleting_nodes = [
            self.saved_trees[0] ,
            self.saved_trees[1].children[0] ,
            self.saved_trees[2].children[0].children[0] ,
        ]
        grp_ids = list(map(lambda node: node.value['id'], self.deleting_nodes))
        body = list(map(lambda gid:{'id':gid}, grp_ids))
        self.api_call_kwargs = client_req_csrf_setup()
        self.api_call_kwargs.update({'path': self.path, 'method':'delete', 'body': body})

    def tearDown(self):
        super().tearDown()
        update_accounts_privilege.app.conf.task_always_eager = False


    def test_no_permission(self):
        # assume the user is missing the access control to a few deleting groups
        missing_grp_ids = [
            self.saved_trees[2].value['id'],
            self.saved_trees[2].children[0].value['id'],
            self.saved_trees[2].children[0].children[0].value['id'],
        ]
        self._profile.groups.filter(group__id__in=missing_grp_ids).delete(hard=True)
        # ----------------------------------
        access_token = self._prepare_access_token(new_perms_info=['view_genericusergroup','delete_genericusergroup'])
        self.api_call_kwargs['headers']['HTTP_AUTHORIZATION'] = ' '.join(['Bearer', access_token])
        response = self._send_request_to_backend(**self.api_call_kwargs)
        self.assertEqual(int(response.status_code), 403)


    def test_bulk_ok(self):
        # assume staff role was added to these deleting groups
        grp_ids = list(map(lambda node: node.value['id'], self.deleting_nodes))
        deleting_groups = self.saved_trees.entity_data.filter(id__in=grp_ids)
        profiles_iter = iter(self._primitives[GenericUserProfile][2:])
        accounts_data_iter = iter(_fixtures[LoginAccount][2:])
        accounts_watchlist = []
        for del_grp in deleting_groups:
            data = {'role':self._primitives[Role][0], 'approved_by':self._profile_2nd, 'expiry': gen_expiry_time()}
            del_grp.roles.create(**data)
            profile = next(profiles_iter)
            data = {'profile': profile, 'approved_by':self._profile_2nd}
            del_grp.profiles.create(**data)
            account_data = next(accounts_data_iter)
            account_data.update({'is_staff':True, 'profile':profile})
            account = LoginAccount.objects.create_user(**account_data)
            self.assertTrue(account.is_staff)
            accounts_watchlist.append(account)
        # ----------------------------------
        access_token = self._prepare_access_token(new_perms_info=['view_genericusergroup','delete_genericusergroup'])
        self.api_call_kwargs['headers']['HTTP_AUTHORIZATION'] = ' '.join(['Bearer', access_token])
        response = self._send_request_to_backend(**self.api_call_kwargs)
        self.assertEqual(int(response.status_code), 202)
        for del_grp in deleting_groups:
            field_names = ('roles', 'quota', 'profiles')
            for field_name in field_names:
                related_manager = getattr(del_grp, field_name)
                self.assertFalse(related_manager.all().exists())
                actual_count = related_manager.all(with_deleted=True).count()
                self.assertGreater(actual_count, 0)
            field_names = ('phones', 'locations',)
            for field_name in field_names:
                related_manager = getattr(del_grp, field_name)
                self.assertFalse(related_manager.all().exists())
        for account in accounts_watchlist:
            account.refresh_from_db()
            self.assertFalse(account.is_staff)


    def _value_compare_fn(self, val_a, val_b):
        fields_eq = {}
        fields_eq['id'] = val_a['id'] == val_b['id']
        fields_eq['name'] = val_a['name'] == val_b['name']
        fields_eq['roles']  = self._value_compare_roles_fn(val_a=val_a, val_b=val_b)
        fields_eq['quota']  = self._value_compare_quota_fn(val_a=val_a, val_b=val_b)
        fields_eq['emails'] = self._value_compare_contact_fn(val_a=val_a['emails'],
                val_b=val_b['emails'],  _fields_compare=self._nested_field_names['emails'])
        return fields_eq


    def _test_softdelete_nodes_sequence(self):
        access_token = self._prepare_access_token(new_perms_info=['view_genericusergroup',
            'change_genericusergroup', 'delete_genericusergroup'])
        self.api_call_kwargs['headers']['HTTP_AUTHORIZATION'] = ' '.join(['Bearer', access_token])
        deleting_node_sets = [
            ( # ---------- 1st delete request ---------
                self.saved_trees[0],
                self.saved_trees[1].children[0],
                self.saved_trees[2].children[0].children[0]
            ),
            ( # ---------- 2nd delete request ---------
                self.saved_trees[0].children[1],
                self.saved_trees[1].children[0].children[0],
            ),
            ( # ---------- 3rd delete request ---------
                self.saved_trees[0].children[0].children[1],
                self.saved_trees[2]
            ),
        ]
        response = None
        delay_interval_sec = 2
        for deleting_nodes in deleting_node_sets:
            grp_ids = list(map(lambda node: node.value['id'], deleting_nodes))
            body = list(map(lambda gid:{'id':gid}, grp_ids))
            self.api_call_kwargs.update({'method':'delete', 'body': body})
            response = self._send_request_to_backend(**self.api_call_kwargs)
            self.assertEqual(int(response.status_code), 202)
            time.sleep(delay_interval_sec)
        return deleting_node_sets


    def test_undelete_hierarchy_full_recovery(self):
        deleting_node_sets = self._test_softdelete_nodes_sequence()
        # the only way to full recovery of group hierarchy is to un-delete by time
        deleting_node_sets.reverse()
        undelete_node_sets_iter = iter(deleting_node_sets)
        self.api_call_kwargs.pop('body', None)
        self.api_call_kwargs.update({'method':'patch', 'expect_shown_fields':['id','name','phones','locations']})
        while True:
            response = self._send_request_to_backend(**self.api_call_kwargs)
            self.assertIn(int(response.status_code), (200, 410))
            if int(response.status_code) == 410:
                break
            result = response.json()
            actual_grp_ids = set(map(lambda d:d['id'], result['affected_items']))
            undelete_nodes = next(undelete_node_sets_iter)
            expect_grp_ids = set(map(lambda node:node.value['id'], undelete_nodes))
            self.assertSetEqual(actual_grp_ids, expect_grp_ids)
            for affected_item in result['affected_items']:
                self.assertFalse(any(affected_item['phones']))
                self.assertFalse(any(affected_item['locations']))
        trees_before_delete = self.saved_trees
        trees_after_delete = TreeNodeMixin.gen_from_closure_data(entity_data=self.saved_trees.entity_data,
                closure_data=self.saved_trees.closure_data, custom_value_setup_fn=self._closure_node_value_setup )
        matched, not_matched = TreeNodeMixin.compare_trees(trees_a=trees_before_delete, trees_b=trees_after_delete,
                value_compare_fn=self._value_compare_fn)
        self.assertListEqual(not_matched, [])
        self.assertEqual(len(matched), len(trees_before_delete))


    def test_undelete_hierarchy_eviction(self):
        deleting_node_sets = self._test_softdelete_nodes_sequence()
        self.api_call_kwargs.update({'method':'patch',  'expect_shown_fields':['id','name',]})
        for deleting_nodes in deleting_node_sets:
            grp_ids = list(map(lambda node: node.value['id'], deleting_nodes))
            self.api_call_kwargs['body'] = {'ids': grp_ids}
            response = self._send_request_to_backend(**self.api_call_kwargs)
            self.assertEqual(int(response.status_code), 200)
            result = response.json()
            expect_grp_ids = set(grp_ids)
            actual_grp_ids = set(map(lambda d:d['id'], result['affected_items']))
            self.assertSetEqual(actual_grp_ids, expect_grp_ids)
        trees_before_delete = self.saved_trees
        ##trees_after_delete = TreeNodeMixin.gen_from_closure_data(entity_data=self.saved_trees.entity_data,
        ##        closure_data=self.saved_trees.closure_data, custom_value_setup_fn=self._closure_node_value_setup )
        grp_cls = self.saved_trees.entity_data.model
        all_grp_ids = grp_cls.objects.values_list('id', flat=True)
        new_entity_data, new_closure_data =  self.load_closure_data(node_ids=all_grp_ids)
        trees_after_delete = TreeNodeMixin.gen_from_closure_data(entity_data=new_entity_data,
                closure_data=new_closure_data, custom_value_setup_fn=self._closure_node_value_setup )
        actual_root_grps = list(map(lambda node:node.value['id'], trees_after_delete))
        expect_new_root_grps = [
                # ---------- 1st delete request ---------
                self.saved_trees[0],
                self.saved_trees[1].children[0],
                self.saved_trees[2].children[0].children[0],
                # ---------- 2nd delete request ---------
                self.saved_trees[0].children[1],
                #--------split from origin tree sets ------
                self.saved_trees[1],
                self.saved_trees[2],
                self.saved_trees[0].children[0],
            ]
        expect_new_root_grps = list(map(lambda node:node.value['id'], expect_new_root_grps))
        uncovered_root_grps = set(expect_new_root_grps) - set(actual_root_grps)
        self.assertFalse(any(uncovered_root_grps))
        origin_tree_roots = expect_new_root_grps[4:]
        for root_grp_id in origin_tree_roots:
            filtered = filter(lambda node:node.value['id'] == root_grp_id, trees_after_delete)
            root_node = next(filtered)
            self.assertTrue(any(root_node.children))
## end of class GroupDeletionTestCase

class GroupQueryTestCase(GroupBaseUpdateTestCase):
    path = '/groups'
    def test_single_group(self):
        pass

    def test_multiple_groups(self):
        pass

    def test_ancestors(self):
        pass

    def test_descendants(self):
        pass

    def test_group_privilege(self):
        pass

