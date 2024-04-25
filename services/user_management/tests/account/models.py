import random
import hashlib
from datetime import  datetime, timedelta
from unittest.mock import patch

from django.core.exceptions import ObjectDoesNotExist
from django.utils import timezone as django_timezone
from django.test import TransactionTestCase
from rest_framework.exceptions  import PermissionDenied

from ecommerce_common.models.constants     import ROLE_ID_SUPERUSER
from user_management.models.base import EmailAddress, GenericUserProfile
from user_management.models.auth import Role, LoginAccount, UnauthResetAccountRequest
from user_management.async_tasks import clean_expired_reset_requests

from ..common import _fixtures

class ResetRequestCreationTestCase(TransactionTestCase):
    def setUp(self):
        profile_data = _fixtures[GenericUserProfile][0]
        profile = GenericUserProfile.objects.create(**profile_data)
        for email_data in _fixtures[EmailAddress][:4]:
            profile.emails.create(**email_data)
        self._profile = profile

    def tearDown(self):
        # manually delete all data cuz the model is NOT automatically managed by Django ORM
        UnauthResetAccountRequest.objects.all().delete()

    def test_save_then_read(self):
        rst_req = UnauthResetAccountRequest.objects.create(email = self._profile.emails.first())
        token = rst_req.token
        self.assertIsNotNone(token)
        self.assertTrue(isinstance(token, str))
        hashed_token_1 = rst_req.hashed_token
        rst_req = None
        # -------------------------------------
        hashobj = hashlib.sha256()
        hashobj.update(token.encode('utf-8'))
        hashed_token_2 = hashobj.digest()
        self.assertEqual(hashed_token_1, hashed_token_2)
        rst_req = UnauthResetAccountRequest.objects.get(hashed_token=hashed_token_2)
        self.assertIsNotNone(rst_req)
        self.assertFalse( rst_req.is_expired )
        # assume it is expired
        req_valid_secs = UnauthResetAccountRequest.MAX_TOKEN_VALID_TIME + random.randrange(3,15)
        mocked_nowtime = django_timezone.now() + timedelta(seconds=req_valid_secs)
        with patch('user_management.models.auth.django_timezone.now') as mock_nowtime_fn:
            mock_nowtime_fn.return_value = mocked_nowtime
            self.assertTrue( rst_req.is_expired )
        rst_req = None
        # -------------------------------------
        rst_req = UnauthResetAccountRequest.get_request(token_urlencoded=token)
        self.assertIsNotNone(rst_req)
        self.assertEqual(hashed_token_1, rst_req.hashed_token)
        ## with patch('datetime.datetime.now') as mock_nowtime: # NOT ALLOWED to patch ???
        ##     mock_nowtime.return_value = rst_req.time_created + timedelta(seconds=type(rst_req).MAX_TOKEN_VALID_TIME + 2)
        ##     self.assertTrue( rst_req.is_token_expired )

    def test_duplicate_hashed_token(self):
        emails = self._profile.emails.all()
        rst_reqs = []
        rst_reqs.append( UnauthResetAccountRequest.objects.create(email=emails[0]) )
        dup_hash_token = rst_reqs[-1].hashed_token
        rst_reqs.append( UnauthResetAccountRequest.objects.create(email=emails[1], hashed_token=dup_hash_token) )
        rst_reqs.append( UnauthResetAccountRequest.objects.create(email=emails[0], hashed_token=dup_hash_token) )
        hashed_tokens = list(map(lambda obj:obj.hashed_token, rst_reqs))
        distinct_hashed_tokens = set(hashed_tokens)
        self.assertEqual(len(hashed_tokens), len(distinct_hashed_tokens))


class ResetRequestDeletionTestCase(TransactionTestCase):
    def setUp(self):
        profiles = list(map(lambda d: GenericUserProfile.objects.create(**d) , _fixtures[GenericUserProfile][:2]))
        email_data_iter = iter(_fixtures[EmailAddress])
        for profile in profiles:
            for idx in range(4):
                email_data = next(email_data_iter)
                profile.emails.create(**email_data)
            for email in profile.emails.all()[1:]:
                UnauthResetAccountRequest.objects.create(email=email)
        self._profile = profiles[0]
        self._profile_2nd = profiles[1]

    def tearDown(self):
        # manually delete all data cuz the model is NOT automatically managed by Django ORM
        UnauthResetAccountRequest.objects.all().delete()

    def test_remove_user_profile(self):
        self._profile.delete(profile_id=self._profile.id)
        usr_emails = self._profile.emails.all(with_deleted=True)
        for email in usr_emails:
            req_exists = email.rst_account_reqs.all().exists()
            self.assertFalse(req_exists)

    def test_remove_by_cron_job(self):
        deleted_items = clean_expired_reset_requests(days=1)
        self.assertFalse(any(deleted_items))
        # assume the following reset requests are already expired
        expect_expired_reqs = [
            self._profile.emails.last().rst_account_reqs.last(),
            self._profile_2nd.emails.last().rst_account_reqs.first(),
        ]
        req_valid_secs = UnauthResetAccountRequest.MAX_TOKEN_VALID_TIME + random.randrange(5,15)
        mocked_expired_time = django_timezone.now() - timedelta(seconds=req_valid_secs)
        with patch('django.utils.timezone.now') as mock_nowtime_fn:
            mock_nowtime_fn.return_value = mocked_expired_time
            for rst_req in expect_expired_reqs:
                 rst_req.save()
        deleted_items = clean_expired_reset_requests(days=0, hours=0,
                minutes=(UnauthResetAccountRequest.MAX_TOKEN_VALID_TIME/60) )
        expect_value = set(map(lambda obj: (obj.email.user_id, obj.email.addr), expect_expired_reqs))
        actual_value = set(map(lambda d: (d['email__user_id'], d['email__addr']) , deleted_items))
        self.assertSetEqual(expect_value, actual_value)
        req_hashed_tokens = tuple(map(lambda obj: obj.hashed_token, expect_expired_reqs))
        qset = UnauthResetAccountRequest.objects.filter(hashed_token__in=req_hashed_tokens)
        self.assertFalse(qset.exists())



class LoginAccountDeletionTestCase(TransactionTestCase):
    def setUp(self):
        num_superusers = 3
        role_data = {'id':ROLE_ID_SUPERUSER, 'name':'my default superuser'}
        superuser_role = Role.objects.create(**role_data)
        profiles = list(map(lambda d: GenericUserProfile.objects.create(**d) , _fixtures[GenericUserProfile][:num_superusers]))
        tuple(map(lambda profile:profile.roles.create(approved_by=profiles[0], role=superuser_role), profiles))
        account_data_iter = iter(_fixtures[LoginAccount])
        tuple(map(lambda profile: profile.activate(new_account_data=next(account_data_iter)) , profiles))
        self._profiles = profiles

    def test_remove_superusers(self):
        first_profile = self._profiles[0]
        for profile in self._profiles[1:]:
            profile.account
            profile.delete(profile_id=first_profile.id)
            profile.refresh_from_db()
            self.assertTrue(profile.is_deleted())
            with self.assertRaises(ObjectDoesNotExist):
                profile.account
        error_caught = None
        with self.assertRaises(PermissionDenied):
            try:
                first_profile.delete(profile_id=first_profile.id)
            except PermissionDenied as e:
                error_caught = e
                raise
        self.assertIsNotNone(error_caught)
        first_profile.refresh_from_db()
        self.assertFalse(first_profile.is_deleted())
        self.assertTrue(first_profile.account.is_active)
        self.assertTrue(first_profile.account.is_superuser)


    def test_deactivate_superusers(self):
        first_profile = self._profiles[0]
        for profile in self._profiles[1:]:
            profile.deactivate()
            profile.refresh_from_db()
            self.assertFalse(profile.is_deleted())
            self.assertFalse(profile.account.is_active)
        with self.assertRaises(PermissionDenied):
            try:
                first_profile.deactivate()
            except PermissionDenied as e:
                error_caught = e
                raise
        self.assertIsNotNone(error_caught)
        first_profile.refresh_from_db()
        self.assertFalse(first_profile.is_deleted())
        self.assertTrue(first_profile.account.is_active)
        self.assertTrue(first_profile.account.is_superuser)


