import copy
import json
import time
import urllib
import functools
from unittest.mock import Mock, patch

from django.test import TransactionTestCase, Client as DjangoTestClient
from django.db.models import Count
from django.contrib.auth.models import User as AuthUser
from rest_framework.settings import DEFAULTS as drf_default_settings

from common.util.python import sort_nested_object
from common.util.python.messaging.rpc import RpcReplyEvent
from product.permissions import SaleableItemPermissions
from .common import _fixtures as model_fixtures, listitem_rand_assigner, rand_gen_request_body, http_request_body_template, _saleitem_related_instance_setup, HttpRequestDataGenSaleableItem, assert_softdelete_items_exist, SaleableItemVerificationMixin


class _MockInfoMixin:
    stored_models = {}
    _json_mimetype = 'application/json'
    _client = DjangoTestClient(enforce_csrf_checks=False, HTTP_ACCEPT=_json_mimetype)
    _forwarded_pattern = 'by=proxy_api_gateway;for=%s;host=testserver;proto=http'
    mock_profile_id = [123, 124]
    permission_class = None

    def _mock_rpc_succeed_reply_evt(self, succeed_amqp_msg):
        reply_evt = RpcReplyEvent(listener=None, timeout_s=1)
        init_amqp_msg = {'result': {}, 'status': RpcReplyEvent.status_opt.STARTED}
        reply_evt.send(init_amqp_msg)
        succeed_amqp_msg['status'] = RpcReplyEvent.status_opt.SUCCESS
        reply_evt.send(succeed_amqp_msg)
        return reply_evt

    def _mock_get_profile(self, expect_usrprof, http_method):
        mock_role = {'id': 126, 'perm_code': self.permission_class.perms_map[http_method][:]}
        succeed_amqp_msg = {'result': {'id': expect_usrprof, 'roles':[mock_role]},}
        return self._mock_rpc_succeed_reply_evt(succeed_amqp_msg)

    def _send_request_to_backend(self, path, method='post', body=None, expect_shown_fields=None,
            ids=None, extra_query_params=None):
        if body is not None:
            body = json.dumps(body).encode()
        query_params = {}
        if extra_query_params:
            query_params.update(extra_query_params)
        if expect_shown_fields:
            query_params['fields'] = ','.join(expect_shown_fields)
        if ids:
            ids = tuple(map(str, ids))
            query_params['ids'] = ','.join(ids)
        querystrings = urllib.parse.urlencode(query_params)
        path_with_querystring = '%s?%s' % (path, querystrings)
        send_fn = getattr(self._client, method)
        return send_fn(path=path_with_querystring, data=body,  content_type=self._json_mimetype,
                HTTP_FORWARDED=self.http_forwarded)
## end of class _MockInfoMixin


class SaleableItemBaseViewTestCase(TransactionTestCase, _MockInfoMixin, HttpRequestDataGenSaleableItem):
    permission_class = SaleableItemPermissions
    rand_create = True

    def _refresh_req_data(self):
        fixture_source = model_fixtures['ProductSaleableItem']
        if self.rand_create:
            saleitems_data_gen = listitem_rand_assigner(list_=fixture_source)
        else:
            saleitems_data_gen = iter(fixture_source)
        self._request_data = rand_gen_request_body(customize_item_fn=self.customize_req_data_item,
                data_gen=saleitems_data_gen,  template=http_request_body_template['ProductSaleableItem'])
        self._request_data = list(self._request_data)

    def setUp(self):
        self._refresh_req_data()
        # DO NOT create model instance in init() ,
        # in init() the test database hasn't been created yet
        self._user_info = model_fixtures['AuthUser'][0]
        self._account = AuthUser(**self._user_info)
        self._account.set_password(self._user_info['password'])
        self._account.save()
        # for django app, header name has to start with `HTTP_XXXX`
        self.http_forwarded = self._forwarded_pattern % self._user_info['username']

    def tearDown(self):
        self._client.cookies.clear()
        AuthUser.objects.filter(username=self._user_info['username']).delete()
        self.min_num_applied_attrs = 0
        self.min_num_applied_tags = 0
        self.min_num_applied_ingredients = 0

    def _send_request_to_backend(self, path, method='post', body=None, expect_shown_fields=None,
            ids=None, extra_query_params=None, empty_body=False):
        if body is None and not empty_body:
            body = self._request_data
        return super()._send_request_to_backend( path=path, method=method, body=body, ids=ids,
                expect_shown_fields=expect_shown_fields, extra_query_params=extra_query_params )

## end of class SaleableItemBaseViewTestCase


class SaleableItemCreationTestCase(SaleableItemBaseViewTestCase):
    path = '/saleableitems'

    def test_permission_denied(self):
        body = json.dumps(self._request_data).encode()
        # forwarded authentication failure
        response = self._client.post(path=self.path, data=body, content_type=self._json_mimetype)
        self.assertEqual(int(response.status_code), 403)
        # failure because user authentication server is down
        with patch('product.views.base.SaleableItemView._usermgt_rpc.get_profile') as mock_get_profile:
            fake_amqp_msg = {'result': {}, 'status': RpcReplyEvent.status_opt.FAIL_CONN}
            mock_rpc_reply_evt = RpcReplyEvent(listener=None, timeout_s=1)
            mock_rpc_reply_evt.send(fake_amqp_msg)
            mock_get_profile.return_value = mock_rpc_reply_evt
            response = self._client.post(path=self.path, data=body,  content_type=self._json_mimetype,
                    HTTP_FORWARDED=self.http_forwarded)
            self.assertEqual(int(response.status_code), 403)
        # failure due to insufficient permission
        with patch('product.views.base.SaleableItemView._usermgt_rpc.get_profile') as mock_get_profile:
            mock_role = {'id': 126, 'perm_code': SaleableItemPermissions.perms_map['POST'][:-2]}
            succeed_amqp_msg = {'result': {'id': self.mock_profile_id[0], 'roles':[mock_role]},}
            mock_get_profile.return_value = self._mock_rpc_succeed_reply_evt(succeed_amqp_msg)
            response = self._client.post(path=self.path, data=body,  content_type=self._json_mimetype,
                    HTTP_FORWARDED=self.http_forwarded)
            self.assertEqual(int(response.status_code), 403)


    @patch('product.views.base.SaleableItemView._usermgt_rpc.get_profile')
    def test_bulk_ok_with_full_response(self, mock_get_profile):
        _saleitem_related_instance_setup(self.stored_models)
        expect_field_names = ['id', 'usrprof', 'name', 'visible', 'price', 'tags', 'media_set', 'ingredients_applied', 'attributes']
        expect_usrprof = self.mock_profile_id[0]
        mock_get_profile.return_value = self._mock_get_profile(expect_usrprof, 'POST')
        response = self._send_request_to_backend(path=self.path)
        self.assertEqual(int(response.status_code), 201)
        # expect Django backend returns full-size data items
        created_items = json.loads(response.content.decode())
        for created_item in created_items:
            # detail check on all the fields is responsible at serializer level
            for field_name in expect_field_names:
                value = created_item.get(field_name, None)
                self.assertNotEqual(value, None)
            created_id = created_item.get('id')
            self.assertGreater(created_id, 0)
            actual_usrprof = created_item.get('usrprof')
            self.assertEqual(expect_usrprof, actual_usrprof)

    @patch('product.views.base.SaleableItemView._usermgt_rpc.get_profile')
    def test_bulk_ok_with_partial_response(self, mock_get_profile):
        _saleitem_related_instance_setup(self.stored_models)
        expect_shown_fields = ['id', 'name', 'price', 'tags', 'media_set']
        expect_hidden_fields = ['usrprof', 'visible', 'ingredients_applied', 'attributes']
        expect_usrprof = self.mock_profile_id[1]
        mock_get_profile.return_value = self._mock_get_profile(expect_usrprof, 'POST')
        response = self._send_request_to_backend(path=self.path, expect_shown_fields=expect_shown_fields)
        self.assertEqual(int(response.status_code), 201)
        created_items = json.loads(response.content.decode())
        for created_item in created_items:
            for field_name in expect_shown_fields:
                value = created_item.get(field_name, None)
                self.assertNotEqual(value, None)
            for field_name in expect_hidden_fields:
                value = created_item.get(field_name, None)
                self.assertEqual(value, None)


    @patch('product.views.base.SaleableItemView._usermgt_rpc.get_profile')
    def test_validation_error_unclassified_attributes(self, mock_get_profile):
        self.min_num_applied_attrs = 1
        self._refresh_req_data()
        mock_get_profile.return_value = self._mock_get_profile(self.mock_profile_id[1], 'POST')
        response = self._send_request_to_backend(path=self.path)
        self.assertEqual(int(response.status_code), 400)
        err_items = json.loads(response.content.decode())
        expect_items_iter = iter(self._request_data)
        for err_item in err_items:
            self.assertListEqual(['attributes'], list(err_item.keys()))
            expect_item = next(expect_items_iter)
            err_msg_pattern = 'unclassified attribute type `%s`'
            expect_err_msgs = map(lambda x: err_msg_pattern % x['type'], expect_item['attributes'])
            actual_err_msgs = map(lambda x: x['type'][0], err_item['attributes'])
            expect_err_msgs = list(expect_err_msgs)
            actual_err_msgs = list(actual_err_msgs)
            self.assertGreater(len(actual_err_msgs), 0)
            self.assertListEqual(expect_err_msgs, actual_err_msgs)


    @patch('product.views.base.SaleableItemView._usermgt_rpc.get_profile')
    def test_validation_error_unknown_references(self, mock_get_profile):
        _saleitem_related_instance_setup(self.stored_models, num_tags=0, num_ingredients=0)
        self.min_num_applied_tags = 1
        self.min_num_applied_ingredients = 1
        self._refresh_req_data()
        mock_get_profile.return_value = self._mock_get_profile(self.mock_profile_id[1], 'POST')
        response = self._send_request_to_backend(path=self.path)
        self.assertEqual(int(response.status_code), 400)
        err_items = json.loads(response.content.decode())
        for err_item in err_items:
            self.assertNotEqual(err_item.get('tags'), None)
            self.assertGreaterEqual(len(err_item['tags']), 1)
            patt_pos = err_item['tags'][0].find('object does not exist')
            self.assertGreater(patt_pos, 0)
            self.assertNotEqual(err_item.get('ingredients_applied'), None)
            self.assertGreaterEqual(len(err_item['ingredients_applied']), 1)
            err_msg = err_item['ingredients_applied'][0]['ingredient'][0]
            patt_pos = err_msg.startswith('object does not exist')
            self.assertGreaterEqual(patt_pos, 0)
## end of class SaleableItemCreationTestCase


class SaleableItemUpdateBaseTestCase(SaleableItemBaseViewTestCase):
    path = '/saleableitems'

    def setUp(self):
        super().setUp()
        _saleitem_related_instance_setup(self.stored_models)
        self._created_items = None
        expect_shown_fields = ['id', 'name',]
        self.expect_usrprof = self.mock_profile_id[0]
        with patch('product.views.base.SaleableItemView._usermgt_rpc.get_profile') as mock_get_profile:
            mock_get_profile.return_value = self._mock_get_profile(self.expect_usrprof, 'POST')
            response = self._send_request_to_backend(path=self.path, method='post',
                    expect_shown_fields=expect_shown_fields)
            self.assertEqual(int(response.status_code), 201)
            self._created_items = json.loads(response.content.decode())


class SaleableItemUpdateTestCase(SaleableItemUpdateBaseTestCase):
    @patch('product.views.base.SaleableItemView._usermgt_rpc.get_profile')
    def test_invalid_id(self, mock_get_profile):
        key = drf_default_settings['NON_FIELD_ERRORS_KEY']
        edit_data = copy.deepcopy(self._request_data) # edit data without corrent ID
        mock_get_profile.return_value = self._mock_get_profile(self.expect_usrprof, 'PUT')
        # sub case: lack id
        edit_data[0].pop('id',None)
        response = self._send_request_to_backend(path=self.path, method='put', body=edit_data)
        err_items = json.loads(response.content.decode())
        self.assertEqual(int(response.status_code), 403)
        # sub case: invalid data type of id
        edit_data[0]['id'] = 99999
        edit_data[-1]['id'] = 'string_id'
        response = self._send_request_to_backend(path=self.path, method='put', body=edit_data)
        err_items = json.loads(response.content.decode())
        self.assertEqual(int(response.status_code), 403)
        # sub case: mix of valid id and invalid id
        edit_data[0]['id'] = 'wrong_id'
        edit_data[-1]['id'] = self._created_items[0]['id']
        response = self._send_request_to_backend(path=self.path, method='put', body=edit_data)
        err_items = response.json()
        self.assertEqual(int(response.status_code), 403)


    def _rand_gen_edit_data(self):
        edit_data_iter = iter(self._request_data)
        created_ids_gen = listitem_rand_assigner(list_=self._created_items,
                max_num_chosen=(len(self._created_items) + 1))
        edit_data = []
        for item in created_ids_gen: # shuffle the list of valid ID then send edit data
            edit_item = next(edit_data_iter)
            edit_item = copy.deepcopy(edit_item)
            edit_item['id'] = item['id']
            edit_data.append(edit_item)
        return edit_data


    @patch('product.views.base.SaleableItemView._usermgt_rpc.get_profile')
    def test_bulk_ok(self, mock_get_profile):
        expect_shown_fields = ['id', 'name', 'price', 'tags', 'ingredients_applied']
        mock_get_profile.return_value = self._mock_get_profile(self.expect_usrprof, 'PUT')
        for _ in range(10):
            edit_data = self._rand_gen_edit_data()
            response = self._send_request_to_backend(path=self.path, method='put',
                    body=edit_data,  expect_shown_fields=expect_shown_fields)
            edited_items = response.json()
            self.assertEqual(int(response.status_code), 200)
            fn = lambda x:{key:x[key] for key in expect_shown_fields}
            expect_edited_items = list(map(fn, edit_data))
            actual_edited_items = list(map(fn, edited_items))
            expect_edited_items = sort_nested_object(obj=expect_edited_items)
            actual_edited_items = sort_nested_object(obj=actual_edited_items)
            self.assertListEqual(expect_edited_items, actual_edited_items)


    @patch('product.views.base.SaleableItemView._usermgt_rpc.get_profile')
    def test_permission_denied(self, mock_get_profile):
        another_usrprof = self.mock_profile_id[1]
        edit_data = copy.deepcopy(self._request_data[:1])
        edit_data[0]['id'] = self._created_items[0]['id']
        mock_get_profile.return_value = self._mock_get_profile(another_usrprof, 'PUT')
        response = self._send_request_to_backend(path=self.path, method='put', body=edit_data,)
        edited_items = response.json()
        self.assertEqual(int(response.status_code), 403)

    @patch('product.views.base.SaleableItemView._usermgt_rpc.get_profile')
    def test_conflict_item_error(self, mock_get_profile):
        key = drf_default_settings['NON_FIELD_ERRORS_KEY']
        edit_data = self._rand_gen_edit_data()
        discarded_id  = edit_data[0]['id']
        edit_data[0]['id'] = edit_data[1]['id']
        mock_get_profile.return_value = self._mock_get_profile(self.expect_usrprof, 'PUT')
        response = self._send_request_to_backend(path=self.path, method='put', body=edit_data,)
        self.assertEqual(int(response.status_code), 400)
        err_info = response.json()
        err_msg = err_info[key][0]
        pos = err_msg.find('duplicate item found in the list')
        self.assertGreaterEqual(pos, 0)
        pos = err_msg.find(str(discarded_id))
        self.assertLess(pos, 0)
## end of class SaleableItemUpdateTestCase


class SaleableItemDeletionTestCase(SaleableItemUpdateBaseTestCase, SaleableItemVerificationMixin):
    @patch('product.views.base.SaleableItemView._usermgt_rpc.get_profile')
    def test_softdelete_permission_denied(self, mock_get_profile):
        another_usrprof = self.mock_profile_id[1]
        mock_get_profile.return_value = self._mock_get_profile(another_usrprof, 'DELETE')
        deleted_ids = list(map(lambda x: {'id':x['id']}, self._created_items))
        response = self._send_request_to_backend(path=self.path, method='delete', body=deleted_ids)
        self.assertEqual(int(response.status_code), 403)

    @patch('product.views.base.SaleableItemView._usermgt_rpc.get_profile')
    def test_softdelete_ok(self, mock_get_profile):
        num_delete = 2
        mock_get_profile.return_value = self._mock_get_profile(self.expect_usrprof, 'DELETE')
        deleted_ids = list(map(lambda x: {'id':x['id']}, self._created_items[:num_delete]))
        response = self._send_request_to_backend(path=self.path, method='delete', body=deleted_ids)
        self.assertEqual(int(response.status_code), 202)
        deleted_ids = list(map(lambda x: x['id'], self._created_items[:num_delete]))
        remain_ids  = list(map(lambda x: x['id'], self._created_items[num_delete:]))
        assert_softdelete_items_exist(testcase=self, deleted_ids=deleted_ids, remain_ids=remain_ids,
                model_cls_path='product.models.base.ProductSaleableItem',)


    def _softdelete_one_by_one(self, remain_items, deleted_items, delay_interval_sec=0):
        while any(remain_items):
            chosen_item = remain_items.pop()
            response = self._send_request_to_backend(path=self.path, method='delete',
                    body=[{'id': chosen_item['id']}])
            self.assertEqual(int(response.status_code), 202)
            deleted_items.append(chosen_item)  # insert(0, chosen_item)
            deleted_ids = list(map(lambda x: x['id'], deleted_items))
            remain_ids  = list(map(lambda x: x['id'], remain_items ))
            assert_softdelete_items_exist(testcase=self, deleted_ids=deleted_ids, remain_ids=remain_ids,
                    model_cls_path='product.models.base.ProductSaleableItem',)
            print('.', end='')
            time.sleep(delay_interval_sec)

    def _undelete_one(self, body={}, expect_resp_status=200, expect_resp_msg='recovery done'):
        # note that body has to be at least {} or [], must not be null
        # because the content-type is json 
        empty_body = body is None
        response = self._send_request_to_backend( path=self.path, method='patch',
                body=body, empty_body=empty_body)
        response_body = response.json()
        self.assertEqual(int(response.status_code), expect_resp_status)
        self.assertEqual(response_body['message'][0], expect_resp_msg)
        if expect_resp_status != 200:
            return
        undeleted_items = response_body['affected_items']
        self.assertGreaterEqual(len(undeleted_items), 1)
        undeleted_items = sorted(undeleted_items, key=lambda x:x['id'])
        undeleted_ids  = tuple(map(lambda x:x['id'], undeleted_items))
        undeleted_objs = self.serializer_class.Meta.model.objects.filter(
                id__in=undeleted_ids).order_by('id')
        self.verify_objects(actual_instances=undeleted_objs, expect_data=undeleted_items,
                usrprof_id=self.expect_usrprof)
        return undeleted_items

    @patch('product.views.base.SaleableItemView._usermgt_rpc.get_profile')
    def test_undelete_by_time(self, mock_get_profile):
        remain_items  = copy.copy(self._created_items)
        deleted_items = []
        # soft-delete one after another
        for _ in range(5):
            mock_get_profile.return_value = self._mock_get_profile(self.expect_usrprof, 'DELETE')
            self._softdelete_one_by_one(remain_items, deleted_items, delay_interval_sec=2)
            # recover one by one based on the time at which each item was soft-deleted
            mock_get_profile.return_value = self._mock_get_profile(self.expect_usrprof, 'PATCH')
            while any(deleted_items):
                chosen_item = deleted_items.pop()
                undeleted_items = self._undelete_one()
                self.assertEqual(chosen_item['id'] , undeleted_items[0]['id'])
                remain_items.append(chosen_item)
            self.assertListEqual(self._created_items, remain_items)


    @patch('product.views.base.SaleableItemView._usermgt_rpc.get_profile')
    def test_undelete_specific_item(self, mock_get_profile):
        remain_items  = copy.copy(self._created_items)
        deleted_items = []
        for _ in range(5):
            mock_get_profile.return_value = self._mock_get_profile(self.expect_usrprof, 'DELETE')
            self._softdelete_one_by_one(remain_items, deleted_items, delay_interval_sec=0)
            mock_get_profile.return_value = self._mock_get_profile(self.expect_usrprof, 'PATCH')
            undeleting_items_gen = listitem_rand_assigner(list_=deleted_items, min_num_chosen=len(deleted_items),
                    max_num_chosen=(len(deleted_items) + 1))
            undeleting_items = list(undeleting_items_gen)
            half = len(undeleting_items) >> 1
            body = {'ids':[x['id'] for x in undeleting_items[:half]]}
            affected_items = self._undelete_one(body=body)
            body = {'ids':[x['id'] for x in undeleting_items[half:]]}
            affected_items = self._undelete_one(body=body)
            remain_items, deleted_items = deleted_items, remain_items # internally swap 2 lists


    @patch('product.views.base.SaleableItemView._usermgt_rpc.get_profile')
    def test_no_softdeleted_item(self, mock_get_profile):
        mock_get_profile.return_value = self._mock_get_profile(self.expect_usrprof, 'PATCH')
        kwargs = {'expect_resp_status':410, 'expect_resp_msg':'Nothing recovered'}
        self._undelete_one(**kwargs)
        remain_items = self._created_items[:-1]
        kwargs['body'] = {'ids':[x['id'] for x in remain_items]}
        self._undelete_one(**kwargs)


    @patch('product.views.base.SaleableItemView._usermgt_rpc.get_profile')
    def test_undelete_permission_denied(self, mock_get_profile):
        remain_items  = copy.copy(self._created_items)
        deleted_items = []
        mock_get_profile.return_value = self._mock_get_profile(self.expect_usrprof, 'DELETE')
        self._softdelete_one_by_one(remain_items, deleted_items, delay_interval_sec=0)
        self.assertGreater(len(deleted_items), 0)
        another_usrprof = self.mock_profile_id[1]
        mock_get_profile.return_value = self._mock_get_profile(another_usrprof, 'PATCH')
        kwargs = {'expect_resp_status':403, 'expect_resp_msg':'user is not allowed to undelete the item(s)'}
        kwargs['body'] = {'ids':[x['id'] for x in deleted_items]}
        self._undelete_one(**kwargs)

    @patch('product.views.base.SaleableItemView._usermgt_rpc.get_profile')
    def test_undelete_with_invalid_id(self, mock_get_profile):
        mock_get_profile.return_value = self._mock_get_profile(self.expect_usrprof, 'PATCH')
        kwargs = {'expect_resp_status':400, 'expect_resp_msg':'invalid data in request body'}
        kwargs['body'] = {'ids':['wrong_id', 'fake_id']}
        self._undelete_one(**kwargs)
## end of class SaleableItemDeletionTestCase


class SaleableItemQueryTestCase(SaleableItemUpdateBaseTestCase, SaleableItemVerificationMixin):

    @patch('product.views.base.SaleableItemView._usermgt_rpc.get_profile')
    def test_bulk_items_full_info(self, mock_get_profile):
        mock_get_profile.return_value = self._mock_get_profile(self.expect_usrprof, 'GET')
        created_ids = tuple(map(lambda x:x['id'], self._created_items))
        response = self._send_request_to_backend(path=self.path, method='get', ids=created_ids, empty_body=True)
        actual_items = response.json()
        expect_items = self.serializer_class.Meta.model.objects.filter(id__in=created_ids)
        self.assertEqual(int(response.status_code), 200)
        self.assertEqual(len(self._created_items), len(actual_items))
        self.verify_objects(actual_instances=expect_items , expect_data=actual_items)


    @patch('product.views.base.SaleableItemView._usermgt_rpc.get_profile')
    def test_bulk_items_partial_info(self, mock_get_profile):
        expect_shown_fields = ['id','name','media_set','ingredients_applied']
        mock_get_profile.return_value = self._mock_get_profile(self.expect_usrprof, 'GET')
        created_ids = tuple(map(lambda x:x['id'], self._created_items))
        response = self._send_request_to_backend(path=self.path, method='get',
                expect_shown_fields=expect_shown_fields, ids=created_ids, empty_body=True)
        actual_items = response.json()
        expect_items = self.serializer_class.Meta.model.objects.filter(id__in=created_ids)
        self.assertEqual(int(response.status_code), 200)
        self.assertEqual(len(self._created_items), len(actual_items))
        actual_items_iter = iter(actual_items)
        for expect_item in expect_items:
            actual_item = next(actual_items_iter)
            self._assert_simple_fields(check_fields=['id','name'], exp_sale_item=expect_item,  ac_sale_item=actual_item)
            self._assert_mediaset_fields(exp_sale_item=actual_item, ac_sale_item=expect_item)
            self._assert_ingredients_applied_fields(exp_sale_item=actual_item, ac_sale_item=expect_item)


    @patch('product.views.base.SaleableItemView._usermgt_rpc.get_profile')
    def test_single_items_full_info(self, mock_get_profile):
        mock_get_profile.return_value = self._mock_get_profile(self.expect_usrprof, 'GET')
        path_single_item = '/saleableitem/%s'
        for created_item in self._created_items:
            path_with_id = path_single_item % created_item['id']
            response = self._send_request_to_backend(path=path_with_id, method='get', empty_body=True)
            actual_item = response.json()
            expect_item = self.serializer_class.Meta.model.objects.get(id=created_item['id'])
            self.assertEqual(int(response.status_code), 200)
            self.assertEqual(created_item['id'], actual_item['id'])
            self.verify_objects(actual_instances=[expect_item], expect_data=[actual_item])

    @patch('product.views.base.SaleableItemView._usermgt_rpc.get_profile')
    def test_order_by(self, mock_get_profile):
        pass



class SaleableItemSearchFilterTestCase(SaleableItemUpdateBaseTestCase, SaleableItemVerificationMixin):
    rand_create = False
    _model_cls = SaleableItemVerificationMixin.serializer_class.Meta.model

    @patch('product.views.base.SaleableItemView._usermgt_rpc.get_profile')
    def test_simple_search(self, mock_get_profile):
        mock_get_profile.return_value = self._mock_get_profile(self.expect_usrprof, 'GET')
        keywords = ['noodle', 'PI', 'Pi', 'Dev board']
        for keyword in keywords:
            extra_query_params = {'search':keyword}
            response = self._send_request_to_backend(path=self.path, method='get',
                    extra_query_params=extra_query_params, empty_body=True)
            actual_items = response.json()
            icase_search_fn = lambda x: keyword.lower() in x['name'].lower()
            expect_items = tuple(filter(icase_search_fn, self._created_items))
            self.assertEqual(int(response.status_code), 200)
            self.assertEqual(len(actual_items), len(expect_items))
            actual_items = sorted(actual_items, key=lambda x:x['name'])
            expect_items = sorted(expect_items, key=lambda x:x['name'])
            actual_items_iter = iter(actual_items)
            for expect_item in expect_items:
                actual_item = next(actual_items_iter)
                self._assert_simple_fields(check_fields=['id','name'], exp_sale_item=expect_item,
                        ac_sale_item=actual_item)


    def _test_advanced_search_common(self, adv_cond):
        extra_query_params = {'advanced_search': 'yes', 'adv_search_cond': json.dumps(adv_cond)}
        response = self._send_request_to_backend(path=self.path, method='get',
                extra_query_params=extra_query_params, empty_body=True)
        actual_items = response.json()
        self.assertEqual(int(response.status_code), 200)
        self.assertGreaterEqual(len(actual_items), 1)
        return actual_items

    @patch('product.views.base.SaleableItemView._usermgt_rpc.get_profile')
    def test_advanced_search_attr_str(self, mock_get_profile):
        # to ensure there will be at least 2 attribute values assigned to each saleable item
        self.min_num_applied_attrs = 2
        mock_get_profile.return_value = self._mock_get_profile(self.expect_usrprof, 'GET')
        # construct search condition based on existing created saleable items, in
        # order to ensure there is always something returned
        qset = self._model_cls.objects.annotate(num_str_attr=Count('attr_val_str')).filter(num_str_attr__gt=0)
        for expect_obj in qset:
            for attrval in expect_obj.attr_val_str.all():
                adv_cond = {
                    'operator': 'and',
                    'operands':[
                        {
                            'operator':'==',
                            'operands':['attributes__type', attrval.attr_type.pk ],
                            'metadata':{'dtype': attrval.attr_type.dtype}
                        },
                        {
                            'operator':'contains',
                            'operands':['attributes__value', attrval.value ],
                            'metadata':{'dtype': attrval.attr_type.dtype}
                        }
                    ]
                }  ## end of adv_cond
                self._test_advanced_search_common(adv_cond=adv_cond)


    @patch('product.views.base.SaleableItemView._usermgt_rpc.get_profile')
    def test_advanced_search_attr_int(self, mock_get_profile):
        self.min_num_applied_attrs = 2
        mock_get_profile.return_value = self._mock_get_profile(self.expect_usrprof, 'GET')
        variation = 100
        qset = self._model_cls.objects.annotate(num_int_attr=Count('attr_val_int')).filter(num_int_attr__gt=0)
        for expect_obj in qset:
            for attrval in expect_obj.attr_val_int.all():
                upper_bound = attrval.value + variation
                lower_bound = attrval.value - variation
                adv_cond = {
                    'operator': 'and',
                    'operands':[
                        {
                            'operator':'==',
                            'operands':['attributes__type', attrval.attr_type.pk ],
                            'metadata':{'dtype': attrval.attr_type.dtype}
                        },
                        {
                            'operator': 'and',
                            'operands':[
                                {
                                    'operator':'>',
                                    'operands':['attributes__value', lower_bound],
                                    'metadata':{'dtype': attrval.attr_type.dtype}
                                },
                                {
                                    'operator':'<',
                                    'operands':['attributes__value', upper_bound],
                                    'metadata':{'dtype': attrval.attr_type.dtype}
                                }
                            ]
                        }
                    ]
                }  ## end of adv_cond
                actual_items = self._test_advanced_search_common(adv_cond=adv_cond)
                search_result = tuple(filter(lambda x: x['id'] == expect_obj.id, actual_items))
                self.assertEqual(len(search_result), 1)


    @patch('product.views.base.SaleableItemView._usermgt_rpc.get_profile')
    def test_advanced_search_attr_float(self, mock_get_profile):
        self.min_num_applied_attrs = 1
        mock_get_profile.return_value = self._mock_get_profile(self.expect_usrprof, 'GET')
        qset = self._model_cls.objects.annotate(num_float_attr=Count('attr_val_float')).filter(num_float_attr__gt=0)
        qset = qset.values('id', 'attr_val_float__attr_type__id', 'attr_val_float__attr_type__dtype', 'attr_val_float__value')
        data = {v['id']: v  for v in qset}
        adv_cond = {
            'operator': 'or',
            'operands': []
        }
        variation = 1.0
        for d in data.values():
            upper_bound = d['attr_val_float__value'] + variation
            lower_bound = d['attr_val_float__value'] - variation
            adv_cond_sub_clause = {
                'operator': 'and',
                'operands':[
                    {
                        'operator':'==',
                        'operands':['attributes__type', d['attr_val_float__attr_type__id']],
                        'metadata':{'dtype': d['attr_val_float__attr_type__dtype']}
                    },
                    {
                        'operator': 'and',
                        'operands':[
                            {
                                'operator':'>=',
                                'operands':['attributes__value', lower_bound],
                                'metadata':{'dtype': d['attr_val_float__attr_type__dtype']}
                            },
                            {
                                'operator':'<=',
                                'operands':['attributes__value', upper_bound],
                                'metadata':{'dtype': d['attr_val_float__attr_type__dtype']}
                            }
                        ]
                    }
                ]
            }
            adv_cond['operands'].append(adv_cond_sub_clause)
        saleitems_found = self._test_advanced_search_common(adv_cond=adv_cond)
        # TODO, the search condition will make Django backend return duplicate data
        # figure out how it works and how to solve the problem


    @patch('product.views.base.SaleableItemView._usermgt_rpc.get_profile')
    def test_advanced_search_tags(self, mock_get_profile):
        pass


    @patch('product.views.base.SaleableItemView._usermgt_rpc.get_profile')
    def test_advanced_search_ingredients_applied(self, mock_get_profile):
        pass


    @patch('product.views.base.SaleableItemView._usermgt_rpc.get_profile')
    def test_advanced_nested_search_mix(self, mock_get_profile):
        pass


