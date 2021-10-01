import json

from django.test import TransactionTestCase
from django.utils import timezone as django_timezone
from django.contrib.auth.models import Permission as ModelLevelPermission
from django.contrib.contenttypes.models  import ContentType

from common.models.constants     import ROLE_ID_SUPERUSER, ROLE_ID_STAFF
from user_management.models.common import AppCodeOptions
from user_management.models.base import GenericUserProfile, GenericUserGroup, GenericUserGroupClosure, GenericUserGroupRelation, GenericUserAppliedRole, QuotaMaterial, UserQuotaRelation
from user_management.models.auth import LoginAccount, Role


class ProfileCreationTestCase(TransactionTestCase):
    def _setup_groups(self):
        group_data = [
            {'id':4, 'name':'Research center'},
            {'id':5, 'name':'CPU subsystem team'},
            {'id':6, 'name':'hardware team'},
            {'id':7, 'name':'bus team'},
            {'id':8, 'name':'SoC Layout'},
            {'id':9, 'name':'Tooling'},
        ]
        grp_obj_map = dict(map(lambda d: (d['id'], GenericUserGroup.objects.create(**d)), group_data))
        group_closure_data = [
            {'id':1,  'depth':0, 'ancestor':grp_obj_map[4], 'descendant':grp_obj_map[4]},
            {'id':2,  'depth':0, 'ancestor':grp_obj_map[5], 'descendant':grp_obj_map[5]},
            {'id':3,  'depth':0, 'ancestor':grp_obj_map[6], 'descendant':grp_obj_map[6]},
            {'id':4,  'depth':0, 'ancestor':grp_obj_map[7], 'descendant':grp_obj_map[7]},
            {'id':5,  'depth':1, 'ancestor':grp_obj_map[4], 'descendant':grp_obj_map[5]},
            {'id':6,  'depth':1, 'ancestor':grp_obj_map[5], 'descendant':grp_obj_map[6]},
            {'id':7,  'depth':1, 'ancestor':grp_obj_map[6], 'descendant':grp_obj_map[7]},
            {'id':8,  'depth':2, 'ancestor':grp_obj_map[4], 'descendant':grp_obj_map[6]},
            {'id':9,  'depth':2, 'ancestor':grp_obj_map[5], 'descendant':grp_obj_map[7]},
            {'id':10, 'depth':3, 'ancestor':grp_obj_map[4], 'descendant':grp_obj_map[7]},
            # ---------
            {'id':11,  'depth':0, 'ancestor':grp_obj_map[8], 'descendant':grp_obj_map[8]},
            {'id':12,  'depth':0, 'ancestor':grp_obj_map[9], 'descendant':grp_obj_map[9]},
            {'id':13,  'depth':1, 'ancestor':grp_obj_map[4], 'descendant':grp_obj_map[8]},
            {'id':14,  'depth':1, 'ancestor':grp_obj_map[8], 'descendant':grp_obj_map[9]},
            {'id':15,  'depth':2, 'ancestor':grp_obj_map[4], 'descendant':grp_obj_map[9]},
        ]
        list(map(lambda d: GenericUserGroupClosure.objects.create(**d) , group_closure_data))
        return list(grp_obj_map.values())

    def _setup_roles(self):
        app_labels = ('auth', 'user_management',)
        role_data = [
            {'id':ROLE_ID_SUPERUSER, 'name':'my default superuser'},
            {'id':ROLE_ID_STAFF    , 'name':'my default staff'},
            {'id':4, 'name':'my role on auth',  },
            {'id':5, 'name':'my role on usrmgt',},
        ]
        roles = tuple(map(lambda d:Role.objects.create(**d) , role_data))
        roles_iter = iter(roles[2:])
        for app_label in app_labels:
            qset = ModelLevelPermission.objects.filter(content_type__app_label=app_label)
            role = next(roles_iter)
            role.permissions.set(qset[:3])
        return roles

    def _setup_quota_mat(self):
        appcodes = AppCodeOptions
        material_data = [
            {'id':1, 'app_code':appcodes.user_management.value, 'mat_code':3},
            {'id':2, 'app_code':appcodes.user_management.value, 'mat_code':2},
            {'id':3, 'app_code':appcodes.user_management.value, 'mat_code':1},
            {'id':4, 'app_code':appcodes.product.value, 'mat_code':2},
            {'id':5, 'app_code':appcodes.product.value, 'mat_code':1},
            {'id':6, 'app_code':appcodes.fileupload.value, 'mat_code':1},
        ]
        quota_mat = tuple(map(lambda d: QuotaMaterial(**d) , material_data))
        QuotaMaterial.objects.bulk_create(quota_mat)
        return quota_mat

    def setUp(self):
        profile_data = {'id': 3, 'first_name':'Brooklynn', 'last_name':'Jenkins'}
        profile = GenericUserProfile.objects.create(**profile_data)
        account_data = {'username':'ImStaff', 'password':'dontexpose', 'is_active':True, 'is_staff':True,
                'is_superuser':False, 'profile':profile, 'password_last_updated':django_timezone.now(), }
        account = LoginAccount.objects.create_user(**account_data)
        profile_2nd_data = {'id': 4, 'first_name':'Texassal', 'last_name':'Bovaski'}
        profile_2nd = GenericUserProfile.objects.create(**profile_2nd_data)
        self._profile = profile
        self._profile_2nd = profile_2nd
        self._groups = self._setup_groups()
        self._roles  = self._setup_roles()
        self._quota_mat = self._setup_quota_mat()
        grp_rel_data = {'group':self._groups[3], 'profile':profile, 'approved_by':profile_2nd}
        GenericUserGroupRelation.objects.create(**grp_rel_data)
        grp_rel_data = {'group':self._groups[5], 'profile':profile, 'approved_by':profile_2nd}
        GenericUserGroupRelation.objects.create(**grp_rel_data)


    def tearDown(self):
        pass

    def test_inherit_roles(self):
        grp_ct = ContentType.objects.get_for_model(GenericUserGroup)
        data = {'last_updated':django_timezone.now(), 'approved_by':self._profile_2nd, 'role': self._roles[2],
                'user_type':grp_ct, 'user_id': self._groups[0].id}
        GenericUserAppliedRole.objects.create(**data)
        data = {'last_updated':django_timezone.now(), 'approved_by':self._profile_2nd, 'role': self._roles[3],
                'user_type':grp_ct, 'user_id': self._groups[2].id}
        GenericUserAppliedRole.objects.create(**data)
        actual_roles = self._profile.inherit_roles
        expect_roles = self._roles[2:]
        self.assertSetEqual(set(actual_roles), set(expect_roles))


    def test_direct_roles(self):
        for role in self._roles:
            data = {'last_updated':django_timezone.now(), 'approved_by':self._profile_2nd, 'role': role}
            applied_role = GenericUserAppliedRole(**data)
            self._profile.roles.add(applied_role, bulk=False)
        actual_roles = self._profile.direct_roles
        expect_roles = self._roles
        self.assertSetEqual(set(actual_roles), set(expect_roles))


    def test_privilege_status(self):
        root_node_grp = self._groups[0]
        grp_ct  = ContentType.objects.get_for_model(GenericUserGroup)
        prof_ct = ContentType.objects.get_for_model(GenericUserProfile)
        data = {'last_updated':django_timezone.now(), 'approved_by':self._profile_2nd, 'role': self._roles[2],
                'user_type':grp_ct, 'user_id': root_node_grp.id}
        GenericUserAppliedRole.objects.create(**data)
        actual_status = self._profile.privilege_status
        self.assertEqual(actual_status, GenericUserProfile.NONE)
        # ------------------------------
        self._test_privilege_status(data=data)
        # ------------------------------
        data['user_type'] = prof_ct
        data['user_id']   = self._profile.id
        data['role'] = self._roles[3]
        GenericUserAppliedRole.objects.create(**data)
        actual_status = self._profile.privilege_status
        self.assertEqual(actual_status, GenericUserProfile.NONE)
        # ------------------------------
        self._test_privilege_status(data=data)

    def _test_privilege_status(self, data):
        data['role'] = self._roles[0]
        superuser_rel = GenericUserAppliedRole.objects.create(**data)
        actual_status = self._profile.privilege_status
        self.assertEqual(actual_status, GenericUserProfile.SUPERUSER)
        superuser_rel.delete(hard=True)
        # ------------------------------
        data['role'] = self._roles[1]
        staff_rel = GenericUserAppliedRole.objects.create(**data)
        actual_status = self._profile.privilege_status
        self.assertEqual(actual_status, GenericUserProfile.STAFF)
        staff_rel.delete(hard=True)


    def test_all_quota(self):
        # --- quota arragements applied in the inherited groups ---
        grp_ct  = ContentType.objects.get_for_model(GenericUserGroup)
        prof_ct  = ContentType.objects.get_for_model(GenericUserProfile)
        quota_rel_data = [
            {'user_type':grp_ct, 'user_id':self._groups[0].id, 'material':self._quota_mat[0], 'maxnum':15},
            {'user_type':grp_ct, 'user_id':self._groups[1].id, 'material':self._quota_mat[0], 'maxnum':26},
            {'user_type':grp_ct, 'user_id':self._groups[2].id, 'material':self._quota_mat[0], 'maxnum':37},
            {'user_type':grp_ct, 'user_id':self._groups[4].id, 'material':self._quota_mat[0], 'maxnum':25},
            # ---------------------
            {'user_type':grp_ct, 'user_id':self._groups[0].id, 'material':self._quota_mat[1], 'maxnum':18},
            {'user_type':grp_ct, 'user_id':self._groups[1].id, 'material':self._quota_mat[1], 'maxnum':29},
            {'user_type':grp_ct, 'user_id':self._groups[2].id, 'material':self._quota_mat[1], 'maxnum':23},
            # ---------------------
            {'user_type':grp_ct, 'user_id':self._groups[2].id, 'material':self._quota_mat[2], 'maxnum':6},
            {'user_type':grp_ct, 'user_id':self._groups[4].id, 'material':self._quota_mat[2], 'maxnum':5},
            # ---------------------
            {'user_type':grp_ct, 'user_id':self._groups[0].id, 'material':self._quota_mat[3], 'maxnum':9},
            {'user_type':grp_ct, 'user_id':self._groups[1].id, 'material':self._quota_mat[3], 'maxnum':10},
            {'user_type':grp_ct, 'user_id':self._groups[2].id, 'material':self._quota_mat[3], 'maxnum':12},
            # ---------------------
            {'user_type':grp_ct, 'user_id':self._groups[4].id, 'material':self._quota_mat[4], 'maxnum':2},
            # ---------------------
            {'user_type':grp_ct, 'user_id':self._groups[1].id, 'material':self._quota_mat[5], 'maxnum':89},
            {'user_type':grp_ct, 'user_id':self._groups[2].id, 'material':self._quota_mat[5], 'maxnum':21},
            {'user_type':grp_ct, 'user_id':self._groups[4].id, 'material':self._quota_mat[5], 'maxnum':90},
            # ---------------------
            {'user_type':prof_ct, 'user_id':self._profile.id, 'material':self._quota_mat[0], 'maxnum':36},
            {'user_type':prof_ct, 'user_id':self._profile.id, 'material':self._quota_mat[2], 'maxnum':8},
            {'user_type':prof_ct, 'user_id':self._profile.id, 'material':self._quota_mat[4], 'maxnum':4},
        ]
        quota_rel = tuple(map(lambda d:UserQuotaRelation(**d) , quota_rel_data))
        UserQuotaRelation.objects.bulk_create(quota_rel)
        indexes = [5, 7, 11, 12, 15, 2]
        filtered_quota_rel_data = map(lambda idx: quota_rel_data[idx], indexes)
        expect_quota = dict(map(lambda d: (d['material'].id, d['maxnum']), filtered_quota_rel_data))
        actual_quota = self._profile.inherit_quota
        self.assertDictEqual(expect_quota, actual_quota)
        # --- quota arragements directly applied in the user ---
        indexes = [5, 17, 11, 18, 15, 2]
        filtered_quota_rel_data = map(lambda idx: quota_rel_data[idx], indexes)
        expect_quota = dict(map(lambda d: (d['material'].id, d['maxnum']), filtered_quota_rel_data))
        actual_quota = self._profile.all_quota
        self.assertDictEqual(expect_quota, actual_quota)
## end of class ProfileCreationTestCase

