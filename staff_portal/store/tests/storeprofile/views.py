import random
from unittest.mock import patch

import pytest

from common.models.constants  import ROLE_ID_STAFF
from common.models.enums.base import AppCodeOptions, ActivationStatus
from common.util.python.messaging.rpc import RpcReplyEvent

from store.models import StoreProfile, StorePhone, StoreEmail
from store.tests.common import db_engine_resource, session_for_test, session_for_setup, keystore, test_client, store_data, email_data, phone_data, loc_data, opendays_data, staff_data, product_avail_data

app_code = AppCodeOptions.store.value[0]

class TestCreation:
    # class name must start with TestXxxx
    url = '/profiles'

    def test_auth_failure(self, session_for_test, keystore, test_client):
        # no need to test CORS middleware ?
        response = test_client.post(self.url, headers={}, json=[])
        assert response.status_code == 401
        response = test_client.post(self.url, headers={'Authorization': 'Bearer abc1234efg'}, json=[])
        assert response.status_code == 401
        result = response.json()
        assert result['detail'] == 'authentication failure'
        profile_data = {'id':136 , 'privilege_status':ROLE_ID_STAFF, 'quotas':[],
                'roles':[{'app_code':app_code, 'codename':'view_storeprofile'},
                    {'app_code':app_code, 'codename':'add_xoxoxox'}]
                }
        encoded_token = keystore.gen_access_token(profile=profile_data, audience=['store'])
        with patch('jwt.PyJWKClient.fetch_data', keystore._mocked_get_jwks) as mocked_obj:
            response = test_client.post(self.url, headers={'Authorization': 'Bearer %s' % encoded_token},
                    json=[])
        assert response.status_code == 403
        result = response.json()
        assert result['detail'] == 'Permission check failure'


    def _mocked_rpc_reply_refresh(self, *args, **kwargs):
        # skip receiving message from RPC-reply-queue
        pass

    @patch('common.util.python.messaging.rpc.RpcReplyEvent.refresh', _mocked_rpc_reply_refresh)
    def test_bulk_ok(self, session_for_test, keystore, test_client, store_data, email_data, phone_data, loc_data):
        num_items = 4
        profile_data = {'id':130, 'privilege_status':ROLE_ID_STAFF, 'quotas':[],
                'roles':[{'app_code':app_code, 'codename':'view_storeprofile'},
                    {'app_code':app_code, 'codename':'add_storeprofile'}]
                }
        encoded_token = keystore.gen_access_token(profile=profile_data, audience=['store'])
        headers = {'Authorization': 'Bearer %s' % encoded_token}
        body = [next(store_data) for _ in range(num_items)]
        for item in body:
            item['emails'] = [next(email_data) for _ in range(random.randrange(0,3))]
            item['phones'] = [next(phone_data) for _ in range(random.randrange(0,3))]
            if random.choice([True, False]):
                _loc_data = next(loc_data)
                _loc_data['country'] = _loc_data['country'].value
                item['location'] = _loc_data
        with patch('jwt.PyJWKClient.fetch_data', keystore._mocked_get_jwks):
            with patch('common.util.python.messaging.rpc.MethodProxy._call') as mocked_rpc_proxy_call:
                # skip publishing message to RPC queue
                reply_event = RpcReplyEvent(listener=self, timeout_s=7)
                reply_event.resp_body['status'] = RpcReplyEvent.status_opt.SUCCESS
                reply_event.resp_body['result'] = [{
                    'id':item['supervisor_id'], 'auth':ActivationStatus.ACCOUNT_ACTIVATED.value, \
                    'quota':[
                        {'app_code':app_code, 'mat_code':StoreProfile.quota_material.value, 'maxnum':random.randrange(3,6)} ,
                        {'app_code':app_code, 'mat_code':StoreEmail.quota_material.value, 'maxnum':random.randrange(3,6)} ,
                        {'app_code':app_code, 'mat_code':StorePhone.quota_material.value, 'maxnum':random.randrange(3,6)} ,
                    ]
                } for item in body]
                mocked_rpc_proxy_call.return_value = reply_event
                response = test_client.post(self.url, headers=headers, json=body)
        assert response.status_code == 201
        result = response.json()
        expect_prof_ids = list(map(lambda d:d['supervisor_id'], body))
        query = session_for_test.query(StoreProfile.id , StoreProfile.supervisor_id)
        query = query.filter(StoreProfile.supervisor_id.in_(expect_prof_ids))
        expect_data = dict(query.all())
        actual_data = dict(map(lambda d: (d['id'], d['supervisor_id']), result))
        assert expect_data == actual_data


    @patch('common.util.python.messaging.rpc.RpcReplyEvent.refresh', _mocked_rpc_reply_refresh)
    def test_empty_input(self, keystore, test_client):
        profile_data = {'id':58, 'privilege_status':ROLE_ID_STAFF, 'quotas':[],
                'roles':[{'app_code':app_code, 'codename':'view_storeprofile'},
                    {'app_code':app_code, 'codename':'add_storeprofile'}]
                }
        encoded_token = keystore.gen_access_token(profile=profile_data, audience=['store'])
        headers = {'Authorization': 'Bearer %s' % encoded_token}
        with patch('jwt.PyJWKClient.fetch_data', keystore._mocked_get_jwks):
            with patch('common.util.python.messaging.rpc.MethodProxy._call') as mocked_rpc_proxy_call:
                body = []
                response = test_client.post(self.url, headers=headers, json=body)
        assert response.status_code == 422
        result = response.json()
        assert result['detail'][0]['msg'] == 'Empty request body Not Allowed'
        with patch('jwt.PyJWKClient.fetch_data', keystore._mocked_get_jwks):
            with patch('common.util.python.messaging.rpc.MethodProxy._call') as mocked_rpc_proxy_call:
                body = [{}, {}]
                response = test_client.post(self.url, headers=headers, json=body)
        result = response.json()
        assert response.status_code == 422
        for item in result['detail']:
            assert item['loc'][-1] in ('label', 'supervisor_id')
            assert item['msg'] == 'field required'


    @patch('common.util.python.messaging.rpc.RpcReplyEvent.refresh', _mocked_rpc_reply_refresh)
    def test_auth_app_down(self, keystore, test_client, store_data):
        profile_data = {'id':58, 'privilege_status':ROLE_ID_STAFF, 'quotas':[],
                'roles':[{'app_code':app_code, 'codename':'view_storeprofile'},
                    {'app_code':app_code, 'codename':'add_storeprofile'}]
                }
        encoded_token = keystore.gen_access_token(profile=profile_data, audience=['store'])
        headers = {'Authorization': 'Bearer %s' % encoded_token}
        body = [next(store_data)]
        expect_rpc_fail_status = [
                RpcReplyEvent.status_opt.FAIL_CONN,
                RpcReplyEvent.status_opt.FAIL_PUBLISH,
                RpcReplyEvent.status_opt.REMOTE_ERROR,
            ]
        reply_event = RpcReplyEvent(listener=self, timeout_s=7)
        reply_event.resp_body['result'] = []
        with patch('jwt.PyJWKClient.fetch_data', keystore._mocked_get_jwks):
            with patch('common.util.python.messaging.rpc.MethodProxy._call') as mocked_rpc_proxy_call:
                for fail_status in expect_rpc_fail_status:
                    reply_event.resp_body['status'] = fail_status
                    mocked_rpc_proxy_call.return_value = reply_event
                    response = test_client.post(self.url, headers=headers, json=body)
                    result = response.json()
                    assert response.status_code == 503
                    assert result['detail'] == 'Authentication service is currently down'


    @patch('common.util.python.messaging.rpc.RpcReplyEvent.refresh', _mocked_rpc_reply_refresh)
    def test_invalid_supervisor_id(self, keystore, test_client, store_data):
        num_items = 4
        profile_data = {'id':99, 'privilege_status':ROLE_ID_STAFF, 'quotas':[],
                'roles':[{'app_code':app_code, 'codename':'view_storeprofile'},
                    {'app_code':app_code, 'codename':'add_storeprofile'}]
                }
        encoded_token = keystore.gen_access_token(profile=profile_data, audience=['store'])
        headers = {'Authorization': 'Bearer %s' % encoded_token}
        body = [next(store_data) for _ in range(num_items)]
        with patch('jwt.PyJWKClient.fetch_data', keystore._mocked_get_jwks):
            with patch('common.util.python.messaging.rpc.MethodProxy._call') as mocked_rpc_proxy_call:
                # skip publishing message to RPC queue
                reply_event = RpcReplyEvent(listener=self, timeout_s=7)
                reply_event.resp_body['status'] = RpcReplyEvent.status_opt.SUCCESS
                mock_rpc_result = [{ 'id':item['supervisor_id'], 'quota':[],
                    'auth':ActivationStatus.ACCOUNT_ACTIVATED.value, } for item in body[1:]]
                mock_rpc_result[0]['auth'] = ActivationStatus.ACCOUNT_NON_EXISTENT.value
                reply_event.resp_body['result'] = mock_rpc_result
                mocked_rpc_proxy_call.return_value = reply_event
                response = test_client.post(self.url, headers=headers, json=body)
        assert response.status_code == 400
        result = response.json()
        expect_data = {
            body[0]['supervisor_id'] : {'non-existent user profile', 'unable to login'} ,
            body[1]['supervisor_id'] : {'unable to login'} ,
        }
        for supervisor_id, expect_value in expect_data.items():
            actual_value = result['detail']['supervisor_id'][str(supervisor_id)]
            assert expect_value == set(actual_value)


    def test_invalid_email(self, session_for_test, store_data):
        num_stores = 3
        num_emails_per_store = 2
        invalid_emails = ['xyz@ur873', 'alg0exp3rt.\x05O', 'Alg0@expat@AiOoh', None, '', 'xutye']

    def test_invalid_phone(self, session_for_test, store_data):
        pass

    def test_quota_limit_exceeds(self, keystore, test_client, store_data):
        pass


