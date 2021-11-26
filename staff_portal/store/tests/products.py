import random
from unittest.mock import patch

import pytest

from common.models.constants  import ROLE_ID_STAFF
from common.models.enums.base import AppCodeOptions, ActivationStatus
from common.util.python.messaging.rpc import RpcReplyEvent

from store.models import SaleableTypeEnum, StoreProductAvailable
from store.tests.common import db_engine_resource, session_for_test, session_for_setup, keystore, test_client, store_data, email_data, phone_data, loc_data, opendays_data, staff_data, product_avail_data, saved_store_objs

app_code = AppCodeOptions.store.value[0]

class TestUpdate:
    url = '/profile/{store_id}/products'
    _auth_data_pattern = { 'id':-1, 'privilege_status':ROLE_ID_STAFF,
        'quotas': [{'app_code':app_code, 'mat_code': StoreProductAvailable.quota_material.value, 'maxnum':-1}] ,
        'roles':[
            {'app_code':app_code, 'codename':'add_storeproductavailable'},
            {'app_code':app_code, 'codename':'change_storeproductavailable'},
            {'app_code':app_code, 'codename':'delete_storeproductavailable'},
        ],
    }

    def _mocked_rpc_reply_refresh(self, *args, **kwargs):
        # skip receiving message from RPC-reply-queue
        pass

    @patch('common.util.python.messaging.rpc.RpcReplyEvent.refresh', _mocked_rpc_reply_refresh)
    def test_ok(self, session_for_test, keystore, test_client, saved_store_objs, product_avail_data):
        obj = next(saved_store_objs)
        body = [{'product_id':p.product_id, 'product_type':p.product_type.value, 'start_after':p.start_after.isoformat(),
            'end_before':p.end_before.isoformat()} for p in obj.products[2:]]
        new_product_d = [next(product_avail_data) for _ in range(2)]
        for item in new_product_d:
            item['product_type'] = item['product_type'].value
            item['start_after'] = item['start_after'].isoformat()
            item['end_before']  = item['end_before'].isoformat()
        body.extend(new_product_d)
        auth_data = self._auth_data_pattern
        # authorized user can be either supervisor or staff of the store
        auth_data['id'] = obj.staff[-1].staff_id
        auth_data['quotas'][0]['maxnum'] = len(body)
        encoded_token = keystore.gen_access_token(profile=auth_data, audience=['store'])
        headers = {'Authorization': 'Bearer %s' % encoded_token}
        url = self.url.format(store_id=obj.id)
        sale_items_d = filter(lambda d:d['product_type'] == SaleableTypeEnum.ITEM.value, body)
        sale_pkgs_d  = filter(lambda d:d['product_type'] == SaleableTypeEnum.PACKAGE.value, body)
        sale_items_d = map(lambda d:{'id':d['product_id']}, sale_items_d)
        sale_pkgs_d  = map(lambda d:{'id':d['product_id']}, sale_pkgs_d )
        reply_event = RpcReplyEvent(listener=self, timeout_s=7)
        reply_event.resp_body['status'] = RpcReplyEvent.status_opt.SUCCESS
        reply_event.resp_body['result'] = {'item':list(sale_items_d), 'pkg':list(sale_pkgs_d),}
        with patch('jwt.PyJWKClient.fetch_data', keystore._mocked_get_jwks):
            with patch('common.util.python.messaging.rpc.MethodProxy._call') as mocked_rpc_proxy_call:
                mocked_rpc_proxy_call.return_value = reply_event
                response = test_client.patch(url, headers=headers, json=body)
        assert response.status_code == 200
        query = session_for_test.query(StoreProductAvailable).filter(StoreProductAvailable.store_id == obj.id)
        query = query.order_by(StoreProductAvailable.product_id.asc())
        expect_value = sorted(body, key=lambda d:d['product_id'])
        actual_value = list(map(lambda obj:obj.__dict__, query.all()))
        for item in actual_value:
            item.pop('_sa_instance_state', None)
            item.pop('store_id', None)
            item['product_type'] = item['product_type'].value
            item['start_after'] = item['start_after'].isoformat()
            item['end_before']  = item['end_before'].isoformat()
        assert expect_value == actual_value


    @patch('common.util.python.messaging.rpc.RpcReplyEvent.refresh', _mocked_rpc_reply_refresh)
    def test_invalid_product_id(self, session_for_test, keystore, test_client, saved_store_objs):
        obj = next(saved_store_objs)
        body = [{'product_id':p.product_id, 'product_type':p.product_type.value, 'start_after':p.start_after.isoformat(),
            'end_before':p.end_before.isoformat()} for p in obj.products]
        auth_data = self._auth_data_pattern
        auth_data['id'] = obj.staff[0].staff_id
        auth_data['quotas'][0]['maxnum'] = len(body)
        encoded_token = keystore.gen_access_token(profile=auth_data, audience=['store'])
        headers = {'Authorization': 'Bearer %s' % encoded_token}
        url = self.url.format(store_id=obj.id)
        sale_items_d = filter(lambda d:d['product_type'] == SaleableTypeEnum.ITEM.value, body[1:])
        sale_pkgs_d  = filter(lambda d:d['product_type'] == SaleableTypeEnum.PACKAGE.value, body[1:])
        sale_items_d = map(lambda d:{'id':d['product_id']}, sale_items_d)
        sale_pkgs_d  = map(lambda d:{'id':d['product_id']}, sale_pkgs_d )
        reply_event = RpcReplyEvent(listener=self, timeout_s=7)
        reply_event.resp_body['status'] = RpcReplyEvent.status_opt.SUCCESS
        reply_event.resp_body['result'] = {'item':list(sale_items_d), 'pkg':list(sale_pkgs_d),}
        with patch('jwt.PyJWKClient.fetch_data', keystore._mocked_get_jwks):
            with patch('common.util.python.messaging.rpc.MethodProxy._call') as mocked_rpc_proxy_call:
                mocked_rpc_proxy_call.return_value = reply_event
                response = test_client.patch(url, headers=headers, json=body)
        assert response.status_code == 400
        result = response.json()
        assert result['detail']['code'] == 'invalid'
        err_detail = result['detail']['field']
        assert err_detail and any(err_detail)
        expect_value = {k:body[0].get(k) for k in ('product_id','product_type')}
        actual_value = err_detail[0]
        assert expect_value == actual_value


    @patch('common.util.python.messaging.rpc.RpcReplyEvent.refresh', _mocked_rpc_reply_refresh)
    def test_invalid_staff_id(self, session_for_test, keystore, test_client, saved_store_objs):
        invalid_staff_id = 9999
        obj = next(saved_store_objs)
        body = [{'product_id':p.product_id, 'product_type':p.product_type.value, 'start_after':p.start_after.isoformat(),
            'end_before':p.end_before.isoformat()} for p in obj.products]
        auth_data = self._auth_data_pattern
        auth_data['id'] = invalid_staff_id
        auth_data['quotas'][0]['maxnum'] = len(body)
        encoded_token = keystore.gen_access_token(profile=auth_data, audience=['store'])
        headers = {'Authorization': 'Bearer %s' % encoded_token}
        url = self.url.format(store_id=obj.id)
        sale_items_d = filter(lambda d:d['product_type'] == SaleableTypeEnum.ITEM.value, body[:])
        sale_pkgs_d  = filter(lambda d:d['product_type'] == SaleableTypeEnum.PACKAGE.value, body[:])
        sale_items_d = map(lambda d:{'id':d['product_id']}, sale_items_d)
        sale_pkgs_d  = map(lambda d:{'id':d['product_id']}, sale_pkgs_d )
        reply_event = RpcReplyEvent(listener=self, timeout_s=7)
        reply_event.resp_body['status'] = RpcReplyEvent.status_opt.SUCCESS
        reply_event.resp_body['result'] = {'item':list(sale_items_d), 'pkg':list(sale_pkgs_d),}
        with patch('jwt.PyJWKClient.fetch_data', keystore._mocked_get_jwks):
            with patch('common.util.python.messaging.rpc.MethodProxy._call') as mocked_rpc_proxy_call:
                mocked_rpc_proxy_call.return_value = reply_event
                response = test_client.patch(url, headers=headers, json=body)
        assert response.status_code == 403



