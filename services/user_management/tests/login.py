import random
import json
from datetime import timedelta
from unittest.mock import patch

from django.test import TransactionTestCase
from django.conf import settings as django_settings
from django.utils import timezone as django_timezone
from django.contrib.contenttypes.models import ContentType
from django.contrib.auth.models import Permission as ModelLevelPermission
from rest_framework.settings import api_settings as drf_settings
from jwt.exceptions import MissingRequiredClaimError, InvalidAudienceError
from jwt.api_jwk import PyJWK

from ecommerce_common.auth.jwt import JWT
from ecommerce_common.cors.middleware import (
    conf as cors_cfg,
    ACCESS_CONTROL_REQUEST_METHOD,
    ACCESS_CONTROL_REQUEST_HEADERS,
    ACCESS_CONTROL_ALLOW_ORIGIN,
    ACCESS_CONTROL_ALLOW_METHODS,
    ACCESS_CONTROL_ALLOW_HEADERS,
    ACCESS_CONTROL_ALLOW_CREDENTIALS,
    ACCESS_CONTROL_MAX_AGE,
)
from ecommerce_common.util import sort_nested_object, import_module_string
from ecommerce_common.models.constants import ROLE_ID_SUPERUSER, ROLE_ID_STAFF

from user_management.models.common import AppCodeOptions
from user_management.models.base import (
    GenericUserProfile,
    GenericUserAppliedRole,
    QuotaMaterial,
    UserQuotaRelation,
)
from user_management.models.auth import LoginAccount, Role

from ecommerce_common.tests.common import HttpRequestDataGen, KeystoreMixin
from ecommerce_common.tests.common.django import _BaseMockTestClientInfoMixin
from tests.common import _fixtures, client_req_csrf_setup, AuthenticateUserMixin


non_fd_err_key = drf_settings.NON_FIELD_ERRORS_KEY
_expiry_time = django_timezone.now() + timedelta(minutes=5)


class LoginTestCase(TransactionTestCase, _BaseMockTestClientInfoMixin, KeystoreMixin):
    _keystore_init_config = django_settings.AUTH_KEYSTORE
    path = "/login"
    expect_err_msg = "authentication failure"

    def setUp(self):
        self._setup_keystore()
        self.api_call_kwargs = client_req_csrf_setup()
        self.api_call_kwargs["path"] = self.path
        self.api_call_kwargs["method"] = "post"

    def tearDown(self):
        self._client.cookies.clear()
        self._teardown_keystore()

    def test_auth_failure(self):
        profile_data = [
            {"id": 3, "first_name": "Ian", "last_name": "Crutis"},
            {"id": 4, "first_name": "Ham", "last_name": "Simpson"},
            {"id": 5, "first_name": "Ben", "last_name": "Troy"},
        ]
        profiles = tuple(
            map(lambda d: GenericUserProfile.objects.create(**d), profile_data)
        )
        account_data = [
            # active guest
            {
                "username": "ImGuest",
                "password": "hard2guess",
                "is_active": True,
                "is_staff": False,
                "is_superuser": False,
            },
            # inactive staff
            {
                "username": "ImStaff",
                "password": "D0ntexpose",
                "is_active": False,
                "is_staff": True,
                "is_superuser": False,
            },
            # inactive superuser
            {
                "username": "ImSuperUsr",
                "password": "cautious",
                "is_active": False,
                "is_staff": True,
                "is_superuser": True,
            },
        ]
        acc_data_iter = iter(account_data)
        for p in profiles:
            acc_data = next(acc_data_iter)
            acc_data["profile"] = p
            acc_data["password_last_updated"] = django_timezone.now()
        accounts = tuple(
            map(lambda d: LoginAccount.objects.create_user(**d), account_data)
        )
        bodies = [
            None,
            {},
            {"password": "pig1234"},
            {"username": "shrimp_noodle"},
            {"username": "thisuser", "password": "doesnotexist"},
        ]
        bodies.extend(
            tuple(
                map(
                    lambda d: {"username": d["username"], "password": d["password"]},
                    account_data,
                )
            )
        )
        for body in bodies:
            self.api_call_kwargs["body"] = body
            response = self._send_request_to_backend(**self.api_call_kwargs)
            self.assertEqual(int(response.status_code), 401)
            err_info = response.json()
            self.assertEqual(err_info[non_fd_err_key][0], self.expect_err_msg)

    ## end of test_auth_failure()

    def test_ok(self):
        profile_data = {"id": 4, "first_name": "Mathihi", "last_name": "Raj"}
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
        self.api_call_kwargs["body"] = {"username": "ImStaff", "password": "dontexpose"}
        # first login request
        response = self._send_request_to_backend(**self.api_call_kwargs)
        self.assertEqual(int(response.status_code), 200)
        csrf_token = response.cookies.get(django_settings.CSRF_COOKIE_NAME, None)
        refresh_jwt = response.cookies.get(django_settings.JWT_NAME_REFRESH_TOKEN, None)
        self.assertIsNotNone(csrf_token)  # response.cookies.keys()
        self.assertIsNotNone(refresh_jwt)
        expire_time_1 = refresh_jwt["expires"].split()[:4]
        expire_time_2 = csrf_token["expires"].split()[:4]
        self.assertListEqual(expire_time_1, expire_time_2)
        payld_verified = self._verify_recv_jwt(encoded=refresh_jwt.value)
        self.assertEqual(int(payld_verified["profile"]), profile_data["id"])
        # assume frontend accidentally sends the same request again in a few milliseconds
        response_2nd = self._send_request_to_backend(**self.api_call_kwargs)
        self.assertEqual(int(response_2nd.status_code), 200)
        expect_token = response.cookies.get(
            django_settings.JWT_NAME_REFRESH_TOKEN, None
        ).value
        actual_token = self._client.cookies.get(
            django_settings.JWT_NAME_REFRESH_TOKEN, None
        ).value
        self.assertEqual(expect_token, actual_token)
        self.assertIsNone(
            response_2nd.cookies.get(django_settings.JWT_NAME_REFRESH_TOKEN, None)
        )

    def _verify_recv_jwt(self, encoded):
        _jwt = JWT(encoded=encoded)
        self.assertEqual(_jwt.header["typ"], "JWT")
        self.assertIn(_jwt.header["alg"], ("RS256", "RS384", "RS512"))
        payld_unverified = _jwt.payload
        with self.assertRaises(MissingRequiredClaimError):
            payld_verified = _jwt.verify(
                keystore=self._keystore,
                audience=["sale", "payment"],
                raise_if_failed=True,
            )
        payld_verified = _jwt.verify(
            keystore=self._keystore, audience=None, raise_if_failed=True
        )
        self.assertNotEqual(payld_verified, None)
        self.assertIsNone(
            payld_verified.get("aud")
        )  # refresh token doesn't include `aud` field
        return payld_verified


## end of class LoginTestCase


class RefreshAccessTokenTestCase(
    TransactionTestCase, _BaseMockTestClientInfoMixin, AuthenticateUserMixin
):
    path = "/refresh_access_token"

    def _setup_role(self, profile, approved_by):
        role_data = [
            {
                "id": ROLE_ID_STAFF,
                "name": "base staff",
            },
            {
                "id": 4,
                "name": "my role on auth",
            },
            {
                "id": 5,
                "name": "my role on usrmgt",
            },
        ]
        app_labels = (
            "auth",
            "user_management",
        )
        roles = tuple(map(lambda d: Role.objects.create(**d), role_data))
        roles_iter = iter(roles[1:])
        for app_label in app_labels:
            qset = ModelLevelPermission.objects.filter(
                content_type__app_label=app_label
            )
            role = next(roles_iter)
            role.permissions.set(qset[:3])
        for role in roles:
            data = {"expiry": _expiry_time, "approved_by": approved_by, "role": role}
            applied_role = GenericUserAppliedRole(**data)
            self._profile.roles.add(applied_role, bulk=False)
        self.applied_perm_map = {
            AppCodeOptions.user_management.value: list(
                roles[2].permissions.values_list("codename", flat=True)
            )
        }
        return role_data

    def _setup_quota_mat(self, profile):
        appcodes = AppCodeOptions
        material_data = [
            {"id": 2, "app_code": appcodes.user_management.value, "mat_code": 2},
            {"id": 3, "app_code": appcodes.user_management.value, "mat_code": 1},
            {"id": 4, "app_code": appcodes.product.value, "mat_code": 2},
            {"id": 5, "app_code": appcodes.product.value, "mat_code": 1},
            {"id": 6, "app_code": appcodes.media.value, "mat_code": 1},
        ]
        mat_objs = tuple(map(lambda d: QuotaMaterial(**d), material_data))
        QuotaMaterial.objects.bulk_create(mat_objs)
        for mat_obj in mat_objs:
            data = {"material": mat_obj, "maxnum": random.randrange(11, 25)}
            applied_quota = UserQuotaRelation(**data)
            self._profile.quota.add(applied_quota, bulk=False)
        return material_data

    def setUp(self):
        self._setup_keystore()
        self._profile, _ = self._auth_setup(testcase=self)
        self.api_call_kwargs = client_req_csrf_setup()
        self.api_call_kwargs.update({"path": self.path, "method": "get"})
        profile_2nd_data = {"id": 4, "first_name": "Texassal", "last_name": "Bovaski"}
        profile_2nd = GenericUserProfile.objects.create(**profile_2nd_data)
        self._role_data = self._setup_role(
            profile=self._profile, approved_by=profile_2nd
        )
        self._quota_mat_data = self._setup_quota_mat(profile=self._profile)

    def tearDown(self):
        self._client.cookies.clear()
        self._teardown_keystore()

    def test_ok(self):
        expect_audience = ["user_management"]
        self.api_call_kwargs["extra_query_params"] = {
            "audience": ",".join(expect_audience)
        }
        response = self._send_request_to_backend(**self.api_call_kwargs)
        self.assertEqual(int(response.status_code), 200)
        result = response.json()
        expect_jwks_url = "%s/jwks" % (cors_cfg.ALLOWED_ORIGIN["user_management"])
        actual_jwks_url = result["jwks_url"]
        self.assertEqual(expect_jwks_url, actual_jwks_url)
        actual_token = result.get("access_token")
        self.assertIsNotNone(actual_token)
        payld_verified = self._verify_recv_jwt(
            actual_token, expect_audience=expect_audience
        )
        self.assertEqual(self._profile.id, payld_verified["profile"])
        self.assertEqual(ROLE_ID_STAFF, payld_verified["priv_status"])
        self.assertListEqual(expect_audience, payld_verified["aud"])
        # validate expiration time
        expect_valid_period = django_settings.JWT_ACCESS_TOKEN_VALID_PERIOD
        actual_valid_period = payld_verified["exp"] - payld_verified["iat"]
        self.assertGreaterEqual(expect_valid_period, actual_valid_period)
        # validate permissions data
        usrmgt_appcode = AppCodeOptions.user_management.value
        expect_perm_data = self.applied_perm_map[usrmgt_appcode]
        actual_perm_data = filter(
            lambda d: d["app_code"] == usrmgt_appcode, payld_verified["perms"]
        )
        actual_perm_data = list(map(lambda d: d["codename"], actual_perm_data))
        self.assertSetEqual(set(expect_perm_data), set(actual_perm_data))
        # validate quota data
        usrmgt_appcode = getattr(AppCodeOptions, expect_audience[0]).value
        qset = self._profile.quota.filter(material__app_code=usrmgt_appcode)
        qset = qset.values_list(
            "material__app_code",
            "material__mat_code",
            "maxnum",
        )
        expect_quota_data = list(
            map(lambda v: {"app_code": v[0], "mat_code": v[1], "maxnum": v[2]}, qset)
        )
        actual_quota_data = payld_verified["quota"]
        expect_quota_data = sort_nested_object(expect_quota_data)
        actual_quota_data = sort_nested_object(actual_quota_data)
        expect_quota_data = json.dumps(expect_quota_data)
        actual_quota_data = json.dumps(actual_quota_data)
        self.assertEqual(expect_quota_data, actual_quota_data)

    def test_missing_refresh_token(self):
        self._client.cookies.pop(django_settings.JWT_NAME_REFRESH_TOKEN, None)
        expect_audience = ["user_management"]
        self.api_call_kwargs["extra_query_params"] = {
            "audience": ",".join(expect_audience)
        }
        response = self._send_request_to_backend(**self.api_call_kwargs)
        self.assertEqual(int(response.status_code), 403)

    def test_invalid_app_labels(self):
        expect_audience = ["non_existent_app_1", "non_existent_app_2"]
        self.api_call_kwargs["extra_query_params"] = {
            "audience": ",".join(expect_audience)
        }
        response = self._send_request_to_backend(**self.api_call_kwargs)
        self.assertEqual(int(response.status_code), 400)
        result = response.json()
        self.assertEqual(result[non_fd_err_key][0], "invalid audience field")

    def test_app_access_denied(self):
        # delete staff role, then there shouldn't be any role existing in the response below
        self._profile.roles.filter(role__id=ROLE_ID_STAFF).delete(hard=True)
        expect_audience = ["non_existent_app_1", "media"]
        self.api_call_kwargs["extra_query_params"] = {
            "audience": ",".join(expect_audience)
        }
        response = self._send_request_to_backend(**self.api_call_kwargs)
        result = response.json()
        self.assertEqual(int(response.status_code), 403)
        self.assertEqual(
            result[non_fd_err_key][0],
            "the user does not have access to these resource services listed in audience field",
        )

    def _verify_recv_jwt(self, encoded, expect_audience):
        _jwt = JWT(encoded=encoded)
        self.assertEqual(_jwt.header["typ"], "JWT")
        self.assertIn(_jwt.header["alg"], ("RS256", "RS384", "RS512"))
        payld_unverified = _jwt.payload
        with self.assertRaises(InvalidAudienceError):
            payld_verified = _jwt.verify(
                keystore=self._keystore,
                audience=["delivery", "payment"],
                raise_if_failed=True,
            )
        payld_verified = _jwt.verify(
            keystore=self._keystore, audience=expect_audience, raise_if_failed=True
        )
        self.assertNotEqual(payld_verified, None)
        self.assertIsNotNone(
            payld_verified.get("aud")
        )  # access token should include `aud` field
        return payld_verified


## end of class RefreshAccessTokenTestCase


class LogoutTestCase(
    TransactionTestCase, _BaseMockTestClientInfoMixin, AuthenticateUserMixin
):
    path = "/logout"

    def setUp(self):
        self._setup_keystore()
        self._profile, _ = self._auth_setup(testcase=self)
        self.api_call_kwargs = client_req_csrf_setup()
        self.api_call_kwargs.update({"path": self.path, "method": "post"})

    def tearDown(self):
        self._client.cookies.clear()
        self._teardown_keystore()

    def test_ok(self):
        response = self._send_request_to_backend(**self.api_call_kwargs)
        self.assertEqual(int(response.status_code), 200)
        response = self._send_request_to_backend(**self.api_call_kwargs)
        self.assertEqual(int(response.status_code), 403)


class JwksPublicKeyTestCase(
    TransactionTestCase, _BaseMockTestClientInfoMixin, KeystoreMixin
):
    _keystore_init_config = django_settings.AUTH_KEYSTORE
    path = "/jwks"

    def setUp(self):
        self._setup_keystore()

    def tearDown(self):
        self._teardown_keystore()

    def test_ok(self):
        api_call_kwargs = client_req_csrf_setup()
        api_call_kwargs.update({"path": self.path, "method": "get"})
        response = self._send_request_to_backend(**api_call_kwargs)
        self.assertEqual(int(response.status_code), 200)
        rawdata = []
        for content in response.streaming_content:
            rawdata.append(content.decode())
        rawdata = "".join(rawdata)
        jwks_dict = json.loads(rawdata)
        for jwk_dict in jwks_dict["keys"]:
            jwk_pub = PyJWK(jwk_data=jwk_dict)
            self.assertEqual(jwk_dict["kid"], jwk_pub.key_id)
