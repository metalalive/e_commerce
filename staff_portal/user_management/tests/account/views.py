from unittest.mock import  patch

from django.test import TransactionTestCase
from django.contrib.auth.models import Permission
from django.core.exceptions import ObjectDoesNotExist

from common.util.python.async_tasks  import sendmail as async_send_mail
from user_management.models.base import GenericUserProfile, GenericUserGroup, GenericUserGroupClosure,  EmailAddress
from user_management.models.auth import Role, LoginAccount, UnauthResetAccountRequest

from tests.python.common.django import _BaseMockTestClientInfoMixin

from ..common import _fixtures, gen_expiry_time, client_req_csrf_setup, AuthenticateUserMixin


class BaseViewTestCase(TransactionTestCase, _BaseMockTestClientInfoMixin, AuthenticateUserMixin):
    def _setup_user_roles(self, profile, approved_by, roles=None):
        roles = roles or []
        role_rel_data = {'expiry':gen_expiry_time(minutes_valid=10), 'approved_by': approved_by,}
        tuple(map(lambda role: profile.roles.create(role=role, **role_rel_data), roles))

    def setUp(self):
        async_send_mail.app.conf.task_always_eager = True

    def tearDown(self):
        self._client.cookies.clear()
        async_send_mail.app.conf.task_always_eager = False


class AccountActivationTestCase(BaseViewTestCase):
    path = '/account/activate'

    def setUp(self):
        super().setUp()
        num_profiles = 5
        num_emails_per_usr = 3
        # all user profiles are in the same group for simplicity
        group = GenericUserGroup.objects.create(**_fixtures[GenericUserGroup][0])
        GenericUserGroupClosure.objects.create(id=1, depth=0, ancestor=group, descendant=group)
        profiles = list(map(lambda d: GenericUserProfile.objects.create(**d) , _fixtures[GenericUserProfile][:num_profiles]))
        self._default_login_profile = profiles[0]
        self._test_profiles = profiles[1:]
        tuple(map(lambda profile:profile.groups.create(group=group, approved_by=self._default_login_profile) , profiles))
        # ---- set up role/permission for the user who performs the activation
        roles = list(map(lambda d: Role.objects.create(**d) , _fixtures[Role][:2]))
        self._setup_user_roles(profile=self._default_login_profile, approved_by=self._default_login_profile, roles=roles,)
        perm_objs = Permission.objects.filter(content_type__app_label='user_management',
                        codename__in=['add_unauthresetaccountrequest', 'change_loginaccount'])
        roles[1].permissions.set(perm_objs)
        # ---- set up email addresses to rest of user profiles
        email_data_iter = iter(_fixtures[EmailAddress])
        for profile in self._test_profiles:
            for idx in range(num_emails_per_usr):
                email_data = next(email_data_iter)
                profile.emails.create(**email_data)
        # login & prepare access token
        self._auth_setup(testcase=self, profile=self._default_login_profile, is_superuser=False,
                new_account_data=_fixtures[LoginAccount][0].copy())
        acs_tok_resp = self._refresh_access_token(testcase=self, audience=['user_management'])
        default_user_access_token = acs_tok_resp['access_token']
        api_call_kwargs = client_req_csrf_setup()
        api_call_kwargs.update({'path': self.path, 'method':'post'})
        api_call_kwargs['headers']['HTTP_AUTHORIZATION'] = ' '.join(['Bearer', default_user_access_token])
        self.api_call_kwargs = api_call_kwargs

    def tearDown(self):
        super().tearDown()
        UnauthResetAccountRequest.objects.all().delete()

    def test_activate_first_time(self):
        body = list(map(lambda profile:{'profile':profile.id, 'email':profile.emails.last().id}, self._test_profiles))
        self.api_call_kwargs['body'] = body
        with patch('django.core.mail.message.EmailMultiAlternatives') as mocked_obj:
            response = self._send_request_to_backend(**self.api_call_kwargs)
        self.assertEqual(int(response.status_code), 201)
        result = response.json()
        # TODO, examine the extra field `async_task`
        expect_email_ids = set(UnauthResetAccountRequest.objects.values_list('email', flat=True))
        actual_email_ids = set(map(lambda d:d['email'], result))
        self.assertSetEqual(expect_email_ids, actual_email_ids)


    def test_invalid_input(self):
        # subcase #1
        body = [{}, {'profile': None, 'email': None}, {'profile': -123, 'email': -123},
                {'profile':'xyz', 'email':'xyz'} ]
        self.api_call_kwargs['body'] = body
        response = self._send_request_to_backend(**self.api_call_kwargs)
        self.assertEqual(int(response.status_code), 403) # blocked by permission class
        # subcase #2
        body = [{}, {'email': None}, {'email': -123}, {'email':'xyz'}]
        self.api_call_kwargs['body'] = body
        response = self._send_request_to_backend(**self.api_call_kwargs)
        self.assertEqual(int(response.status_code), 403)
        # subcase #3 , non-existent email ID
        body = list(map(lambda profile:{'email':profile.emails.last().id}, self._test_profiles))
        body.append({'email': 9999})
        self.api_call_kwargs['body'] = body
        response = self._send_request_to_backend(**self.api_call_kwargs)
        self.assertEqual(int(response.status_code), 403)


    @patch('django.core.mail.message.EmailMultiAlternatives')
    def test_overwrite_existing_request(self, mocked_email_obj):
        dup_email = self._test_profiles[0].emails.last()
        existing_req  = UnauthResetAccountRequest.objects.create(email=dup_email)
        body = list(map(lambda profile:{'email':profile.emails.last().id}, self._test_profiles))
        self.api_call_kwargs['body'] = body
        response = self._send_request_to_backend(**self.api_call_kwargs)
        self.assertEqual(int(response.status_code), 201)
        new_req = UnauthResetAccountRequest.objects.get(email=existing_req.email)
        self.assertNotEqual(existing_req.hashed_token, new_req.hashed_token)
        with self.assertRaises(ObjectDoesNotExist):
            existing_req.refresh_from_db()


    @patch('django.core.mail.message.EmailMultiAlternatives')
    def test_reactivate(self, mocked_email_obj):
        self._test_profiles[-1].activate(new_account_data=_fixtures[LoginAccount][1])
        self._test_profiles[-1].deactivate(remove_account=False)
        self.assertFalse(self._test_profiles[-1].account.is_active)
        body = list(map(lambda profile:{'email':profile.emails.first().id}, self._test_profiles[:-1]))
        body.append({'profile': self._test_profiles[-1].id})
        self.api_call_kwargs['body'] = body
        response = self._send_request_to_backend(**self.api_call_kwargs)
        self.assertEqual(int(response.status_code), 201)
        self._test_profiles[-1].account.refresh_from_db()
        self.assertTrue(self._test_profiles[-1].account.is_active)
        result = response.json()
        expect_email_ids = set(UnauthResetAccountRequest.objects.values_list('email', flat=True))
        actual_email_ids = set(map(lambda d:d['email'], result))
        self.assertSetEqual(expect_email_ids, actual_email_ids)


