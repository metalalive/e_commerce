import random
import json

from django.test import TransactionTestCase
from django.db.models import Q
from django.utils  import timezone

from ecommerce_common.tests.common import TreeNodeMixin
from ecommerce_common.util import sort_nested_object

from user_management.models.common import AppCodeOptions
from user_management.models.base import GenericUserProfile, GenericUserGroup, GenericUserGroupClosure,  QuotaMaterial, EmailAddress, PhoneNumber, GeoLocation, UserQuotaRelation, GenericUserAppliedRole
from user_management.models.auth import Role

from ..common import _fixtures

_data = {
    GenericUserGroupClosure:[
        #
        #               3
        #          /        \
        #         4          5
        #      /    \      /   \
        #     6     7     8     9
        #    / \   / \   /
        #   10 11 12 13 14
        #
        {'id':1,  'depth':0, 'ancestor':GenericUserGroup(id=3 ), 'descendant':GenericUserGroup(id=3) },
        {'id':2,  'depth':0, 'ancestor':GenericUserGroup(id=4 ), 'descendant':GenericUserGroup(id=4 )},
        {'id':3,  'depth':0, 'ancestor':GenericUserGroup(id=5 ), 'descendant':GenericUserGroup(id=5 )},
        {'id':4,  'depth':0, 'ancestor':GenericUserGroup(id=6 ), 'descendant':GenericUserGroup(id=6 )},
        {'id':5,  'depth':0, 'ancestor':GenericUserGroup(id=7 ), 'descendant':GenericUserGroup(id=7 )},
        {'id':6,  'depth':0, 'ancestor':GenericUserGroup(id=8 ), 'descendant':GenericUserGroup(id=8 )},
        {'id':7,  'depth':0, 'ancestor':GenericUserGroup(id=9 ), 'descendant':GenericUserGroup(id=9 )},
        {'id':8,  'depth':0, 'ancestor':GenericUserGroup(id=10), 'descendant':GenericUserGroup(id=10)},
        {'id':9,  'depth':0, 'ancestor':GenericUserGroup(id=11), 'descendant':GenericUserGroup(id=11)},
        {'id':10, 'depth':0, 'ancestor':GenericUserGroup(id=12), 'descendant':GenericUserGroup(id=12)},
        {'id':11, 'depth':0, 'ancestor':GenericUserGroup(id=13), 'descendant':GenericUserGroup(id=13)},
        {'id':12, 'depth':0, 'ancestor':GenericUserGroup(id=14), 'descendant':GenericUserGroup(id=14)},
        {'id':13, 'depth':1, 'ancestor':GenericUserGroup(id=3 ), 'descendant':GenericUserGroup(id=4 )},
        {'id':14, 'depth':1, 'ancestor':GenericUserGroup(id=3 ), 'descendant':GenericUserGroup(id=5 )},
        {'id':15, 'depth':1, 'ancestor':GenericUserGroup(id=4 ), 'descendant':GenericUserGroup(id=6 )},
        {'id':16, 'depth':1, 'ancestor':GenericUserGroup(id=4 ), 'descendant':GenericUserGroup(id=7 )},
        {'id':17, 'depth':1, 'ancestor':GenericUserGroup(id=5 ), 'descendant':GenericUserGroup(id=8 )},
        {'id':18, 'depth':1, 'ancestor':GenericUserGroup(id=5 ), 'descendant':GenericUserGroup(id=9 )},
        {'id':19, 'depth':1, 'ancestor':GenericUserGroup(id=6 ), 'descendant':GenericUserGroup(id=10)},
        {'id':20, 'depth':1, 'ancestor':GenericUserGroup(id=6 ), 'descendant':GenericUserGroup(id=11)},
        {'id':21, 'depth':1, 'ancestor':GenericUserGroup(id=7 ), 'descendant':GenericUserGroup(id=12)},
        {'id':22, 'depth':1, 'ancestor':GenericUserGroup(id=7 ), 'descendant':GenericUserGroup(id=13)},
        {'id':23, 'depth':1, 'ancestor':GenericUserGroup(id=8 ), 'descendant':GenericUserGroup(id=14)},
        {'id':24, 'depth':2, 'ancestor':GenericUserGroup(id=3 ), 'descendant':GenericUserGroup(id=6 )},
        {'id':25, 'depth':2, 'ancestor':GenericUserGroup(id=3 ), 'descendant':GenericUserGroup(id=7 )},
        {'id':26, 'depth':2, 'ancestor':GenericUserGroup(id=3 ), 'descendant':GenericUserGroup(id=8 )},
        {'id':27, 'depth':2, 'ancestor':GenericUserGroup(id=3 ), 'descendant':GenericUserGroup(id=9 )},
        {'id':28, 'depth':2, 'ancestor':GenericUserGroup(id=4 ), 'descendant':GenericUserGroup(id=10)},
        {'id':29, 'depth':2, 'ancestor':GenericUserGroup(id=4 ), 'descendant':GenericUserGroup(id=11)},
        {'id':30, 'depth':2, 'ancestor':GenericUserGroup(id=4 ), 'descendant':GenericUserGroup(id=12)},
        {'id':31, 'depth':2, 'ancestor':GenericUserGroup(id=4 ), 'descendant':GenericUserGroup(id=13)},
        {'id':32, 'depth':2, 'ancestor':GenericUserGroup(id=5 ), 'descendant':GenericUserGroup(id=14)},
        {'id':33, 'depth':3, 'ancestor':GenericUserGroup(id=3 ), 'descendant':GenericUserGroup(id=10)},
        {'id':34, 'depth':3, 'ancestor':GenericUserGroup(id=3 ), 'descendant':GenericUserGroup(id=11)},
        {'id':35, 'depth':3, 'ancestor':GenericUserGroup(id=3 ), 'descendant':GenericUserGroup(id=12)},
        {'id':36, 'depth':3, 'ancestor':GenericUserGroup(id=3 ), 'descendant':GenericUserGroup(id=13)},
        {'id':37, 'depth':3, 'ancestor':GenericUserGroup(id=3 ), 'descendant':GenericUserGroup(id=14)},
    ],
} ## end of _fixtures

extra_data_keys = (Role, GenericUserGroup, QuotaMaterial, EmailAddress, PhoneNumber, GeoLocation)
for key in extra_data_keys:
    _data[key] =_fixtures[key]


class GroupDeletionTestCase(TransactionTestCase):
    def setUp(self):
        objs = {UserQuotaRelation:[], GenericUserAppliedRole:[]}
        for k_cls, data in _data.items():
            objs[k_cls] = list(map(lambda d: k_cls(**d), data))
        for cls in (Role, QuotaMaterial, GenericUserGroup):
            cls.objects.bulk_create(objs[cls])
        GenericUserGroupClosure.objects.bulk_create(objs[GenericUserGroupClosure])
        self._delete_grp_ids = [5,7]
        grps_map = {obj.id: obj for obj in objs[GenericUserGroup]}
        chosen_grp = grps_map[self._delete_grp_ids[0]]
        for email_obj in objs[EmailAddress]:
            chosen_grp.emails.add(email_obj, bulk=False)
        for phone_obj in objs[PhoneNumber]:
            chosen_grp.phones.add(phone_obj, bulk=False)
        for loc_obj in objs[GeoLocation]:
            chosen_grp.locations.add(loc_obj, bulk=False)
        for quota_mat in objs[QuotaMaterial]:
            d = {'expiry': timezone.now(), 'material':quota_mat, 'maxnum':random.randrange(1,25)}
            quota_rel = UserQuotaRelation(**d)
            objs[UserQuotaRelation].append(quota_rel)
        profile = GenericUserProfile.objects.create(id=4, first_name='Knowledge', last_name='Hoarder')
        for role in objs[Role]:
            d = {'expiry': timezone.now(), 'approved_by':profile, 'role':role}
            role_rel = GenericUserAppliedRole(**d)
            objs[GenericUserAppliedRole].append(role_rel)
        for role_rel in objs[GenericUserAppliedRole]:
            chosen_grp.roles.add(role_rel, bulk=False)
        for quota_rel in objs[UserQuotaRelation]:
            chosen_grp.quota.add(quota_rel, bulk=False)
        self._objs = objs
        self._chosen_grp = chosen_grp
        self._profile = profile

    def tearDown(self):
        pass

    def _get_affected_paths(self, delete_ids, data):
        out = {}
        for del_id  in delete_ids:
            fn_affected_acs = lambda item: item['depth'] > 0 and item['ancestor'] == del_id
            filtered = filter(fn_affected_acs, data)
            affected_descs = list(map(lambda d:d['descendant'], filtered))
            fn_affected_decs = lambda item: item['depth'] > 0 and item['descendant'] == del_id
            filtered = filter(fn_affected_decs, data)
            affected_ascs = list(map(lambda d:d['ancestor'], filtered))
            out[del_id] = {'asc':affected_ascs, 'desc':affected_descs}
        return out

    def _assert_hierarchy_change(self, nodes_before_delete, nodes_after_delete):
        path_map = {(item['ancestor'], item['descendant']):item for item in nodes_before_delete}
        affected_path_map = self._get_affected_paths(delete_ids=self._delete_grp_ids,
                data=nodes_before_delete)
        for del_id  in self._delete_grp_ids:
            keys = [(asc, desc) for asc in affected_path_map[del_id]['asc'] for desc \
                    in affected_path_map[del_id]['desc']]
            affected_paths = map(lambda k: path_map[k], keys)
            for path_val in affected_paths:
                self.assertGreater(path_val['depth'] , 0)
                path_val['depth'] -= 1

        fn_exclude_ascs  = lambda item: item['ancestor'] not in self._delete_grp_ids
        fn_exclude_descs = lambda item: item['descendant'] not in self._delete_grp_ids
        expect_value = filter(fn_exclude_ascs, nodes_before_delete)
        expect_value = list(filter(fn_exclude_descs, expect_value))
        expect_value = sort_nested_object(expect_value)
        actual_value = sort_nested_object(nodes_after_delete)
        self.assertEqual(len(actual_value), len(expect_value))
        self.assertListEqual(actual_value, expect_value)


    def test_bulk_hard_delete(self):
        expect_num_emails    = self._chosen_grp.emails.all(with_deleted=True).count()
        expect_num_phones    = self._chosen_grp.phones.all().count()
        expect_num_locations = self._chosen_grp.locations.all().count()
        expect_num_roles  = self._chosen_grp.roles.all(with_deleted=True).count()
        expect_num_quota  = self._chosen_grp.quota.all().count()
        expect_num_ancs   = self._chosen_grp.ancestors.all(with_deleted=True).count()
        expect_num_descs  = self._chosen_grp.descendants.all(with_deleted=True).count()
        self.assertEqual(expect_num_emails   , len(self._objs[EmailAddress]))
        self.assertEqual(expect_num_phones   , len(self._objs[PhoneNumber]))
        self.assertEqual(expect_num_locations, len(self._objs[GeoLocation]))
        self.assertEqual(expect_num_roles, len(self._objs[Role]))
        self.assertEqual(expect_num_quota, len(self._objs[QuotaMaterial]))
        self.assertGreater(expect_num_ancs , 0)
        self.assertGreater(expect_num_descs, 0)
        grp_hier_before_delete = GenericUserGroupClosure.objects.values('id','depth','ancestor','descendant')
        grp_hier_before_delete = list(grp_hier_before_delete)

        grps = GenericUserGroup.objects.filter(id__in=self._delete_grp_ids)
        grps.delete(hard=True)

        qset = GenericUserGroup.objects.filter(id__in=self._delete_grp_ids, with_deleted=True)
        self.assertFalse(qset.exists())
        closure_qset = GenericUserGroupClosure.objects.filter(ancestor__in=self._delete_grp_ids, with_deleted=True)
        self.assertFalse(closure_qset.exists())
        closure_qset = GenericUserGroupClosure.objects.filter(descendant__in=self._delete_grp_ids, with_deleted=True)
        self.assertFalse(closure_qset.exists())
        grp_hier_after_delete = GenericUserGroupClosure.objects.values('id','depth','ancestor','descendant')
        grp_hier_after_delete = list(grp_hier_after_delete)
        self._assert_hierarchy_change(grp_hier_before_delete, grp_hier_after_delete)

        expect_num_emails    = self._chosen_grp.emails.all(with_deleted=True).count()
        expect_num_phones    = self._chosen_grp.phones.all().count()
        expect_num_locations = self._chosen_grp.locations.all().count()
        expect_num_roles  = self._chosen_grp.roles.all(with_deleted=True).count()
        expect_num_quota  = self._chosen_grp.quota.all().count()
        expect_num_ancs   = self._chosen_grp.ancestors.all(with_deleted=True).count()
        expect_num_descs  = self._chosen_grp.descendants.all(with_deleted=True).count()
        self.assertEqual(expect_num_emails   , 0)
        self.assertEqual(expect_num_phones   , 0)
        self.assertEqual(expect_num_locations, 0)
        self.assertEqual(expect_num_roles, 0)
        self.assertEqual(expect_num_quota, 0)
        self.assertEqual(expect_num_ancs , 0)
        self.assertEqual(expect_num_descs, 0)

        remain_grps = filter(lambda obj: obj.pk not in self._delete_grp_ids, self._objs[GenericUserGroup])
        remain_grp_ids = tuple(map(lambda obj: obj.pk, remain_grps))
        actual_remain_grps = GenericUserGroup.objects.filter(id__in=remain_grp_ids)
        self.assertEqual(actual_remain_grps.count(), len(remain_grp_ids))
    ## end of test_bulk_hard_delete()


    def test_bulk_soft_delete(self):
        grp_hier_before_delete = GenericUserGroupClosure.objects.values('id','depth','ancestor','descendant')
        grp_hier_before_delete = list(grp_hier_before_delete)

        closure_qset = GenericUserGroupClosure.objects.filter(ancestor__in=self._delete_grp_ids)
        expect_softdelete_ascs_ids = list(closure_qset.values_list('id', flat=True))
        closure_qset = GenericUserGroupClosure.objects.filter(descendant__in=self._delete_grp_ids)
        expect_softdelete_descs_ids = list(closure_qset.values_list('id', flat=True))

        grps = GenericUserGroup.objects.filter(id__in=self._delete_grp_ids)
        grps.delete(profile_id=self._profile.id)
        # check soft-deleted closure nodes
        closure_qset = GenericUserGroupClosure.objects.filter(ancestor__in=self._delete_grp_ids)
        self.assertFalse(closure_qset.exists())
        closure_qset = GenericUserGroupClosure.objects.filter(descendant__in=self._delete_grp_ids)
        self.assertFalse(closure_qset.exists())
        closure_qset = GenericUserGroupClosure.objects.filter(ancestor__in=self._delete_grp_ids, with_deleted=True)
        actual_softdelete_ascs_ids = list(closure_qset.values_list('id', flat=True))
        closure_qset = GenericUserGroupClosure.objects.filter(descendant__in=self._delete_grp_ids, with_deleted=True)
        actual_softdelete_descs_ids = list(closure_qset.values_list('id', flat=True))
        self.assertSetEqual(set(expect_softdelete_ascs_ids), set(actual_softdelete_ascs_ids))
        self.assertSetEqual(set(expect_softdelete_descs_ids), set(actual_softdelete_descs_ids))
        # check group hierarchy after the soft-delete operation
        grp_hier_after_delete = GenericUserGroupClosure.objects.values('id','depth','ancestor','descendant')
        grp_hier_after_delete = list(grp_hier_after_delete)
        self._assert_hierarchy_change(grp_hier_before_delete, grp_hier_after_delete)
        # without specifying deleted set, all related fields should return empty content
        related_field_names = ('emails','phones','locations','roles','quota','ancestors','descendants')
        for field_name in related_field_names:
            related_field = getattr(self._chosen_grp, field_name)
            expect_num    = related_field.all().count()
            self.assertEqual(expect_num, 0)
        # check soft-deleted emails
        expect_softdelete_emails = _data[EmailAddress]
        actual_softdelete_emails = list(self._chosen_grp.emails.get_deleted_set().values('id','addr'))
        expect_softdelete_emails = sort_nested_object(expect_softdelete_emails)
        actual_softdelete_emails = sort_nested_object(actual_softdelete_emails)
        self.assertListEqual(expect_softdelete_emails, actual_softdelete_emails)
        # check soft-deleted role relations
        expect_softdelete_role_rels = set(map(lambda obj:obj.role.id, self._objs[GenericUserAppliedRole]))
        actual_softdelete_role_rels = set(self._chosen_grp.roles.get_deleted_set().values_list('role', flat=True))
        self.assertSetEqual(expect_softdelete_role_rels, actual_softdelete_role_rels)
    ## end of test_bulk_soft_delete()


    def test_bulk_undelete_full_recover_hierarchy(self):
        grp_hier_before_delete = GenericUserGroupClosure.objects.values('id','depth','ancestor','descendant')
        grp_hier_before_delete = list(grp_hier_before_delete)

        grps = GenericUserGroup.objects.filter(id__in=self._delete_grp_ids)
        grps.delete(profile_id=self._profile.id)
        grps = GenericUserGroup.objects.get_deleted_set().filter(id__in=self._delete_grp_ids)
        grps.undelete(profile_id=self._profile.id)

        grp_hier_after_undelete = GenericUserGroupClosure.objects.values('id','depth','ancestor','descendant')
        grp_hier_after_undelete = list(grp_hier_after_undelete)
        expect_value = sort_nested_object(grp_hier_before_delete)
        actual_value = sort_nested_object(grp_hier_after_undelete)
        self.assertListEqual(expect_value, actual_value)
        # some related fields still perform hard-delete and shouldn't keep any data
        related_field_names = ('phones','locations',)
        for field_name in related_field_names:
            related_field = getattr(self._chosen_grp, field_name)
            expect_num    = related_field.all().count()
            self.assertEqual(expect_num, 0)
        # check undeleted emails
        expect_undelete_emails = _data[EmailAddress]
        actual_undelete_emails = list(self._chosen_grp.emails.all().values('id','addr'))
        expect_undelete_emails = sort_nested_object(expect_undelete_emails)
        actual_undelete_emails = sort_nested_object(actual_undelete_emails)
        self.assertListEqual(expect_undelete_emails, actual_undelete_emails)
        # check soft-deleted role relations
        expect_undelete_role_rels = set(map(lambda obj:obj.role.id, self._objs[GenericUserAppliedRole]))
        actual_undelete_role_rels = set(self._chosen_grp.roles.all().values_list('role', flat=True))
        self.assertSetEqual(expect_undelete_role_rels, actual_undelete_role_rels)
        # check soft-deleted quota arrangements
        expect_undelete_quota_rels = set(map(lambda obj:(obj.material.id, obj.maxnum) , self._objs[UserQuotaRelation]))
        actual_undelete_quota_rels = set(self._chosen_grp.quota.all().values_list('material', 'maxnum',))
        self.assertSetEqual(expect_undelete_quota_rels, actual_undelete_quota_rels)


    def _bulk_undelete_evict_hierarchy(self, delete_grp_ids, delete_2nd_grp_id, expect_evict_ids):
        grp_hier_before_delete = GenericUserGroupClosure.objects.values('id','depth','ancestor','descendant')
        grp_hier_before_delete = list(grp_hier_before_delete)
        grps = GenericUserGroup.objects.filter(id__in=delete_grp_ids)
        grps.delete(profile_id=self._profile.id)
        grps = GenericUserGroup.objects.filter(id=delete_2nd_grp_id)
        grps.delete(profile_id=self._profile.id)
        grps = GenericUserGroup.objects.get_deleted_set().filter(id__in=delete_grp_ids)
        grps.undelete(profile_id=self._profile.id)

        grps_after_undelete = GenericUserGroup.objects.values('id','name')
        grp_hier_after_undelete = GenericUserGroupClosure.objects.values('id','depth','ancestor','descendant')
        # raise assertion error if the new tree set is corrupted
        group_hierarchy = TreeNodeMixin.gen_from_closure_data(entity_data=grps_after_undelete,
                closure_data=grp_hier_after_undelete)
        grp_hier_after_undelete = list(grp_hier_after_undelete)

        delete_ops_history = expect_evict_ids.copy() # (delete_grp_ids[0], delete_2nd_grp_id,)
        delete_ops_history.append( delete_2nd_grp_id )
        _fn = lambda d: d['descendant'] in expect_evict_ids and d['ancestor'] in expect_evict_ids
        undel_grps = list(filter(_fn, grp_hier_before_delete))

        affected_path_map = self._get_affected_paths(delete_ids=delete_ops_history, data=grp_hier_before_delete)
        grp_hier_before_delete_map = {(item['ancestor'], item['descendant']):item for item in grp_hier_before_delete}
        for grp_id in delete_ops_history:
            path_map = affected_path_map[grp_id]
            keys = [(asc,desc) for asc in path_map['asc'] for desc in path_map['desc']]
            edit_paths = list(map(lambda key: grp_hier_before_delete_map[key], keys))
            for path in edit_paths:
                path['depth'] -= 1
        _fn = lambda d: d['descendant'] not in delete_ops_history and d['ancestor'] not in delete_ops_history
        grp_hier_without_eviction = list(filter(_fn, grp_hier_before_delete))
        grp_hier_without_eviction.extend( undel_grps )
        expect_value = sort_nested_object(grp_hier_without_eviction)
        actual_value = sort_nested_object(grp_hier_after_undelete)
        self.assertListEqual(actual_value, expect_value)
        return group_hierarchy

    def test_bulk_undelete_evict_hierarchy_1(self):
        self._bulk_undelete_evict_hierarchy(delete_grp_ids=(7, 5), delete_2nd_grp_id=4, expect_evict_ids=[7])

    def test_bulk_undelete_evict_hierarchy_2(self):
        self._bulk_undelete_evict_hierarchy(delete_grp_ids=(4, 5), delete_2nd_grp_id=7, expect_evict_ids=[4])

    def test_bulk_undelete_evict_hierarchy_3(self):
        self._bulk_undelete_evict_hierarchy(delete_grp_ids=(4, 5), delete_2nd_grp_id=11, expect_evict_ids=[4])

    def test_bulk_undelete_evict_hierarchy_4(self):
        group_hierarchy = self._bulk_undelete_evict_hierarchy(delete_grp_ids=(11, 5), delete_2nd_grp_id=4, expect_evict_ids=[11])
        expect_tree_roots = set([3, 11])
        actual_tree_roots = set(map(lambda t:t.value['id'] , group_hierarchy))
        self.assertSetEqual(expect_tree_roots, actual_tree_roots)

    def test_bulk_undelete_evict_hierarchy_5(self):
        group_hierarchy = self._bulk_undelete_evict_hierarchy(delete_grp_ids=(3, 6), delete_2nd_grp_id=7, expect_evict_ids=[3])
        expect_tree_roots = set([3, 4, 5])
        actual_tree_roots = set(map(lambda t:t.value['id'] , group_hierarchy))
        self.assertSetEqual(expect_tree_roots, actual_tree_roots)

    def test_bulk_undelete_evict_hierarchy_6(self):
        group_hierarchy = self._bulk_undelete_evict_hierarchy(delete_grp_ids=(7, 6), delete_2nd_grp_id=3, expect_evict_ids=[6,7])
        expect_tree_roots = set([4, 5, 6, 7])
        actual_tree_roots = set(map(lambda t:t.value['id'] , group_hierarchy))
        self.assertSetEqual(expect_tree_roots, actual_tree_roots)

    def test_bulk_undelete_evict_hierarchy_7(self):
        group_hierarchy = self._bulk_undelete_evict_hierarchy(delete_grp_ids=(8, 7, 6), delete_2nd_grp_id=3, expect_evict_ids=[6,7,8])
        expect_tree_roots = set([4, 5, 6, 7, 8])
        actual_tree_roots = set(map(lambda t:t.value['id'] , group_hierarchy))
        self.assertSetEqual(expect_tree_roots, actual_tree_roots)

    def test_bulk_undelete_evict_hierarchy_8(self):
        group_hierarchy = self._bulk_undelete_evict_hierarchy(delete_grp_ids=(11,14), delete_2nd_grp_id=5, expect_evict_ids=[14])
        expect_tree_roots = set([3, 14])
        actual_tree_roots = set(map(lambda t:t.value['id'] , group_hierarchy))
        self.assertSetEqual(expect_tree_roots, actual_tree_roots)

    def test_bulk_undelete_evict_hierarchy_9(self):
        group_hierarchy = self._bulk_undelete_evict_hierarchy(delete_grp_ids=(11,4), delete_2nd_grp_id=13, expect_evict_ids=[4])
        expect_tree_roots = set([3, 4])
        actual_tree_roots = set(map(lambda t:t.value['id'] , group_hierarchy))
        self.assertSetEqual(expect_tree_roots, actual_tree_roots)


