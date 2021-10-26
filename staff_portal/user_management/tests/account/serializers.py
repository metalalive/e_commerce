import random
import copy
from datetime import timedelta
from unittest.mock import Mock, patch

from django.test import TransactionTestCase
from django.utils import timezone as django_timezone
from rest_framework.exceptions import ValidationError as DRFValidationError
from rest_framework.settings   import api_settings as drf_settings

from common.util.python.async_tasks  import sendmail as async_send_mail
from user_management.models.base import GenericUserProfile, EmailAddress
from user_management.serializers.auth import UnauthRstAccountReqSerializer

from tests.python.common import listitem_rand_assigner
from ..common import  _fixtures, gen_expiry_time

non_field_err_key = drf_settings.NON_FIELD_ERRORS_KEY

class RstAccountReqCreationTestCase(TransactionTestCase):
    def setUp(self):
        num_profiles = 4
        num_emails_per_usr = 3
        async_send_mail.app.conf.task_always_eager = True
        profiles = list(map(lambda d: GenericUserProfile.objects.create(**d) , _fixtures[GenericUserProfile][:num_profiles]))
        email_data_iter = iter(_fixtures[EmailAddress])
        for profile in profiles:
            for idx in range(num_emails_per_usr):
                email_data = next(email_data_iter)
                profile.emails.create(**email_data)
        self._profiles = profiles
        url_host = 'web.ecommerce.com'
        url_resource = 'account/create'
        self.expect_url_pattern = '/'.join([url_host, url_resource, '%s'])
        self.serializer_kwargs = {
            'msg_template_path': 'user_management/data/mail/body/user_activation_link_send.html',
            'subject_template' : 'user_management/data/mail/subject/user_activation_link_send.txt',
            'url_host': url_host, 'many':True, 'data':None,
            'url_resource': url_resource, # for account activation web page
        }

    def tearDown(self):
        UnauthRstAccountReqSerializer.Meta.model.objects.all().delete()
        async_send_mail.app.conf.task_always_eager = False

    def test_new_request_ok(self):
        req_data = list(map(lambda profile:{'email':profile.emails.last().id}, self._profiles))
        self.serializer_kwargs['data'] = req_data
        serializer = UnauthRstAccountReqSerializer(**self.serializer_kwargs)
        serializer.is_valid(raise_exception=True)
        with patch('django.core.mail.message.EmailMultiAlternatives') as mocked_obj:
            mocked_obj.send.return_value = 1234
            created_requests = serializer.save()
            profiles_iter = iter(self._profiles)
            call_args_iter = iter(mocked_obj.call_args_list)
            for req in created_requests:
                self.assertFalse(req.is_expired)
                self.assertIsNotNone(req.token)
                call_args = next(call_args_iter)
                self.assertIn(req.email.addr, call_args.kwargs['to'])
                profile = next(profiles_iter)
                pos = call_args.kwargs['subject'].find(profile.first_name)
                self.assertGreaterEqual(pos, 0)
                expect_url = self.expect_url_pattern % req.token
                pos = call_args.kwargs['body'].find(expect_url)
                self.assertGreaterEqual(pos, 0)
        actual_data = serializer.data
        self.assertSetEqual({'email', 'time_created', 'async_task'}, set(actual_data[0].keys()))


    def test_invalid_input(self):
        req_data = list(map(lambda profile:{'email':profile.emails.last().id}, self._profiles))
        invalid_reqs = [{}, {'email': None}, {'email': -123}, {'email':'xyz'}]
        req_data.extend(invalid_reqs)
        self.serializer_kwargs['data'] = req_data
        serializer = UnauthRstAccountReqSerializer(**self.serializer_kwargs)
        error_caught = None
        with self.assertRaises(DRFValidationError):
            try:
                serializer.is_valid(raise_exception=True)
            except DRFValidationError as e:
                error_caught = e
                raise
        self.assertIsNotNone(error_caught)
        err_info = error_caught.detail
        expect_err_code_seq = ['required', 'null', 'does_not_exist', 'incorrect_type']
        actual_err_code_seq = list(map(lambda e:e['email'][0].code, err_info[-4:]))
        self.assertListEqual(expect_err_code_seq, actual_err_code_seq)


    def test_new_request_dup_emails(self):
        req_data = list(map(lambda profile:{'email':profile.emails.last().id}, self._profiles[:2]))
        dup_req = req_data[0].copy()
        req_data.append(dup_req)
        self.serializer_kwargs['data'] = req_data
        serializer = UnauthRstAccountReqSerializer(**self.serializer_kwargs)
        serializer.is_valid(raise_exception=True)
        with patch('django.core.mail.message.EmailMultiAlternatives') as mocked_obj:
            mocked_obj.send.return_value = 1234
            created_requests = serializer.save()
        self._validate_dup_requests(evicted_req=created_requests[0], saved_req=created_requests[-1])

    def test_overwrite_existing_request(self):
        dup_email = self._profiles[0].emails.last()
        existing_req  = UnauthRstAccountReqSerializer.Meta.model.objects.create(email=dup_email)
        req_data = list(map(lambda profile:{'email':profile.emails.last().id}, self._profiles[:2]))
        self.serializer_kwargs['data'] = req_data
        serializer = UnauthRstAccountReqSerializer(**self.serializer_kwargs)
        serializer.is_valid(raise_exception=True)
        with patch('django.core.mail.message.EmailMultiAlternatives') as mocked_obj:
            mocked_obj.send.return_value = 1234
            created_requests = serializer.save()
        overwritten_req = created_requests[0]
        self._validate_dup_requests(evicted_req=existing_req, saved_req=overwritten_req)

    def _validate_dup_requests(self, evicted_req, saved_req):
        self.assertEqual(evicted_req.email, saved_req.email)
        self.assertNotEqual(evicted_req.hashed_token, saved_req.hashed_token)
        qset =  UnauthRstAccountReqSerializer.Meta.model.objects.filter(hashed_token=evicted_req.hashed_token)
        self.assertFalse(qset.exists())
        qset =  UnauthRstAccountReqSerializer.Meta.model.objects.filter(hashed_token=saved_req.hashed_token)
        self.assertTrue(qset.exists())


