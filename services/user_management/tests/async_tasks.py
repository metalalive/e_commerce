import random

from MySQLdb.constants.ER import NO_SUCH_TABLE
from celery import states as CeleryStates

from django.db.utils import ProgrammingError
from django.test import TransactionTestCase
from django.contrib.contenttypes.models import ContentType
from django.contrib.auth.models import Permission as ModelLevelPermission

from ecommerce_common.util import flatten_nested_iterable

from user_management.models.auth import Role, LoginAccount
from user_management.models.base import (
    QuotaMaterial,
    GenericUserProfile,
    GenericUserGroup,
    GenericUserGroupClosure,
)
from user_management.serializers.common import (
    serialize_profile_quota,
    serialize_profile_permissions,
)
from user_management.async_tasks import get_profile, profile_descendant_validity

from .common import (
    _fixtures,
    _setup_login_account,
    UserNestedFieldSetupMixin,
    UserNestedFieldVerificationMixin,
)


class GetProfileCase(
    TransactionTestCase, UserNestedFieldSetupMixin, UserNestedFieldVerificationMixin
):
    num_profiles = 8

    _permission_info = {
        "store": ["storeprofile", "storestaff", "storeproductavail"],
        "order": ["orderinvoice", "orderreceipt", "orderreturn"],
    }

    def _setup_extra_permission_info(self):
        ct_objs = []
        perm_objs = []
        actions = ("add", "change", "delete", "view")
        for app_label, model_name_list in self._permission_info.items():
            data = map(lambda name: {"model": name, "app_label": app_label}, model_name_list)
            objs = map(lambda d: ContentType.objects.create(**d), data)
            ct_objs.extend(list(objs))
        for action in actions:
            data = map(
                lambda ct: {
                    "codename": "%s_%s" % (action, ct.model),
                    "name": ct.model,
                    "content_type": ct,
                },
                ct_objs,
            )
            objs = map(lambda d: ModelLevelPermission.objects.create(**d), data)
            perm_objs.extend(list(objs))
        return ct_objs, perm_objs

    def _teardown_extra_permission_info(self, ct_objs, perm_objs):
        try:
            tuple(map(lambda obj: obj.delete(), perm_objs))
            tuple(map(lambda obj: obj.delete(), ct_objs))
        except ProgrammingError as e:
            if e.args[0] == NO_SUCH_TABLE and e.args[1].find("auth_group_permissions") > 0:
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
        default_profile_data = _fixtures[GenericUserProfile][: self.num_profiles]
        objs[GenericUserProfile] = list(
            map(lambda d: GenericUserProfile(**d), default_profile_data)
        )
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
            applied_role_data = self._gen_roles(
                role_objs=self._primitives[Role], num=3, serializable=False
            )
            applied_quota_data = self._gen_quota(
                quota_mat_objs=self._primitives[QuotaMaterial],
                num=3,
                serializable=False,
            )
            applied_email_data = self._gen_emails(num=4)
            applied_phone_data = self._gen_phones(num=4)
            for data in applied_role_data:
                data["approved_by"] = self._primitives[GenericUserProfile][0]
            for data in applied_email_data:
                data.pop("id", None)
            for data in applied_phone_data:
                data.pop("id", None)
            # TODO, randomly create login account
            tuple(map(lambda d: profile.roles.create(**d), applied_role_data))
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
            prof_ids = list(map(lambda obj: obj.id, chosen_profiles))
            app_labels = list(self._permission_info.keys())
            chosen_app_label = random.choice(app_labels)
            input_kwargs = {"ids": prof_ids, "fields": ["id", "roles", "quota"]}
            eager_result = get_profile.apply_async(
                kwargs=input_kwargs, headers={"src_app": chosen_app_label}
            )
            self.assertEqual(eager_result.state, CeleryStates.SUCCESS)
            for data in eager_result.result:
                actual_perms = data.get("perms")
                actual_quota = data.get("quota")
                self.assertIsNotNone(actual_perms)
                self.assertIsNotNone(actual_quota)
                expect_profile = next(filter(lambda obj: obj.id == data["id"], chosen_profiles))
                expect_perms = serialize_profile_permissions(
                    expect_profile, app_labels=[chosen_app_label]
                )
                expect_quota = serialize_profile_quota(
                    expect_profile, app_labels=[chosen_app_label]
                )
                actual_perms = sorted(actual_perms, key=lambda d: d["codename"])
                expect_perms = sorted(expect_perms, key=lambda d: d["codename"])
                actual_quota = sorted(actual_quota, key=lambda d: d["mat_code"])
                expect_quota = sorted(expect_quota, key=lambda d: d["mat_code"])
                self.assertListEqual(actual_perms, expect_perms)
                self.assertListEqual(actual_quota, expect_quota)
            # ----------------------------
            input_kwargs = {"ids": prof_ids, "fields": ["id", "emails", "phones"]}
            eager_result = get_profile.apply_async(
                kwargs=input_kwargs, headers={"src_app": chosen_app_label}
            )
            self.assertEqual(eager_result.state, CeleryStates.SUCCESS)
            for data in eager_result.result:
                actual_emails = data.get("emails")
                actual_phones = data.get("phones")
                self.assertIsNotNone(actual_emails)
                self.assertIsNotNone(actual_phones)
                expect_profile = next(filter(lambda obj: obj.id == data["id"], chosen_profiles))
                expect_emails = expect_profile.emails.values("id", "addr")
                expect_phones = expect_profile.phones.values("id", "country_code", "line_number")
                actual_emails = sorted(actual_emails, key=lambda d: d["id"])
                actual_phones = sorted(actual_phones, key=lambda d: d["id"])
                expect_emails = sorted(expect_emails, key=lambda d: d["id"])
                expect_phones = sorted(expect_phones, key=lambda d: d["id"])
                self.assertListEqual(actual_emails, expect_emails)
                self.assertListEqual(actual_phones, expect_phones)
        ## end of loop

    ## end of test_success()

    def test_invalid_id(self):
        chosen_profiles = random.choices(self._primitives[GenericUserProfile], k=3)
        prof_ids = list(map(lambda obj: obj.id, chosen_profiles))
        prof_ids.append("1xx")
        chosen_app_label = "non_existent_service"
        input_kwargs = {"ids": prof_ids, "fields": ["id", "roles", "emails"]}
        eager_result = get_profile.apply_async(
            kwargs=input_kwargs, headers={"src_app": chosen_app_label}
        )
        self.assertEqual(eager_result.state, CeleryStates.FAILURE)
        self.assertTrue(isinstance(eager_result.result, ValueError))
        pos = eager_result.result.args[0].find("1xx")
        self.assertGreater(pos, 0)

    def test_nonexist_app_label(self):
        chosen_profiles = random.choices(self._primitives[GenericUserProfile], k=3)
        prof_ids = list(map(lambda obj: obj.id, chosen_profiles))
        headers = {}
        input_kwargs = {"ids": prof_ids, "fields": ["id", "roles", "quota"]}
        eager_result = get_profile.apply_async(kwargs=input_kwargs, headers=headers)
        self.assertEqual(eager_result.state, CeleryStates.FAILURE)
        self.assertTrue(isinstance(eager_result.result, ValueError))
        self.assertTrue(eager_result.result.args[0].startswith("src_app_label is required"))
        chosen_app_label = "non_existent_service"
        headers = {"src_app": chosen_app_label}
        eager_result = get_profile.apply_async(kwargs=input_kwargs, headers=headers)
        self.assertEqual(eager_result.state, CeleryStates.FAILURE)
        self.assertTrue(isinstance(eager_result.result, ValueError))
        actual_err_msg = eager_result.result.args[0]
        self.assertGreater(actual_err_msg.find("invalid app_label"), 0)
        self.assertGreater(actual_err_msg.find("AppCodeOptions"), 0)
        self.assertGreater(actual_err_msg.find(chosen_app_label), 0)


## from common.util.python.messaging.rpc import RPCproxy
## auth_app_rpc = RPCproxy(dst_app_name='user_management', src_app_name='store')
## reply_evt = auth_app_rpc.get_profile(ids=[2,3,4] , fields=['id', 'roles', 'quota'])
## reply_evt.refresh(retry=False, timeout=0.6, num_of_msgs_fetch=1)
## reply_evt.result['result']


class ProfileDescendantValidityCase(TransactionTestCase):
    def _setup_groups_hierarchy(self, grp_obj_map):
        """
        the tree structure of the group hierarchy in this test case
                   3            10
                 /    \        /  \
                4      5      11  12
               / \    / \    /  \
              6   7  8   9  13   14
        """
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
                "depth": 0,
                "ancestor": grp_obj_map[7],
                "descendant": grp_obj_map[7],
            },
            {
                "id": 6,
                "depth": 0,
                "ancestor": grp_obj_map[8],
                "descendant": grp_obj_map[8],
            },
            {
                "id": 7,
                "depth": 0,
                "ancestor": grp_obj_map[9],
                "descendant": grp_obj_map[9],
            },
            {
                "id": 8,
                "depth": 0,
                "ancestor": grp_obj_map[10],
                "descendant": grp_obj_map[10],
            },
            {
                "id": 9,
                "depth": 0,
                "ancestor": grp_obj_map[11],
                "descendant": grp_obj_map[11],
            },
            {
                "id": 10,
                "depth": 0,
                "ancestor": grp_obj_map[12],
                "descendant": grp_obj_map[12],
            },
            {
                "id": 11,
                "depth": 0,
                "ancestor": grp_obj_map[13],
                "descendant": grp_obj_map[13],
            },
            {
                "id": 12,
                "depth": 0,
                "ancestor": grp_obj_map[14],
                "descendant": grp_obj_map[14],
            },
            {
                "id": 13,
                "depth": 1,
                "ancestor": grp_obj_map[3],
                "descendant": grp_obj_map[4],
            },
            {
                "id": 14,
                "depth": 1,
                "ancestor": grp_obj_map[3],
                "descendant": grp_obj_map[5],
            },
            {
                "id": 15,
                "depth": 1,
                "ancestor": grp_obj_map[4],
                "descendant": grp_obj_map[6],
            },
            {
                "id": 16,
                "depth": 1,
                "ancestor": grp_obj_map[4],
                "descendant": grp_obj_map[7],
            },
            {
                "id": 17,
                "depth": 1,
                "ancestor": grp_obj_map[5],
                "descendant": grp_obj_map[8],
            },
            {
                "id": 18,
                "depth": 1,
                "ancestor": grp_obj_map[5],
                "descendant": grp_obj_map[9],
            },
            {
                "id": 19,
                "depth": 1,
                "ancestor": grp_obj_map[10],
                "descendant": grp_obj_map[11],
            },
            {
                "id": 20,
                "depth": 1,
                "ancestor": grp_obj_map[10],
                "descendant": grp_obj_map[12],
            },
            {
                "id": 21,
                "depth": 1,
                "ancestor": grp_obj_map[11],
                "descendant": grp_obj_map[13],
            },
            {
                "id": 22,
                "depth": 1,
                "ancestor": grp_obj_map[11],
                "descendant": grp_obj_map[14],
            },
            {
                "id": 23,
                "depth": 2,
                "ancestor": grp_obj_map[3],
                "descendant": grp_obj_map[6],
            },
            {
                "id": 24,
                "depth": 2,
                "ancestor": grp_obj_map[3],
                "descendant": grp_obj_map[7],
            },
            {
                "id": 25,
                "depth": 2,
                "ancestor": grp_obj_map[3],
                "descendant": grp_obj_map[8],
            },
            {
                "id": 26,
                "depth": 2,
                "ancestor": grp_obj_map[3],
                "descendant": grp_obj_map[9],
            },
            {
                "id": 27,
                "depth": 2,
                "ancestor": grp_obj_map[10],
                "descendant": grp_obj_map[13],
            },
            {
                "id": 28,
                "depth": 2,
                "ancestor": grp_obj_map[10],
                "descendant": grp_obj_map[14],
            },
        ]
        list(
            map(
                lambda d: GenericUserGroupClosure.objects.create(**d),
                group_closure_data,
            )
        )

    def init_primitive(self):
        objs = {}
        grp_objs = list(map(lambda d: GenericUserGroup(**d), _fixtures[GenericUserGroup]))
        GenericUserGroup.objects.bulk_create(grp_objs)
        grp_obj_map = dict(map(lambda obj: (obj.id, obj), grp_objs))
        objs[GenericUserGroup] = grp_obj_map
        # the profiles that were already created, will be used to create another new profiles
        # through serializer or API endpoint
        self.num_profiles = len(grp_objs)
        default_profile_data = _fixtures[GenericUserProfile][: self.num_profiles]
        objs[GenericUserProfile] = list(
            map(lambda d: GenericUserProfile(**d), default_profile_data)
        )
        GenericUserProfile.objects.bulk_create(objs[GenericUserProfile])
        account_data_iter = iter(_fixtures[LoginAccount])
        for obj in objs[GenericUserProfile]:
            account_data = next(account_data_iter)
            _setup_login_account(account_data, profile_obj=obj)
        return objs

    def setUp(self):
        profile_descendant_validity.app.conf.task_always_eager = True
        self._primitives = self.init_primitive()
        self._setup_groups_hierarchy(grp_obj_map=self._primitives[GenericUserGroup])
        approved_by = self._primitives[GenericUserProfile][0]
        profiles_iter = iter(self._primitives[GenericUserProfile])
        for grp in self._primitives[GenericUserGroup].values():
            prof = next(profiles_iter)
            data = {"group": grp, "approved_by": approved_by}
            prof.groups.create(**data)

    def tearDown(self):
        profile_descendant_validity.app.conf.task_always_eager = False

    def test_ok(self):
        # subcase 1, non-existent profile ID
        input_kwargs = {"asc": -999, "descs": [-997, -996]}
        eager_result = profile_descendant_validity.apply_async(kwargs=input_kwargs)
        self.assertEqual(eager_result.state, CeleryStates.FAILURE)
        self.assertTrue(isinstance(eager_result.result, AssertionError))
        self.assertEqual(eager_result.result.args[0], "invalid profile ID for ancestor")
        # subcase 2, correct profile ID, return descendant
        grp = self._primitives[GenericUserGroup][4]
        grp_2 = self._primitives[GenericUserGroup][11]
        prof = grp.profiles.first().profile
        prof.groups.create(group=grp_2, approved_by=prof)
        all_prof_ids = set(map(lambda obj: obj.id, self._primitives[GenericUserProfile]))
        descs = list(all_prof_ids - {prof.id})
        descs.extend([-995, -994, -993])
        input_kwargs = {"asc": prof.id, "descs": descs}
        eager_result = profile_descendant_validity.apply_async(kwargs=input_kwargs)
        self.assertEqual(eager_result.state, CeleryStates.SUCCESS)
        actual_value = sorted(eager_result.result)
        expect_grp_ids = (4, 6, 7, 11, 13, 14)
        expect_grps = map(lambda gid: self._primitives[GenericUserGroup][gid], expect_grp_ids)
        expect_prof_ids = map(
            lambda g: list(g.profiles.values_list("profile__pk", flat=True)),
            expect_grps,
        )
        expect_prof_ids = flatten_nested_iterable(list_=expect_prof_ids)
        expect_value = list(set(expect_prof_ids) - {prof.id})
        expect_value = sorted(expect_value)
        self.assertListEqual(actual_value, expect_value)
        # subcase 3, correct profile ID, return empty
        input_kwargs = {"asc": prof.id, "descs": [-995, -994, -993]}
        eager_result = profile_descendant_validity.apply_async(kwargs=input_kwargs)
        self.assertEqual(eager_result.state, CeleryStates.SUCCESS)
        self.assertFalse(any(eager_result.result))
