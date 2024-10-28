import json
from datetime import timedelta

from django.test import TransactionTestCase
from django.utils import timezone as django_timezone
from django.core.exceptions import ObjectDoesNotExist
from django.contrib.auth.models import Permission as ModelLevelPermission
from django.contrib.contenttypes.models import ContentType

from ecommerce_common.models.constants import ROLE_ID_SUPERUSER, ROLE_ID_STAFF
from user_management.models.common import AppCodeOptions
from user_management.models.base import (
    GenericUserProfile,
    GenericUserGroup,
    GenericUserGroupClosure,
    GenericUserGroupRelation,
    GenericUserAppliedRole,
    QuotaMaterial,
    UserQuotaRelation,
)
from user_management.models.auth import LoginAccount, Role

from ..common import _fixtures

_expiry_time = django_timezone.now() + timedelta(minutes=10)


class ProfileCommonTestCase(TransactionTestCase):
    def _setup_groups(self):
        # the tree structure of the group hierarchy
        #             3
        #          /    \
        #         4      7
        #        /      / \
        #       5      8  9
        #      /
        #     6
        #
        group_data = _fixtures[GenericUserGroup]
        grp_obj_map = dict(
            map(lambda d: (d["id"], GenericUserGroup.objects.create(**d)), group_data)
        )
        group_closure_data = [
            {
                "id": 1,
                "depth": 0,
                "ancestor": grp_obj_map[3],
                "descendant": grp_obj_map[3],
            },
            {
                "id": 2,
                "depth": 0,
                "ancestor": grp_obj_map[4],
                "descendant": grp_obj_map[4],
            },
            {
                "id": 3,
                "depth": 0,
                "ancestor": grp_obj_map[5],
                "descendant": grp_obj_map[5],
            },
            {
                "id": 4,
                "depth": 0,
                "ancestor": grp_obj_map[6],
                "descendant": grp_obj_map[6],
            },
            {
                "id": 5,
                "depth": 1,
                "ancestor": grp_obj_map[3],
                "descendant": grp_obj_map[4],
            },
            {
                "id": 6,
                "depth": 1,
                "ancestor": grp_obj_map[4],
                "descendant": grp_obj_map[5],
            },
            {
                "id": 7,
                "depth": 1,
                "ancestor": grp_obj_map[5],
                "descendant": grp_obj_map[6],
            },
            {
                "id": 8,
                "depth": 2,
                "ancestor": grp_obj_map[3],
                "descendant": grp_obj_map[5],
            },
            {
                "id": 9,
                "depth": 2,
                "ancestor": grp_obj_map[4],
                "descendant": grp_obj_map[6],
            },
            {
                "id": 10,
                "depth": 3,
                "ancestor": grp_obj_map[3],
                "descendant": grp_obj_map[6],
            },
            # ---------
            {
                "id": 11,
                "depth": 0,
                "ancestor": grp_obj_map[7],
                "descendant": grp_obj_map[7],
            },
            {
                "id": 12,
                "depth": 0,
                "ancestor": grp_obj_map[8],
                "descendant": grp_obj_map[8],
            },
            {
                "id": 13,
                "depth": 1,
                "ancestor": grp_obj_map[3],
                "descendant": grp_obj_map[7],
            },
            {
                "id": 14,
                "depth": 1,
                "ancestor": grp_obj_map[7],
                "descendant": grp_obj_map[8],
            },
            {
                "id": 15,
                "depth": 2,
                "ancestor": grp_obj_map[3],
                "descendant": grp_obj_map[8],
            },
            # ---------
            {
                "id": 16,
                "depth": 0,
                "ancestor": grp_obj_map[9],
                "descendant": grp_obj_map[9],
            },
            {
                "id": 17,
                "depth": 0,
                "ancestor": grp_obj_map[10],
                "descendant": grp_obj_map[10],
            },
            {
                "id": 18,
                "depth": 1,
                "ancestor": grp_obj_map[9],
                "descendant": grp_obj_map[10],
            },
        ]
        list(
            map(
                lambda d: GenericUserGroupClosure.objects.create(**d),
                group_closure_data,
            )
        )
        return grp_obj_map

    def _setup_roles(self):
        app_labels = (
            "auth",
            "user_management",
        )
        role_data = [
            {"id": ROLE_ID_SUPERUSER, "name": "my default superuser"},
            {"id": ROLE_ID_STAFF, "name": "my default staff"},
            {
                "id": 4,
                "name": "my role on auth",
            },
            {
                "id": 5,
                "name": "my role on usrmgt",
            },
            {
                "id": 6,
                "name": "my third custom role",
            },
            {
                "id": 7,
                "name": "my fourth custom role",
            },
        ]
        roles = tuple(map(lambda d: Role.objects.create(**d), role_data))
        roles_iter = iter(roles[2:])
        for app_label in app_labels:
            qset = ModelLevelPermission.objects.filter(
                content_type__app_label=app_label
            )
            role = next(roles_iter)
            role.permissions.set(qset[:3])
        return roles

    def _setup_quota_mat(self):
        appcodes = AppCodeOptions
        material_data = [
            {"id": 1, "app_code": appcodes.user_management.value, "mat_code": 3},
            {"id": 2, "app_code": appcodes.user_management.value, "mat_code": 2},
            {"id": 3, "app_code": appcodes.user_management.value, "mat_code": 1},
            {"id": 4, "app_code": appcodes.product.value, "mat_code": 2},
            {"id": 5, "app_code": appcodes.product.value, "mat_code": 1},
            {"id": 6, "app_code": appcodes.media.value, "mat_code": 1},
        ]
        quota_mat = tuple(map(lambda d: QuotaMaterial(**d), material_data))
        QuotaMaterial.objects.bulk_create(quota_mat)
        return quota_mat

    def setUp(self):
        profile_data = _fixtures[GenericUserProfile][0]
        profile = GenericUserProfile.objects.create(**profile_data)
        account_data = {
            "username": "ImStaff",
            "password": "dontexpose",
            "is_active": True,
            "is_staff": True,
            "is_superuser": False,
            "profile": profile,
            "password_last_updated": django_timezone.now(),
        }
        account = LoginAccount.objects.create_user(**account_data)
        profile_2nd_data = _fixtures[GenericUserProfile][1]
        profile_2nd = GenericUserProfile.objects.create(**profile_2nd_data)
        self._profile = profile
        self._profile_2nd = profile_2nd
        self._groups = self._setup_groups()
        self._roles = self._setup_roles()
        self._quota_mat = self._setup_quota_mat()
        grp_rel_data = [
            {
                "group": self._groups[6],
                "profile": self._profile,
                "approved_by": self._profile_2nd,
            },
            {
                "group": self._groups[8],
                "profile": self._profile,
                "approved_by": self._profile_2nd,
            },
        ]
        for data in grp_rel_data:
            GenericUserGroupRelation.objects.create(**data)

    def tearDown(self):
        pass


class ProfileCreationTestCase(ProfileCommonTestCase):
    def test_inherit_roles(self):
        grp_ct = ContentType.objects.get_for_model(GenericUserGroup)
        # subcase #1 : add roles to parent group, see whether all its descendant groups can inherit the same roles
        role_rel_data = [
            {
                "expiry": _expiry_time,
                "approved_by": self._profile_2nd,
                "role": self._roles[2],
                "user_type": grp_ct,
                "user_id": self._groups[3].id,
            },
            {
                "expiry": _expiry_time,
                "approved_by": self._profile_2nd,
                "role": self._roles[3],
                "user_type": grp_ct,
                "user_id": self._groups[5].id,
            },
        ]
        for data in role_rel_data:
            GenericUserAppliedRole.objects.create(**data)
        actual_roles = self._profile.inherit_roles
        expect_roles = self._roles[2:4]
        self.assertSetEqual(set(actual_roles), set(expect_roles))
        # subcase #2 : add new group to user profile and new role to parent group, see how roles change
        self._profile.groups.create(
            group=self._groups[10], approved_by=self._profile_2nd
        )
        self._groups[9].roles.create(
            expiry=None, approved_by=self._profile_2nd, role=self._roles[4]
        )
        actual_roles = self._profile.inherit_roles
        expect_roles = self._roles[2:5]
        self.assertSetEqual(set(actual_roles), set(expect_roles))
        # subcase #3 : assume roles are expired, that should NOT be referred to by inherit_roles property
        expired_time = django_timezone.now() - timedelta(minutes=15)
        saved_role_rel = self._groups[3].roles.get(role=self._roles[2])
        saved_role_rel.expiry = expired_time
        saved_role_rel.save(update_fields=["expiry"])
        self._groups[3].roles.create(
            expiry=expired_time, approved_by=self._profile_2nd, role=self._roles[5]
        )
        actual_roles = self._profile.inherit_roles
        expect_roles = self._roles[3:5]
        self.assertSetEqual(set(actual_roles), set(expect_roles))

    def test_direct_roles(self):
        expect_roles = self._roles[:-1]
        for role in expect_roles:
            data = {
                "expiry": _expiry_time,
                "approved_by": self._profile_2nd,
                "role": role,
            }
            applied_role = GenericUserAppliedRole(**data)
            self._profile.roles.add(applied_role, bulk=False)
        actual_roles = self._profile.direct_roles
        self.assertSetEqual(set(actual_roles), set(expect_roles))
        # subcase #2 : assume role is expired, that should NOT be referred to by inherit_roles property
        expired_time = django_timezone.now() - timedelta(minutes=15)
        saved_roles_rel = self._profile.roles.filter(role__in=self._roles[:2])
        for role_rel in saved_roles_rel:
            role_rel.expiry = expired_time
        saved_roles_rel.bulk_update(saved_roles_rel, fields=["expiry"])
        expect_roles = self._roles[2:-1]
        actual_roles = self._profile.direct_roles
        self.assertSetEqual(set(actual_roles), set(expect_roles))

    def test_privilege_status(self):
        root_node_grp = self._groups[3]
        grp_ct = ContentType.objects.get_for_model(GenericUserGroup)
        prof_ct = ContentType.objects.get_for_model(GenericUserProfile)
        data = {
            "expiry": _expiry_time,
            "approved_by": self._profile_2nd,
            "role": self._roles[2],
            "user_type": grp_ct,
            "user_id": root_node_grp.id,
        }
        GenericUserAppliedRole.objects.create(**data)
        actual_status = self._profile.privilege_status
        self.assertEqual(actual_status, GenericUserProfile.NONE)
        # ------------------------------
        self._test_privilege_status(data=data)
        # ------------------------------
        data["user_type"] = prof_ct
        data["user_id"] = self._profile.id
        data["role"] = self._roles[3]
        GenericUserAppliedRole.objects.create(**data)
        actual_status = self._profile.privilege_status
        self.assertEqual(actual_status, GenericUserProfile.NONE)
        # ------------------------------
        self._test_privilege_status(data=data)

    def _test_privilege_status(self, data):
        data["role"] = self._roles[0]
        superuser_rel = GenericUserAppliedRole.objects.create(**data)
        actual_status = self._profile.privilege_status
        self.assertEqual(actual_status, GenericUserProfile.SUPERUSER)
        superuser_rel.delete(hard=True)
        # ------------------------------
        data["role"] = self._roles[1]
        staff_rel = GenericUserAppliedRole.objects.create(**data)
        actual_status = self._profile.privilege_status
        self.assertEqual(actual_status, GenericUserProfile.STAFF)
        staff_rel.delete(hard=True)

    def _get_default_quota_rel_data(self):
        grp_ct = ContentType.objects.get_for_model(GenericUserGroup)
        prof_ct = ContentType.objects.get_for_model(GenericUserProfile)
        out = [
            {
                "user_type": grp_ct,
                "user_id": self._groups[3].id,
                "material": self._quota_mat[0],
                "maxnum": 15,
            },
            {
                "user_type": grp_ct,
                "user_id": self._groups[4].id,
                "material": self._quota_mat[0],
                "maxnum": 26,
            },
            {
                "user_type": grp_ct,
                "user_id": self._groups[5].id,
                "material": self._quota_mat[0],
                "maxnum": 37,
            },
            {
                "user_type": grp_ct,
                "user_id": self._groups[7].id,
                "material": self._quota_mat[0],
                "maxnum": 25,
            },
            # ---------------------
            {
                "user_type": grp_ct,
                "user_id": self._groups[3].id,
                "material": self._quota_mat[1],
                "maxnum": 18,
            },
            {
                "user_type": grp_ct,
                "user_id": self._groups[4].id,
                "material": self._quota_mat[1],
                "maxnum": 29,
            },
            {
                "user_type": grp_ct,
                "user_id": self._groups[5].id,
                "material": self._quota_mat[1],
                "maxnum": 23,
            },
            # ---------------------
            {
                "user_type": grp_ct,
                "user_id": self._groups[5].id,
                "material": self._quota_mat[2],
                "maxnum": 6,
            },
            {
                "user_type": grp_ct,
                "user_id": self._groups[7].id,
                "material": self._quota_mat[2],
                "maxnum": 5,
            },
            # ---------------------
            {
                "user_type": grp_ct,
                "user_id": self._groups[3].id,
                "material": self._quota_mat[3],
                "maxnum": 9,
            },
            {
                "user_type": grp_ct,
                "user_id": self._groups[4].id,
                "material": self._quota_mat[3],
                "maxnum": 10,
            },
            {
                "user_type": grp_ct,
                "user_id": self._groups[5].id,
                "material": self._quota_mat[3],
                "maxnum": 12,
            },
            # ---------------------
            {
                "user_type": grp_ct,
                "user_id": self._groups[7].id,
                "material": self._quota_mat[4],
                "maxnum": 2,
            },
            # ---------------------
            {
                "user_type": grp_ct,
                "user_id": self._groups[4].id,
                "material": self._quota_mat[5],
                "maxnum": 89,
            },
            {
                "user_type": grp_ct,
                "user_id": self._groups[5].id,
                "material": self._quota_mat[5],
                "maxnum": 21,
            },
            {
                "user_type": grp_ct,
                "user_id": self._groups[7].id,
                "material": self._quota_mat[5],
                "maxnum": 90,
            },
            # ---------------------
            {
                "user_type": prof_ct,
                "user_id": self._profile.id,
                "material": self._quota_mat[0],
                "maxnum": 36,
            },
            {
                "user_type": prof_ct,
                "user_id": self._profile.id,
                "material": self._quota_mat[2],
                "maxnum": 8,
            },
            {
                "user_type": prof_ct,
                "user_id": self._profile.id,
                "material": self._quota_mat[4],
                "maxnum": 4,
            },
        ]
        return out

    ## end of _get_default_quota_rel_data()

    def test_all_quota(self):
        # subcase #1 : quota arragements inherited from applied groups
        quota_rel_data = self._get_default_quota_rel_data()
        def ut_setup_obj(d):
            m = UserQuotaRelation(**d)
            m.save()
            return m
        quota_rel = list(map(ut_setup_obj, quota_rel_data))
        indexes = [5, 7, 11, 12, 15, 2]
        filtered_quota_rel_data = map(lambda idx: quota_rel_data[idx], indexes)
        expect_quota = dict(
            map(lambda d: (d["material"].id, d["maxnum"]), filtered_quota_rel_data)
        )
        actual_quota = self._profile.inherit_quota
        self.assertDictEqual(expect_quota, actual_quota)
        # subcase #2 : quota arragements directly applied to the user
        indexes = [5, 17, 11, 18, 15, 2]
        filtered_quota_rel_data = map(lambda idx: quota_rel_data[idx], indexes)
        expect_quota = dict(
            map(lambda d: (d["material"].id, d["maxnum"]), filtered_quota_rel_data)
        )
        actual_quota = self._profile.all_quota
        self.assertDictEqual(expect_quota, actual_quota)
        # subcase #3 : quota arragements expired, test the 2 subcases above again
        expired_time = django_timezone.now() - timedelta(minutes=15)
        quota_rel[2].expiry = expired_time
        quota_rel[2].save(update_fields=["expiry"])
        quota_rel[11].expiry = expired_time
        quota_rel[11].save(update_fields=["expiry"])
        indexes = [5, 7, 10, 12, 15, 1]
        filtered_quota_rel_data = map(lambda idx: quota_rel_data[idx], indexes)
        expect_quota = dict(
            map(lambda d: (d["material"].id, d["maxnum"]), filtered_quota_rel_data)
        )
        actual_quota = self._profile.inherit_quota
        self.assertDictEqual(expect_quota, actual_quota)
        indexes = [5, 17, 10, 18, 15, 16]
        filtered_quota_rel_data = map(lambda idx: quota_rel_data[idx], indexes)
        expect_quota = dict(
            map(lambda d: (d["material"].id, d["maxnum"]), filtered_quota_rel_data)
        )
        actual_quota = self._profile.all_quota
        self.assertDictEqual(expect_quota, actual_quota)

    def test_activate_deactivate(self):
        verify_items = [
            {
                "role": self._roles[0],
                "expect_is_staff": True,
                "expect_is_superuser": True,
            },
            {
                "role": self._roles[1],
                "expect_is_staff": True,
                "expect_is_superuser": False,
            },
            {
                "role": self._roles[2],
                "expect_is_staff": False,
                "expect_is_superuser": False,
            },
        ]
        # ensure there is at least one active superuser account in the system,
        # since another active superuser account will be deleted later in this test case
        default_su_account_data = _fixtures[LoginAccount][0].copy()
        edit_data = {
            "is_staff": True,
            "is_superuser": True,
            "is_active": True,
            "profile": self._profile_2nd,
            "password_last_updated": django_timezone.now(),
        }
        default_su_account_data.update(edit_data)
        LoginAccount.objects.create_user(**default_su_account_data)
        # data for new login account
        expect_account_data = _fixtures[LoginAccount][1].copy()
        edit_data = {
            "is_staff": False,
            "is_active": False,
            "password_last_updated": django_timezone.now(),
        }
        expect_account_data.update(edit_data)
        self._profile.account.delete()
        # 3 subcases here, each activates account with specific privilege status
        for verify_item in verify_items:
            self._profile.refresh_from_db()
            with self.assertRaises(ObjectDoesNotExist):
                self._profile.account
            self._groups[4].roles.all(with_deleted=True).delete(hard=True)
            self._groups[4].roles.create(
                role=verify_item["role"], approved_by=self._profile_2nd
            )
            # first-time activation
            actual_account = self._profile.activate(
                new_account_data=expect_account_data
            )
            self._profile.account
            expect_passwd = expect_account_data["password"]
            passwd_result = actual_account.check_password(expect_passwd)
            self.assertTrue(passwd_result)
            self.assertTrue(actual_account.is_active)
            # check account privilege is sync with roles
            self.assertEqual(actual_account.is_staff, verify_item["expect_is_staff"])
            self.assertEqual(
                actual_account.is_superuser, verify_item["expect_is_superuser"]
            )
            # assume deactivated, then activate again
            self._profile.deactivate()
            self.assertFalse(actual_account.is_active)
            actual_account = self._profile.activate(new_account_data=None)
            self.assertTrue(actual_account.is_active)
            self.assertEqual(actual_account.is_staff, verify_item["expect_is_staff"])
            self.assertEqual(
                actual_account.is_superuser, verify_item["expect_is_superuser"]
            )
            # deactivated and delete account
            self._profile.deactivate(remove_account=True)

    ## end of def test_activate_deactivate


## end of class ProfileCreationTestCase


class ProfileDeletionTestCase(ProfileCommonTestCase):
    def setUp(self):
        super().setUp()

    def test_hard_delete(self):
        self._profile.account
        prof_id = self._profile.id
        grp_ids = self._profile.groups.values_list("group", flat=True)
        self.assertSetEqual({6, 8}, set(grp_ids))
        self._profile.delete(hard=True)
        qset = GenericUserGroupRelation.objects.all(with_deleted=True).filter(
            id={"profile": prof_id, "group__in": grp_ids}
        )
        self.assertFalse(qset.exists())
        qset = GenericUserProfile.objects.all(with_deleted=True).filter(id=prof_id)
        self.assertFalse(qset.exists())
        qset = LoginAccount.objects.filter(profile__id=prof_id)
        self.assertFalse(qset.exists())

    def test_soft_delete(self):
        self._profile.account
        prof_id = self._profile.id
        grp_ids = self._profile.groups.values_list("group", flat=True)
        self.assertSetEqual({6, 8}, set(grp_ids))
        self._profile.delete(profile_id=self._profile_2nd.id)
        self._profile.refresh_from_db()
        qset = GenericUserGroupRelation.objects.filter(
            profile=prof_id, group__in={6, 8}
        )
        self.assertFalse(qset.exists())
        qset = GenericUserGroupRelation.objects.all(with_deleted=True).filter(
            profile=prof_id, group__in=list(grp_ids)
        )
        self.assertTrue(qset.exists())
        softdel_grp_ids = qset.values_list("group", flat=True)
        self.assertSetEqual(set(softdel_grp_ids), set(grp_ids))
        with self.assertRaises(ObjectDoesNotExist):
            self._profile.account
        # undelete
        profile = GenericUserProfile.objects.get_deleted_set().get(id=prof_id)
        # In typical case, the user who deleted a profile is the only one to recover the soft-deleted profile
        profile.undelete(profile_id=self._profile_2nd.id)
        self._profile.refresh_from_db()
        self.assertFalse(self._profile.is_deleted())
        grp_ids = self._profile.groups.values_list("group", flat=True)
        self.assertSetEqual(set(softdel_grp_ids), set(grp_ids))
        with self.assertRaises(ObjectDoesNotExist):
            self._profile.account

    def test_self_removal(self):
        # 3rd user profile
        profile_3rd = GenericUserProfile.objects.create(
            **_fixtures[GenericUserProfile][2]
        )
        # 4th user profile
        profile_4th = GenericUserProfile.objects.create(
            **_fixtures[GenericUserProfile][3]
        )
        # 5th user profile, superuser
        profile_5th = GenericUserProfile.objects.create(
            **_fixtures[GenericUserProfile][4]
        )
        profile_5th.roles.create(
            role=self._roles[0], approved_by=profile_5th, expiry=None
        )
        # set up group relations
        grp_rel_data = [
            {
                "group": self._groups[5],
                "profile": self._profile_2nd,
                "approved_by": profile_3rd,
            },
            {
                "group": self._groups[3],
                "profile": profile_3rd,
                "approved_by": profile_5th,
            },
            {
                "group": self._groups[7],
                "profile": profile_4th,
                "approved_by": profile_5th,
            },
        ]
        for data in grp_rel_data:
            GenericUserGroupRelation.objects.create(**data)
        # start deleting ...
        revomal_sequence = (self._profile, self._profile_2nd, profile_3rd, profile_4th)
        for profile in revomal_sequence:
            profile.delete(profile_id=profile.id)
            profile.refresh_from_db()
        # undelete sequence
        profile_4th.undelete(profile_id=profile_5th.id)
        with self.assertRaises(
            ObjectDoesNotExist
        ):  # 4th user does not have recovery permission on 2nd user
            self._profile_2nd.undelete(profile_id=profile_4th.id)
        profile_3rd.undelete(profile_id=profile_5th.id)
        self._profile_2nd.undelete(profile_id=profile_3rd.id)
        self._profile.undelete(profile_id=self._profile_2nd.id)
        for profile in revomal_sequence:
            profile.refresh_from_db()
            self.assertFalse(profile.is_deleted())
        # --------------
        self._profile.delete(profile_id=self._profile.id)
        self._profile.refresh_from_db()
        self._profile.undelete(profile_id=profile_4th.id)
        self._profile.refresh_from_db()
        self.assertFalse(self._profile.is_deleted())


## end of class ProfileDeletionTestCase
