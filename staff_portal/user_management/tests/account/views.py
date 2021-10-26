import copy
from unittest.mock import  patch

from django.test import TransactionTestCase
from django.contrib.auth.models import Permission
from django.core.exceptions import ObjectDoesNotExist

from common.util.python.async_tasks  import sendmail as async_send_mail
from user_management.views.constants import  WEB_HOST
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



class AccountDeactivationTestCase(BaseViewTestCase):
    path = '/account/deactivate'

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
        perms_code_required = ['delete_unauthresetaccountrequest', 'change_loginaccount', 'delete_loginaccount']
        perm_objs = Permission.objects.filter(content_type__app_label='user_management', codename__in=perms_code_required)
        roles[1].permissions.set(perm_objs)
        # ---- activation at model level, after role setup
        account_data = copy.deepcopy(_fixtures[LoginAccount][:num_profiles])
        account_data_iter = iter(account_data)
        tuple(map(lambda profile:profile.activate(new_account_data=next(account_data_iter)), profiles))
        # login & prepare access token
        self._auth_setup(testcase=self, profile=self._default_login_profile, is_superuser=False,
                login_password=_fixtures[LoginAccount][0]['password'])
        acs_tok_resp = self._refresh_access_token(testcase=self, audience=['user_management'])
        default_user_access_token = acs_tok_resp['access_token']
        api_call_kwargs = client_req_csrf_setup()
        api_call_kwargs.update({'path': self.path, 'method':'post'})
        api_call_kwargs['headers']['HTTP_AUTHORIZATION'] = ' '.join(['Bearer', default_user_access_token])
        self.api_call_kwargs = api_call_kwargs


    def test_deactivate_ok(self):
        body = list(map(lambda profile:{'profile':profile.id, 'remove_account':False}, self._test_profiles))
        body[0]['remove_account'] = True
        body[2]['remove_account'] = True
        self.api_call_kwargs['body'] = body
        response = self._send_request_to_backend(**self.api_call_kwargs)
        self.assertEqual(int(response.status_code), 200)
        tuple(map(lambda profile: profile.refresh_from_db(), self._test_profiles))
        profiles_account_deleted = (self._test_profiles[0], self._test_profiles[2])
        profiles_account_inactive = (self._test_profiles[1], self._test_profiles[3])
        for profile in profiles_account_deleted:
            with self.assertRaises(ObjectDoesNotExist):
                profile.account
        for profile in profiles_account_inactive:
            self.assertFalse(profile.account.is_active)


    def test_invalid_input(self):
        # subcase #1
        body = [{}, {'profile': None}, {'profile': -123}, {'profile':'xyz'}]
        self.api_call_kwargs['body'] = body
        response = self._send_request_to_backend(**self.api_call_kwargs)
        self.assertEqual(int(response.status_code), 403) # blocked by permission class
        # subcase #2 , non-existent email ID
        body = list(map(lambda profile:{'profile':profile.id}, self._test_profiles))
        body.append({'profile': 9999})
        self.api_call_kwargs['body'] = body
        response = self._send_request_to_backend(**self.api_call_kwargs)
        self.assertEqual(int(response.status_code), 403)


class UnauthPasswdRstReqTestCase(BaseViewTestCase):
    path = '/account/password/reset'

    def setUp(self):
        super().setUp()
        profile = GenericUserProfile.objects.create(**_fixtures[GenericUserProfile][0])
        email_data = _fixtures[EmailAddress][0]
        profile.emails.create(**email_data)
        account_data = _fixtures[LoginAccount][0]
        profile.activate(new_account_data=account_data)
        self._account_data = account_data
        self._profile = profile
        api_call_kwargs = client_req_csrf_setup()
        api_call_kwargs.update({'path': self.path, 'method':'post'})
        self.api_call_kwargs = api_call_kwargs

    def tearDown(self):
        super().tearDown()
        UnauthResetAccountRequest.objects.all().delete()

    @patch('django.core.mail.message.EmailMultiAlternatives')
    def test_request_ok(self, mocked_email_obj):
        expect_addr = self._profile.emails.first().addr
        body = {'addr': expect_addr}
        self.api_call_kwargs['body'] = body
        response = self._send_request_to_backend(**self.api_call_kwargs)
        self.assertEqual(int(response.status_code), 202)
        self.assertEqual(1, mocked_email_obj.call_count)
        mocked_email_obj = mocked_email_obj.call_args.kwargs
        self.assertIn(expect_addr, mocked_email_obj['to'])
        expect_url_prefix = '%s%s' % (WEB_HOST, self.path)
        pos = mocked_email_obj['body'].find(expect_url_prefix)
        self.assertGreaterEqual(pos, 0)
        # find token
        qset = UnauthResetAccountRequest.objects.all()
        self.assertEqual(1, qset.count())
        rst_req = qset.first()
        self.assertEqual(self._profile.emails.first(), rst_req.email)
        self.assertFalse(rst_req.is_expired)
        #pos += len(expect_url_prefix) + 1
        #token = mocked_email_obj['body'][pos: pos+32]


    @patch('django.core.mail.message.EmailMultiAlternatives')
    def test_invalid_input(self, mocked_email_obj):
        expect_addr = 'invalid%s' % self._profile.emails.first().addr
        body = {'addr': expect_addr}
        self.api_call_kwargs['body'] = body
        response = self._send_request_to_backend(**self.api_call_kwargs)
        self.assertEqual(int(response.status_code), 202)
        self.assertEqual(0, mocked_email_obj.call_count)
        qset = UnauthResetAccountRequest.objects.all()
        self.assertFalse(qset.exists())


