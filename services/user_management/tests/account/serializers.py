from unittest.mock import patch

from django.conf import settings as django_settings
from django.test import TransactionTestCase
from django.core.exceptions import ObjectDoesNotExist
from rest_framework.exceptions import ValidationError as DRFValidationError
from rest_framework.settings import api_settings as drf_settings

from ecommerce_common.util.async_tasks import sendmail as async_send_mail
from user_management.models.base import GenericUserProfile, EmailAddress
from user_management.models.auth import LoginAccount
from user_management.serializers.auth import (
    UnauthRstAccountReqSerializer,
    LoginAccountSerializer,
)

from tests.common import _fixtures

non_field_err_key = drf_settings.NON_FIELD_ERRORS_KEY

MAIL_DATA_BASEPATH = django_settings.APP_DIR.joinpath("data/mail")


class BaseTestCase(TransactionTestCase):
    def setUp(self):
        async_send_mail.app.conf.task_always_eager = True

    def tearDown(self):
        async_send_mail.app.conf.task_always_eager = False
        UnauthRstAccountReqSerializer.Meta.model.objects.all().delete()

    def _gen_profiles(self, num_profiles, num_emails_per_usr):
        profiles = list(
            map(
                lambda d: GenericUserProfile.objects.create(**d),
                _fixtures[GenericUserProfile][:num_profiles],
            )
        )
        email_data_iter = iter(_fixtures[EmailAddress])
        for profile in profiles:
            for idx in range(num_emails_per_usr):
                email_data = next(email_data_iter)
                profile.emails.create(**email_data)
        return profiles


class AccountCreationRequestTestCase(BaseTestCase):
    def setUp(self):
        super().setUp()
        self._profiles = self._gen_profiles(num_profiles=4, num_emails_per_usr=3)
        url_host = "web.ecommerce.com"
        url_resource = "account/create"
        self.expect_url_pattern = "/".join([url_host, url_resource, "%s"])
        self.serializer_kwargs = {
            "msg_template_path": MAIL_DATA_BASEPATH.joinpath(
                "body/user_activation_link_send.html"
            ),
            "subject_template": MAIL_DATA_BASEPATH.joinpath(
                "subject/user_activation_link_send.txt"
            ),
            "url_host": url_host,
            "many": True,
            "data": None,
            "url_resource": url_resource,  # for account activation web page
        }

    def test_new_request_ok(self):
        req_data = list(
            map(lambda profile: {"email": profile.emails.last().id}, self._profiles)
        )
        self.serializer_kwargs["data"] = req_data
        serializer = UnauthRstAccountReqSerializer(**self.serializer_kwargs)
        serializer.is_valid(raise_exception=True)
        created_requests = serializer.save()
        expect_mail_ids = list(
            map(lambda profile: profile.emails.last().id, self._profiles)
        )
        for req in created_requests:
            expect_mail_ids.index(req.email.id)
            self.assertGreater(len(req.hashed_token), 0)
        actual_data = serializer.data
        self.assertSetEqual(
            {"email", "time_created", "async_task"}, set(actual_data[0].keys())
        )

    def test_invalid_input(self):
        req_data = list(
            map(lambda profile: {"email": profile.emails.last().id}, self._profiles)
        )
        invalid_reqs = [{}, {"email": None}, {"email": -123}, {"email": "xyz"}]
        req_data.extend(invalid_reqs)
        self.serializer_kwargs["data"] = req_data
        serializer = UnauthRstAccountReqSerializer(**self.serializer_kwargs)
        with self.assertRaises(DRFValidationError) as e:
            serializer.is_valid(raise_exception=True)
        self.assertIsNotNone(e.exception)
        err_info = e.exception.detail
        expect_err_code_seq = ["required", "null", "does_not_exist", "incorrect_type"]
        actual_err_code_seq = list(map(lambda e: e["email"][0].code, err_info[-4:]))
        self.assertListEqual(expect_err_code_seq, actual_err_code_seq)

    def test_new_request_dup_emails(self):
        req_data = list(
            map(lambda profile: {"email": profile.emails.last().id}, self._profiles[:2])
        )
        dup_req = req_data[0].copy()
        req_data.append(dup_req)
        self.serializer_kwargs["data"] = req_data
        serializer = UnauthRstAccountReqSerializer(**self.serializer_kwargs)
        serializer.is_valid(raise_exception=True)
        created_requests = serializer.save()
        self._validate_dup_requests(
            evicted_req=created_requests[0], saved_req=created_requests[-1]
        )

    def test_overwrite_existing_request(self):
        dup_email = self._profiles[0].emails.last()
        existing_req = UnauthRstAccountReqSerializer.Meta.model.objects.create(
            email=dup_email
        )
        req_data = list(
            map(lambda profile: {"email": profile.emails.last().id}, self._profiles[:2])
        )
        self.serializer_kwargs["data"] = req_data
        serializer = UnauthRstAccountReqSerializer(**self.serializer_kwargs)
        serializer.is_valid(raise_exception=True)
        with patch("django.core.mail.message.EmailMultiAlternatives") as mocked_obj:
            mocked_obj.send.return_value = 1234
            created_requests = serializer.save()
        overwritten_req = created_requests[0]
        self._validate_dup_requests(evicted_req=existing_req, saved_req=overwritten_req)

    def _validate_dup_requests(self, evicted_req, saved_req):
        self.assertEqual(evicted_req.email, saved_req.email)
        self.assertNotEqual(evicted_req.hashed_token, saved_req.hashed_token)
        qset = UnauthRstAccountReqSerializer.Meta.model.objects.filter(
            hashed_token=evicted_req.hashed_token
        )
        self.assertFalse(qset.exists())
        qset = UnauthRstAccountReqSerializer.Meta.model.objects.filter(
            hashed_token=saved_req.hashed_token
        )
        self.assertTrue(qset.exists())


## end of class AccountCreationRequestTestCase


class LoginAccountCreationTestCase(BaseTestCase):
    def setUp(self):
        super().setUp()
        profiles = self._gen_profiles(num_profiles=1, num_emails_per_usr=1)
        profile = profiles[0]
        rst_req = UnauthRstAccountReqSerializer.Meta.model.objects.create(
            email=profile.emails.first()
        )
        self.serializer_kwargs = {
            "mail_kwargs": {
                "msg_template_path": MAIL_DATA_BASEPATH.joinpath(
                    "body/user_activated.html"
                ),
                "subject_template": MAIL_DATA_BASEPATH.joinpath(
                    "subject/user_activated.txt"
                ),
            },
            "passwd_required": True,
            "confirm_passwd": True,
            "uname_required": True,
            "account": None,
            "rst_req": rst_req,
            "many": False,
        }
        self._profile = profile

    def test_create_ok(self):
        req_data = _fixtures[LoginAccount][0].copy()
        req_data["password2"] = req_data["password"]
        self.serializer_kwargs["data"] = req_data
        serializer = LoginAccountSerializer(**self.serializer_kwargs)
        serializer.is_valid(raise_exception=True)
        account = serializer.save()
        account.refresh_from_db()
        self.assertEqual(self._profile, account.profile)
        self.assertTrue(account.check_password(req_data["password"]))
        self.assertFalse(account.check_password(req_data["password"].lower()))
        with self.assertRaises(ObjectDoesNotExist):
            self.serializer_kwargs["rst_req"].refresh_from_db()
        self.assertEqual(self._profile.id, account.profile.id)

    def test_invalid_input(self):
        rst_req_bak = self.serializer_kwargs["rst_req"]
        req_data = {}
        self.serializer_kwargs["data"] = req_data
        self.serializer_kwargs["rst_req"] = None
        with self.assertRaises(AssertionError):
            LoginAccountSerializer(**self.serializer_kwargs)
        self.serializer_kwargs["rst_req"] = rst_req_bak
        serializer = LoginAccountSerializer(**self.serializer_kwargs)
        with self.assertRaises(DRFValidationError) as e:
            serializer.is_valid(raise_exception=True)
        error_caught = e.exception
        self.assertIsNotNone(error_caught)
        field_names = ("username", "password", "password2")
        for field_name in field_names:
            actual_err_code = error_caught.detail[field_name][0].code
            self.assertEqual(actual_err_code, "required")

    def test_username_duplicate(self):
        dup_account_data = _fixtures[LoginAccount][0]
        existing_profile = GenericUserProfile.objects.create(
            **_fixtures[GenericUserProfile][1]
        )
        existing_profile.activate(new_account_data=dup_account_data)
        req_data = dup_account_data.copy()
        req_data["password2"] = req_data["password"]
        self.serializer_kwargs["data"] = req_data
        serializer = LoginAccountSerializer(**self.serializer_kwargs)
        with self.assertRaises(DRFValidationError) as e:
            serializer.is_valid(raise_exception=True)
        error_caught = e.exception
        self.assertIsNotNone(error_caught)
        pos = error_caught.detail["username"][0].find("username already exists")
        self.assertGreater(pos, 0)

    def test_passwd_check_failure(self):
        req_data = _fixtures[LoginAccount][0].copy()
        self.serializer_kwargs["data"] = req_data
        req_data["password"] = "TooeAsy"
        req_data["password2"] = "TooEasy"
        serializer = LoginAccountSerializer(**self.serializer_kwargs)
        with self.assertRaises(DRFValidationError) as e:
            serializer.is_valid(raise_exception=True)
        error_caught = e.exception
        self.assertIsNotNone(error_caught)
        expect_err_codes = {"min_length", "confirm_fail", "special_char_required"}
        actual_err_codes = set(map(lambda ed: ed.code, error_caught.detail["password"]))
        self.assertSetEqual(expect_err_codes, actual_err_codes)


## end of class LoginAccountCreationTestCase


class UnauthResetPasswordTestCase(BaseTestCase):
    def setUp(self):
        super().setUp()
        profiles = self._gen_profiles(num_profiles=1, num_emails_per_usr=1)
        profile = profiles[0]
        profile.activate(new_account_data=_fixtures[LoginAccount][0])
        rst_req = UnauthRstAccountReqSerializer.Meta.model.objects.create(
            email=profile.emails.first()
        )
        self.serializer_kwargs = {
            "mail_kwargs": {
                "msg_template_path": MAIL_DATA_BASEPATH.joinpath(
                    "body/unauth_passwd_reset.html"
                ),
                "subject_template": MAIL_DATA_BASEPATH.joinpath(
                    "subject/unauth_passwd_reset.txt"
                ),
            },
            "data": None,
            "passwd_required": True,
            "confirm_passwd": True,
            "account": profile.account,
            "rst_req": rst_req,
            "many": False,
        }
        self._profile = profile

    def test_modify_ok(self):
        old_passwd = _fixtures[LoginAccount][0]["password"]
        new_passwd = "aBcDeFg$1234"
        req_data = {"password": new_passwd, "password2": new_passwd}
        self.serializer_kwargs["data"] = req_data
        self.assertTrue(self._profile.account.check_password(old_passwd))
        self.assertFalse(self._profile.account.check_password(new_passwd))
        serializer = LoginAccountSerializer(**self.serializer_kwargs)
        serializer.is_valid(raise_exception=True)
        account = serializer.save()
        account.refresh_from_db()
        self.assertEqual(self._profile.account, account)
        self.assertTrue(account.check_password(new_passwd))
        self.assertFalse(account.check_password(old_passwd))
        # request should be deleted immediately
        with self.assertRaises(ObjectDoesNotExist) as e:
            self.serializer_kwargs["rst_req"].refresh_from_db()
        self.assertIsNotNone(e.exception)

    def test_passwd_check_failure(self):
        req_data = {"password": "TooeAsy", "password2": "TooEasy"}
        self.serializer_kwargs["data"] = req_data
        serializer = LoginAccountSerializer(**self.serializer_kwargs)
        with self.assertRaises(DRFValidationError) as e:
            serializer.is_valid(raise_exception=True)
        error_caught = e.exception
        self.assertIsNotNone(error_caught)
        expect_err_codes = {"min_length", "confirm_fail", "special_char_required"}
        actual_err_codes = set(map(lambda ed: ed.code, error_caught.detail["password"]))
        self.assertSetEqual(expect_err_codes, actual_err_codes)


## end of class UnauthResetPasswordTestCase


class AuthChangeUsernameTestCase(BaseTestCase):
    def setUp(self):
        super().setUp()
        profiles = self._gen_profiles(num_profiles=1, num_emails_per_usr=1)
        profile = profiles[0]
        account = profile.activate(new_account_data=_fixtures[LoginAccount][0])
        self.serializer_kwargs = {
            "data": None,
            "uname_required": True,
            "old_uname_required": True,
            "account": account,
            "rst_req": None,
            "many": False,
        }
        self._profile = profile

    def test_modify_ok(self):
        old_uname = _fixtures[LoginAccount][0]["username"]
        new_uname = "kanbanORagile"
        req_data = {"username": new_uname, "old_uname": old_uname}
        self.serializer_kwargs["data"] = req_data
        self.assertTrue(self._profile.account.username, old_uname)
        serializer = LoginAccountSerializer(**self.serializer_kwargs)
        serializer.is_valid(raise_exception=True)
        account = serializer.save()
        account.refresh_from_db()
        self.assertTrue(account.username, new_uname)

    def test_invalid_old_uname(self):
        old_uname = _fixtures[LoginAccount][0]["username"][1:]
        new_uname = "kanbanORagile"
        req_data = {"username": new_uname, "old_uname": old_uname}
        self.serializer_kwargs["data"] = req_data
        serializer = LoginAccountSerializer(**self.serializer_kwargs)
        with self.assertRaises(DRFValidationError) as error_caught:
            serializer.is_valid(raise_exception=True)
        error_caught = error_caught.exception
        self.assertIsNotNone(error_caught)
        self.assertEqual(error_caught.detail["old_uname"][0], "incorrect old username")


class AuthChangePasswordTestCase(BaseTestCase):
    def setUp(self):
        super().setUp()
        profiles = self._gen_profiles(num_profiles=1, num_emails_per_usr=1)
        profile = profiles[0]
        account = profile.activate(new_account_data=_fixtures[LoginAccount][0])
        self.serializer_kwargs = {
            "data": None,
            "passwd_required": True,
            "confirm_passwd": True,
            "old_passwd_required": True,
            "account": account,
            "rst_req": None,
            "many": False,
        }
        self._profile = profile

    def test_modify_ok(self):
        old_passwd = _fixtures[LoginAccount][0]["password"]
        new_passwd = "aBcDeFg$1234"
        req_data = {
            "password": new_passwd,
            "password2": new_passwd,
            "old_passwd": old_passwd,
        }
        self.serializer_kwargs["data"] = req_data
        self.assertTrue(self._profile.account.check_password(old_passwd))
        self.assertFalse(self._profile.account.check_password(new_passwd))
        serializer = LoginAccountSerializer(**self.serializer_kwargs)
        serializer.is_valid(raise_exception=True)
        account = serializer.save()
        account.refresh_from_db()
        self.assertTrue(account.check_password(new_passwd))
        self.assertFalse(account.check_password(old_passwd))

    def test_passwd_check_failure(self):
        incorrect_old_passwd = _fixtures[LoginAccount][0]["password"][:-1]
        req_data = {
            "password": "tooEasy",
            "password2": "2easy",
            "old_passwd": incorrect_old_passwd,
        }
        self.serializer_kwargs["data"] = req_data
        serializer = LoginAccountSerializer(**self.serializer_kwargs)
        with self.assertRaises(DRFValidationError) as error_caught:
            serializer.is_valid(raise_exception=True)
        error_caught = error_caught.exception
        self.assertIsNotNone(error_caught)
        expect_err_codes = {"min_length", "confirm_fail", "special_char_required"}
        actual_err_codes = set(map(lambda ed: ed.code, error_caught.detail["password"]))
        self.assertSetEqual(expect_err_codes, actual_err_codes)
        self.assertEqual("incorrect old password", error_caught.detail["old_passwd"][0])
