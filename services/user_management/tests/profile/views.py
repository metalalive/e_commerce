import string
import random
import copy
import time

from django.test import TransactionTestCase
from django.contrib.auth.models import Permission
from rest_framework.settings import api_settings as drf_settings

from user_management.models.common import AppCodeOptions
from user_management.models.base import (
    GenericUserProfile,
    GenericUserGroup,
    QuotaMaterial,
)
from user_management.models.auth import Role, LoginAccount
from user_management.serializers import LoginAccountExistField

from ecommerce_common.tests.common import listitem_rand_assigner, rand_gen_request_body
from ecommerce_common.tests.common.django import _BaseMockTestClientInfoMixin

from ..common import (
    _fixtures,
    client_req_csrf_setup,
    AuthenticateUserMixin,
    gen_expiry_time,
    _setup_login_account,
)
from .common import HttpRequestDataGenProfile, ProfileVerificationMixin

non_field_err_key = drf_settings.NON_FIELD_ERRORS_KEY


class BaseViewTestCase(
    TransactionTestCase,
    _BaseMockTestClientInfoMixin,
    AuthenticateUserMixin,
    HttpRequestDataGenProfile,
    ProfileVerificationMixin,
):
    usermgt_material_data = tuple(
        filter(
            lambda d: d["app_code"] == AppCodeOptions.user_management,
            _fixtures[QuotaMaterial],
        )
    )

    def setUp(self):
        self.init_primitive()
        self._setup_keystore()
        self._grp_map = self._setup_groups_hierarchy()
        self._profile = self._primitives[GenericUserProfile][0]
        self._profile_2nd = self._primitives[GenericUserProfile][1]
        self.profile_data_for_test = _fixtures[GenericUserProfile][
            self.num_default_profiles :
        ]
        self._setup_user_roles(
            profile=self._profile,
            approved_by=self._profile_2nd,
            roles=self._primitives[Role][:],
        )
        self._auth_setup(
            testcase=self,
            profile=self._profile,
            is_superuser=False,
            new_account_data=_fixtures[LoginAccount][0].copy(),
        )

    def tearDown(self):
        self._client.cookies.clear()
        self._teardown_keystore()

    def _setup_user_roles(self, profile, approved_by, roles=None):
        roles = roles or []
        role_rel_data = {
            "expiry": gen_expiry_time(minutes_valid=10),
            "approved_by": approved_by,
        }
        tuple(map(lambda role: profile.roles.create(role=role, **role_rel_data), roles))

    def _prepare_access_token(self, new_perms_info=None, chosen_role=None):
        chosen_role = chosen_role or self._primitives[Role][1]
        if new_perms_info:
            qset = Permission.objects.filter(
                content_type__app_label="user_management", codename__in=new_perms_info
            )
            chosen_role.permissions.set(qset)
        else:
            chosen_role.permissions.clear()
        acs_tok_resp = self._refresh_access_token(
            testcase=self, audience=["user_management"]
        )
        return acs_tok_resp["access_token"]


class ProfileCreationTestCase(BaseViewTestCase):
    path = "/profiles"
    num_roles = 2
    num_quota = 3
    num_groups = 3

    def setUp(self):
        super().setUp()
        self.api_call_kwargs = client_req_csrf_setup()
        self.api_call_kwargs.update({"path": self.path, "method": "post"})

    def test_no_permission(self):
        access_token = self._prepare_access_token(
            new_perms_info=["view_role", "view_genericuserprofile"]
        )
        self.api_call_kwargs["headers"]["HTTP_AUTHORIZATION"] = " ".join(
            ["Bearer", access_token]
        )
        response = self._send_request_to_backend(**self.api_call_kwargs)
        self.assertEqual(int(response.status_code), 403)
        err_info = response.json()
        expect_errmsg = "not allowed to perform this action on the profile(s)"
        actual_errmsg = err_info[non_field_err_key][0]
        self.assertEqual(expect_errmsg, actual_errmsg)

    def test_bulk_ok(self):
        num_profiles = 3
        access_token = self._prepare_access_token(
            new_perms_info=["view_genericuserprofile", "add_genericuserprofile"]
        )
        self.api_call_kwargs["headers"]["HTTP_AUTHORIZATION"] = " ".join(
            ["Bearer", access_token]
        )
        profs_data_gen = listitem_rand_assigner(
            list_=self.profile_data_for_test,
            min_num_chosen=num_profiles,
            max_num_chosen=(num_profiles + 1),
        )
        request_data = rand_gen_request_body(
            customize_item_fn=self.customize_req_data_item, data_gen=profs_data_gen
        )
        request_data = list(request_data)
        self.api_call_kwargs.update(
            {"body": request_data, "expect_shown_fields": ["id", "first_name"]}
        )
        response = self._send_request_to_backend(**self.api_call_kwargs)
        self.assertEqual(int(response.status_code), 400)
        err_info = response.json()
        for idx in range(num_profiles):
            for jdx in range(self.num_groups):
                errmsg_pattern = "Current login user does NOT belong to this group"
                actual_errmsg = err_info[idx]["groups"][jdx]["group"][0]
                pos = actual_errmsg.find(errmsg_pattern)
                self.assertGreaterEqual(pos, 0)
        # ----- assume the user-group relation is added later
        top_grps = (self._grp_map[3], self._grp_map[8], self._grp_map[11])
        self._refresh_applied_groups(profile=self._profile, groups=top_grps)
        response = self._send_request_to_backend(**self.api_call_kwargs)
        self.assertEqual(int(response.status_code), 201)
        response_body = response.json()
        new_prof_ids = tuple(map(lambda d: d["id"], response_body))
        actual_instances = GenericUserProfile.objects.filter(id__in=new_prof_ids)
        self.verify_data(actual_data=actual_instances, expect_data=request_data)


class ProfileBaseUpdateTestCase(BaseViewTestCase):
    num_roles = 2
    num_quota = 3
    num_groups = 3
    num_profiles = 3

    def setUp(self):
        super().setUp()
        top_grps = (self._grp_map[3], self._grp_map[8], self._grp_map[11])
        self._refresh_applied_groups(profile=self._profile, groups=top_grps)
        self.request_data = self._init_profiles()

    def _init_profiles(self):
        api_call_kwargs = client_req_csrf_setup()
        access_token = self._prepare_access_token(
            new_perms_info=["view_genericuserprofile", "add_genericuserprofile"]
        )
        api_call_kwargs["headers"]["HTTP_AUTHORIZATION"] = " ".join(
            ["Bearer", access_token]
        )
        profs_data_gen = listitem_rand_assigner(
            list_=self.profile_data_for_test,
            min_num_chosen=self.num_profiles,
            max_num_chosen=(self.num_profiles + 1),
        )
        request_data = rand_gen_request_body(
            customize_item_fn=self.customize_req_data_item, data_gen=profs_data_gen
        )
        request_data = list(request_data)
        api_call_kwargs.update(
            {
                "path": "/profiles",
                "method": "post",
                "body": request_data,
            }
        )
        response = self._send_request_to_backend(**api_call_kwargs)
        self.assertEqual(int(response.status_code), 201)
        self._profile_access_token = access_token
        return response.json()


class ProfileUpdateTestCase(ProfileBaseUpdateTestCase):
    path = "/profiles"

    def setUp(self):
        super().setUp()
        self.api_call_kwargs = client_req_csrf_setup()
        self.api_call_kwargs.update({"path": self.path, "method": "put"})

    def test_no_permission(self):
        self.api_call_kwargs["headers"]["HTTP_AUTHORIZATION"] = " ".join(
            ["Bearer", self._profile_access_token]
        )
        self.api_call_kwargs.update(
            {
                "body": self.request_data[:1],
            }
        )
        response = self._send_request_to_backend(**self.api_call_kwargs)
        self.assertEqual(int(response.status_code), 403)
        err_info = response.json()
        expect_errmsg = "not allowed to perform this action on the profile(s)"
        actual_errmsg = err_info[non_field_err_key][0]
        self.assertEqual(expect_errmsg, actual_errmsg)

    def test_bulk_ok(self):
        access_token = self._prepare_access_token(
            new_perms_info=["view_genericuserprofile", "change_genericuserprofile"]
        )
        self.api_call_kwargs["headers"]["HTTP_AUTHORIZATION"] = " ".join(
            ["Bearer", access_token]
        )
        req_data = self.request_data
        for req_data_item in req_data:
            req_data_item["last_name"] = "".join(
                random.choices(string.ascii_letters, k=6)
            )
            # --- group ---
            applied_grps = tuple(map(lambda d: d["group"], req_data_item["groups"]))
            available_grps = filter(
                lambda obj: obj.id not in applied_grps,
                self._primitives[GenericUserGroup],
            )
            new_grp = next(available_grps)
            req_data_item["groups"][-1]["group"] = new_grp.id
        self.api_call_kwargs.update({"body": req_data})
        response = self._send_request_to_backend(**self.api_call_kwargs)
        self.assertEqual(int(response.status_code), 200)
        new_prof_ids = tuple(map(lambda d: d["id"], req_data))
        edited_profile_objs = GenericUserProfile.objects.filter(id__in=new_prof_ids)
        self.verify_data(actual_data=edited_profile_objs, expect_data=req_data)

    def test_auth_user_edit_own_profile(self):
        access_token = self._prepare_access_token(
            new_perms_info=["view_genericuserprofile", "change_genericuserprofile"]
        )
        self.api_call_kwargs["headers"]["HTTP_AUTHORIZATION"] = " ".join(
            ["Bearer", access_token]
        )
        req_data = self.request_data
        for req_data_item in req_data:
            req_data_item["last_name"] = "".join(
                random.choices(string.ascii_letters, k=6)
            )
            req_data_item["roles"][0]["expiry"] = gen_expiry_time(
                minutes_valid=random.randrange(23, 29)
            )
        origin_profile_data = self._load_profiles_from_instances(objs=[self._profile])
        origin_profile_data = origin_profile_data[0]
        origin_profile_data["last_name"] = "WillChange"
        edit_profile_data = copy.deepcopy(origin_profile_data)
        edit_profile_data["roles"].clear()
        edit_profile_data["quota"].clear()
        edit_profile_data["groups"].clear()
        req_data = [req_data[0], edit_profile_data, req_data[1]]
        self.api_call_kwargs.update({"body": req_data})
        response = self._send_request_to_backend(**self.api_call_kwargs)
        self.assertEqual(int(response.status_code), 200)
        # all the changes on other profiles are committed
        new_prof_ids = [req_data[0]["id"], req_data[2]["id"]]
        edited_profile_objs = GenericUserProfile.objects.filter(id__in=new_prof_ids)
        expect_req_data = [req_data[0], req_data[2]]
        self.verify_data(actual_data=edited_profile_objs, expect_data=expect_req_data)
        # the changes on privilege fields (e.g. roles, quota, groups) of the user profiles are NOT
        # committed before each user is NOT allowed to edit their own privilege nested fields
        self._profile.refresh_from_db()
        self.verify_data(actual_data=[self._profile], expect_data=[origin_profile_data])

    def test_unauth_user_edit_own_profile(self):
        self.api_call_kwargs["headers"]["HTTP_AUTHORIZATION"] = " ".join(
            ["Bearer", self._profile_access_token]
        )
        origin_profile_data = self._load_profiles_from_instances(objs=[self._profile])
        origin_profile_data = origin_profile_data[0]
        origin_profile_data["first_name"] = "Something"
        origin_profile_data["last_name"] = "WillChange"
        edit_profile_data = copy.deepcopy(origin_profile_data)
        edit_profile_data["roles"].clear()
        edit_profile_data["quota"].clear()
        edit_profile_data["groups"].clear()
        req_data = [edit_profile_data]
        self.api_call_kwargs.update({"body": req_data})
        response = self._send_request_to_backend(**self.api_call_kwargs)
        self.assertEqual(int(response.status_code), 200)
        self._profile.refresh_from_db()
        self.verify_data(actual_data=[self._profile], expect_data=[origin_profile_data])


class ProfileDeletionTestCase(ProfileBaseUpdateTestCase):
    path = "/profiles"
    num_profiles = 10

    def setUp(self):
        super().setUp()
        self.api_call_kwargs = client_req_csrf_setup()
        self.api_call_kwargs.update({"path": self.path, "method": "delete"})

    def test_no_permission(self):
        self.api_call_kwargs["headers"]["HTTP_AUTHORIZATION"] = " ".join(
            ["Bearer", self._profile_access_token]
        )
        body = list(map(lambda d: {"id": d["id"]}, self.request_data))
        self.api_call_kwargs.update(
            {
                "body": body,
            }
        )
        response = self._send_request_to_backend(**self.api_call_kwargs)
        self.assertEqual(int(response.status_code), 403)
        err_info = response.json()
        expect_errmsg = "not allowed to perform this action on the profile(s)"
        actual_errmsg = err_info[non_field_err_key][0]
        self.assertEqual(expect_errmsg, actual_errmsg)

    def _value_compare_fn(self, val_a, val_b):
        fields_eq = {}
        fields_eq["id"] = val_a["id"] == val_b["id"]
        fields_eq["first_name"] = val_a["first_name"] == val_b["first_name"]
        fields_eq["last_name"] = val_a["last_name"] == val_b["last_name"]
        fields_eq["groups"] = self._value_compare_groups_fn(val_a=val_a, val_b=val_b)
        return fields_eq

    def _setup_another_login_account(self, account_data, profile, roles, new_perms):
        account_data.update({"is_active": True, "is_staff": True})
        _setup_login_account(
            account_data=account_data, profile_obj=profile, roles=roles, expiry=None
        )
        top_grps = (self._grp_map[3], self._grp_map[8], self._grp_map[11])
        self._refresh_applied_groups(profile=profile, groups=top_grps)
        role_rel = profile.roles.first()
        role_rel.role.permissions.set(new_perms)

    def test_bulk_ok(self):
        new_perms_info = (
            "view_genericuserprofile",
            "change_genericuserprofile",
            "delete_genericuserprofile",
        )
        perms_qset = Permission.objects.filter(
            content_type__app_label="user_management", codename__in=new_perms_info
        )
        role_rel = self._profile.roles.first()
        role_rel.role.permissions.set(perms_qset)
        self._setup_another_login_account(
            account_data=_fixtures[LoginAccount][1],
            profile=self._profile_2nd,
            roles=self._primitives[Role][:3],
            new_perms=perms_qset,
        )
        # ---------------------------------
        behavior_sequence = [
            {
                "profile": self._profile,
                "req_data": self.request_data[0:2],
                "login_password": _fixtures[LoginAccount][0]["password"],
            },
            {
                "profile": self._profile_2nd,
                "req_data": self.request_data[2:4],
                "login_password": _fixtures[LoginAccount][1]["password"],
            },
            {
                "profile": self._profile,
                "req_data": self.request_data[4:6],
                "login_password": _fixtures[LoginAccount][0]["password"],
            },
            {
                "profile": self._profile_2nd,
                "req_data": self.request_data[6:8],
                "login_password": _fixtures[LoginAccount][1]["password"],
            },
        ]
        for del_kwargs in behavior_sequence:
            self._single_softdel_operation(**del_kwargs)
        for del_kwargs in behavior_sequence:
            self._single_undel_operation(**del_kwargs)

    def _single_softdel_operation(
        self, profile, req_data, login_password, delay_interval_sec=2
    ):
        self._client.cookies.clear()
        self._auth_setup(testcase=self, profile=profile, login_password=login_password)
        acs_tok_resp = self._refresh_access_token(
            testcase=self, audience=["user_management"]
        )
        access_token = acs_tok_resp["access_token"]
        self.api_call_kwargs["headers"]["HTTP_AUTHORIZATION"] = " ".join(
            ["Bearer", access_token]
        )
        body = list(map(lambda d: {"id": d["id"]}, req_data))
        self.api_call_kwargs.update(
            {
                "method": "delete",
                "body": body,
            }
        )
        response = self._send_request_to_backend(**self.api_call_kwargs)
        self.assertEqual(int(response.status_code), 202)
        deleted_prof_ids = list(map(lambda d: d["id"], req_data))
        qset = GenericUserProfile.objects.filter(id__in=deleted_prof_ids)
        self.assertFalse(qset.exists())
        qset = GenericUserProfile.objects.get_deleted_set().filter(
            id__in=deleted_prof_ids
        )
        self.assertSetEqual(
            set(qset.values_list("id", flat=True)), set(deleted_prof_ids)
        )
        time.sleep(delay_interval_sec)
        return response

    def _single_undel_operation(self, profile, req_data, login_password):
        self._client.cookies.clear()
        self._auth_setup(testcase=self, profile=profile, login_password=login_password)
        acs_tok_resp = self._refresh_access_token(
            testcase=self, audience=["user_management"]
        )
        access_token = acs_tok_resp["access_token"]
        self.api_call_kwargs["headers"]["HTTP_AUTHORIZATION"] = " ".join(
            ["Bearer", access_token]
        )
        body = {"ids": list(map(lambda d: d["id"], req_data))}
        self.api_call_kwargs.update(
            {
                "method": "patch",
                "body": body,
                "expect_shown_fields": ["id", "first_name", "last_name", "groups"],
            }
        )
        response = self._send_request_to_backend(**self.api_call_kwargs)
        self.assertEqual(int(response.status_code), 200)
        actual_items = response.json()
        expect_items = sorted(req_data, key=lambda d: d["id"])
        actual_items = sorted(actual_items["affected_items"], key=lambda d: d["id"])
        expect_items_iter = iter(expect_items)
        for actual_item in actual_items:
            expect_item = next(expect_items_iter)
            compare_result = self._value_compare_fn(
                val_a=actual_item, val_b=expect_item
            )
            self.assertTrue(compare_result)
        return response

    def test_self_removal(self):
        # current user attempts to delete his/her own login account
        origin_profiles_data = self._load_profiles_from_instances(objs=[self._profile])
        response = self._single_softdel_operation(
            profile=self._profile,
            req_data=origin_profiles_data,
            login_password=_fixtures[LoginAccount][0]["password"],
            delay_interval_sec=0,
        )
        result = response.json()
        self.assertEqual(result["message"], "force logout")
        error_caught = None
        with self.assertRaises(AssertionError):
            try:  # current user deleted his/her own profile and account, unable to login
                self._client.cookies.clear()
                self._auth_setup(
                    testcase=self,
                    profile=self._profile,
                    login_password=_fixtures[LoginAccount][0]["password"],
                )
            except AssertionError as e:
                error_caught = e
                raise
        self.assertIsNotNone(error_caught)
        self.assertEqual("401 != 200", error_caught.args[0])
        # recover the profile by the other user who has recover permission at the same or parent group
        new_perms_info = (
            "view_genericuserprofile",
            "change_genericuserprofile",
        )
        perms_qset = Permission.objects.filter(
            content_type__app_label="user_management", codename__in=new_perms_info
        )
        self._setup_another_login_account(
            account_data=_fixtures[LoginAccount][1],
            profile=self._profile_2nd,
            roles=self._primitives[Role][:4],
            new_perms=perms_qset,
        )
        self._single_undel_operation(
            profile=self._profile_2nd,
            req_data=origin_profiles_data,
            login_password=_fixtures[LoginAccount][1]["password"],
        )


## end of class ProfileDeletionTestCase


class ProfileQueryTestCase(ProfileBaseUpdateTestCase):
    paths = ["/profiles", "/profile/%s"]
    num_roles = 2
    num_quota = 4
    num_profiles = 4
    num_groups = 2

    def setUp(self):
        super().setUp()
        self.api_call_kwargs = client_req_csrf_setup()
        self.hidden_fields = ("first_name", "last_name", "roles", "phones", "quota")
        self.expect_fields = ("id", "groups", "emails", "locations", "auth")
        self.api_call_kwargs.update({"expect_shown_fields": self.expect_fields})
        # the user does NOT have `view_genericuserprofile` permission and not belong to
        # the same group as the same profile does
        self._profile.groups.filter(group__id=3).delete(hard=True)
        access_token = self._prepare_access_token(new_perms_info=["view_role"])
        self.api_call_kwargs["headers"]["HTTP_AUTHORIZATION"] = " ".join(
            ["Bearer", access_token]
        )
        # set up account status
        profile_ids = list(map(lambda d: d["id"], self.request_data))
        profiles = GenericUserProfile.objects.filter(id__in=profile_ids)
        _info = [
            (profiles[idx], _fixtures[LoginAccount][2 + idx])
            for idx in range(1, len(profile_ids))
        ]
        for profile, account_data in _info:
            _setup_login_account(account_data=account_data, profile_obj=profile)
        last_account = profiles.last().account
        last_account.is_active = True
        last_account.save(update_fields=["is_active"])
        self._test_profiles = profiles

    def _gen_quota(self, num=None):
        contact_quota_maxnum = 2
        self.num_locations = contact_quota_maxnum
        self.num_emails = contact_quota_maxnum
        self.num_phones = contact_quota_maxnum
        other_apps_material_data = filter(
            lambda d: d["app_code"] != AppCodeOptions.user_management,
            _fixtures[QuotaMaterial],
        )
        other_apps_material_data = next(other_apps_material_data)
        out = list(
            map(
                lambda d: {
                    "expiry": gen_expiry_time(),
                    "material": d["id"],
                    "maxnum": contact_quota_maxnum,
                },
                self.usermgt_material_data,
            )
        )
        out.append(
            {
                "expiry": gen_expiry_time(),
                "maxnum": random.randrange(3, 30),
                "material": other_apps_material_data["id"],
            }
        )
        return out

    def _value_compare_fn(self, val_a, val_b):
        for field in self.hidden_fields:
            with self.assertRaises(KeyError):
                val_a[field]
        fields_eq = {}
        fields_eq["id"] = val_a["id"] == val_b["id"]
        fields_eq["groups"] = self._value_compare_groups_fn(val_a=val_a, val_b=val_b)
        for k in ("emails", "locations"):
            fields_eq[k] = self._value_compare_contact_fn(
                val_a=val_a[k],
                val_b=val_b[k],
                _fields_compare=self._nested_field_names[k],
            )
        self.assertNotIn(False, fields_eq.values())

    def test_single_item_partial(self):
        chosen_profile_data = self.request_data[-1]
        chosen_profile = self._test_profiles.get(id=chosen_profile_data["id"])
        path = self.paths[1] % chosen_profile_data["id"]
        self.api_call_kwargs.update({"path": path, "method": "get"})
        response = self._send_request_to_backend(**self.api_call_kwargs)
        self.assertEqual(int(response.status_code), 200)
        fetch_profile_data = response.json()
        self.verify_data(actual_data=[chosen_profile], expect_data=[fetch_profile_data])
        self.assertEqual(
            fetch_profile_data["auth"],
            LoginAccountExistField._activation_status.ACCOUNT_ACTIVATED.value,
        )

    def test_multiple_items_partial(self):
        profile_ids = list(map(lambda d: d["id"], self.request_data))
        self.api_call_kwargs.update(
            {"path": self.paths[0], "method": "get", "ids": profile_ids}
        )
        response = self._send_request_to_backend(**self.api_call_kwargs)
        self.assertEqual(int(response.status_code), 200)
        fetch_profiles_data = response.json()
        self.verify_data(
            actual_data=self._test_profiles, expect_data=fetch_profiles_data
        )
        self.assertEqual(
            fetch_profiles_data[0]["auth"],
            LoginAccountExistField._activation_status.ACCOUNT_NON_EXISTENT.value,
        )
        self.assertEqual(
            fetch_profiles_data[1]["auth"],
            LoginAccountExistField._activation_status.ACCOUNT_DEACTIVATED.value,
        )
        self.assertEqual(
            fetch_profiles_data[2]["auth"],
            LoginAccountExistField._activation_status.ACCOUNT_DEACTIVATED.value,
        )
        self.assertEqual(
            fetch_profiles_data[3]["auth"],
            LoginAccountExistField._activation_status.ACCOUNT_ACTIVATED.value,
        )


class ProfileSearchTestCase(ProfileBaseUpdateTestCase):
    path = "/profiles"
    num_roles = 2
    num_quota = 3
    num_profiles = 3
    num_groups = 2

    def setUp(self):
        super().setUp()
        self.api_call_kwargs = client_req_csrf_setup()
        # the user does NOT have `view_genericuserprofile` permission
        access_token = self._prepare_access_token(new_perms_info=["view_role"])
        self.api_call_kwargs["headers"]["HTTP_AUTHORIZATION"] = " ".join(
            ["Bearer", access_token]
        )
        self.api_call_kwargs.update({"path": self.path, "method": "get"})

    def _gen_quota(self, num=None):
        contact_quota_maxnum = 2
        self.num_locations = contact_quota_maxnum
        self.num_emails = contact_quota_maxnum
        self.num_phones = contact_quota_maxnum
        out = list(
            map(
                lambda d: {
                    "expiry": gen_expiry_time(),
                    "material": d["id"],
                    "maxnum": contact_quota_maxnum,
                },
                self.usermgt_material_data,
            )
        )
        return out

    def test_search(self):
        self.api_call_kwargs.update({"extra_query_params": {}})
        # -----------------
        keyword = self.request_data[-1]["last_name"]
        self.api_call_kwargs["extra_query_params"]["search"] = keyword
        response = self._send_request_to_backend(**self.api_call_kwargs)
        self.assertEqual(int(response.status_code), 200)
        fetch_profiles_data = response.json()
        self.assertTrue(any(fetch_profiles_data))
        pos = fetch_profiles_data[0]["last_name"].find(keyword)
        self.assertGreaterEqual(pos, 0)
        # -----------------
        keyword = self.request_data[1]["emails"][0]["addr"]
        self.api_call_kwargs["extra_query_params"]["search"] = keyword
        response = self._send_request_to_backend(**self.api_call_kwargs)
        self.assertEqual(int(response.status_code), 200)
        fetch_profiles_data = response.json()
        self.assertTrue(any(fetch_profiles_data))
        self.assertTrue(any(fetch_profiles_data[0]["emails"]))
        found = False
        for fetch_profile_data in fetch_profiles_data:
            for email_data in fetch_profile_data["emails"]:
                pos = email_data["addr"].find(keyword)
                if pos >= 0:
                    found = True
                    break
        self.assertTrue(found)
        # -----------------
        keyword = self.request_data[-1]["locations"][-1]["locality"]
        self.api_call_kwargs["extra_query_params"]["search"] = keyword
        response = self._send_request_to_backend(**self.api_call_kwargs)
        self.assertEqual(int(response.status_code), 200)
        fetch_profiles_data = response.json()
        self.assertTrue(any(fetch_profiles_data))
        self.assertTrue(any(fetch_profiles_data[0]["locations"]))
        found = False
        for fetch_profile_data in fetch_profiles_data:
            for geoloc_data in fetch_profile_data["locations"]:
                pos = geoloc_data["locality"].find(keyword)
                if pos >= 0:
                    found = True
                    break
        self.assertTrue(found)
        # ----------------- search user profiles that belong to given group name
        chosen_grp_id = self.request_data[0]["groups"][0]["group"]
        chosen_group = next(
            filter(
                lambda obj: obj.id == chosen_grp_id, self._primitives[GenericUserGroup]
            )
        )
        keyword = chosen_group.name
        self.api_call_kwargs["extra_query_params"]["search"] = keyword
        response = self._send_request_to_backend(**self.api_call_kwargs)
        self.assertEqual(int(response.status_code), 200)
        fetch_profiles_data = response.json()
        self.assertTrue(any(fetch_profiles_data))
        self.assertTrue(any(fetch_profiles_data[0]["groups"]))
        found = False
        for fetch_profile_data in fetch_profiles_data:
            for grp_data in fetch_profile_data["groups"]:
                if grp_data["group"] == chosen_grp_id:
                    found = True
                    break
        self.assertTrue(found)
