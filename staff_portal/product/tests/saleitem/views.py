import copy
import json
import time
import functools
from unittest.mock import Mock, patch

from django.test import TransactionTestCase
from django.db.models import Count, Q
from django.db.models.constants import LOOKUP_SEP
from rest_framework.settings import DEFAULTS as drf_default_settings

from common.util.python import sort_nested_object
from common.util.python.messaging.rpc import RpcReplyEvent
from product.permissions import SaleableItemPermissions

from product.tests.common import _MockTestClientInfoMixin, assert_view_permission_denied, listitem_rand_assigner, rand_gen_request_body, http_request_body_template, assert_view_bulk_create_with_response, assert_view_unclassified_attributes, SoftDeleteCommonTestMixin

from ..common import app_code_product, priv_status_staff
from  .common import _fixtures as model_fixtures, _saleitem_related_instance_setup, HttpRequestDataGenSaleableItem, SaleableItemVerificationMixin


class SaleableItemBaseViewTestCase(TransactionTestCase, _MockTestClientInfoMixin, HttpRequestDataGenSaleableItem):
    permission_class = SaleableItemPermissions

    def refresh_req_data(self):
        return super().refresh_req_data(fixture_source=model_fixtures['ProductSaleableItem'],
                http_request_body_template=http_request_body_template['ProductSaleableItem'])

    def setUp(self):
        self._setup_keystore()
        self._request_data = self.refresh_req_data()
        # DO NOT create model instance in init() ,
        # in init() the test database hasn't been created yet

    def tearDown(self):
        self._teardown_keystore()
        self._client.cookies.clear()
        self.min_num_applied_attrs = 0
        self.min_num_applied_tags = 0
        self.min_num_applied_ingredients = 0
## end of class SaleableItemBaseViewTestCase


class SaleableItemCreationTestCase(SaleableItemBaseViewTestCase):
    path = '/saleableitems'

    def test_permission_denied(self):
        assert_view_permission_denied( testcase=self, request_body_data=self._request_data, http_method='post',
                path=self.path, permissions=['view_productsaleableitem', 'add_productsaleableitem'] )

    def test_bulk_ok_with_full_response(self):
        _saleitem_related_instance_setup(self.stored_models)
        expect_field_names = ['id', 'usrprof', 'name', 'visible', 'price', 'tags', 'media_set', 'ingredients_applied', 'attributes']
        expect_usrprof = 71
        permissions = ['view_productsaleableitem', 'add_productsaleableitem']
        access_tok_payld = { 'id':expect_usrprof, 'privilege_status': priv_status_staff, 'quotas':[],
            'roles':[{'app_code':app_code_product, 'codename':codename} for codename in permissions] }
        access_token = self.gen_access_token(profile=access_tok_payld, audience=['product'])
        response = self._send_request_to_backend(path=self.path, body=self._request_data, access_token=access_token)
        self.assertEqual(int(response.status_code), 201)
        # expect Django backend returns full-size data items
        created_items = response.json()
        for created_item in created_items:
            # detail check on all the fields is responsible at serializer level
            for field_name in expect_field_names:
                value = created_item.get(field_name, None)
                self.assertNotEqual(value, None)
            created_id = created_item.get('id')
            self.assertGreater(created_id, 0)
            actual_usrprof = created_item.get('usrprof')
            self.assertEqual(expect_usrprof, actual_usrprof)


    def test_bulk_ok_with_partial_response(self):
        _saleitem_related_instance_setup(self.stored_models)
        expect_shown_fields = ['id', 'name', 'price', 'tags', 'media_set']
        expect_hidden_fields = ['usrprof', 'visible', 'ingredients_applied', 'attributes']
        assert_view_bulk_create_with_response(testcase=self, expect_shown_fields=expect_shown_fields,
            permissions=['view_productsaleableitem', 'add_productsaleableitem'],
            expect_hidden_fields=expect_hidden_fields, path=self.path, body=self._request_data )


    def test_validation_error_unclassified_attributes(self):
        # don't create any attribute type instance in this case
        self.min_num_applied_attrs = 1
        new_request_data = self.refresh_req_data()
        assert_view_unclassified_attributes(testcase=self, path=self.path, body=new_request_data,
            permissions=['view_productsaleableitem', 'add_productsaleableitem'] )


    def test_validation_error_unknown_references(self):
        _saleitem_related_instance_setup(self.stored_models, num_tags=0, num_ingredients=0)
        self.min_num_applied_tags = 1
        self.min_num_applied_ingredients = 1
        request_data = self.refresh_req_data()
        expect_usrprof = 71
        permissions = ['view_productsaleableitem', 'add_productsaleableitem']
        access_tok_payld = { 'id':expect_usrprof, 'privilege_status': priv_status_staff, 'quotas':[],
            'roles':[{'app_code':app_code_product, 'codename':codename} for codename in permissions] }
        access_token = self.gen_access_token(profile=access_tok_payld, audience=['product'])
        response = self._send_request_to_backend(path=self.path, body=request_data, access_token=access_token)
        self.assertEqual(int(response.status_code), 400)
        err_items = response.json()
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
        expect_usrprof = 71
        permissions = ['view_productsaleableitem', 'add_productsaleableitem']
        access_tok_payld = { 'id':expect_usrprof, 'privilege_status': priv_status_staff, 'quotas':[],
            'roles':[{'app_code':app_code_product, 'codename':codename} for codename in permissions] }
        access_token = self.gen_access_token(profile=access_tok_payld, audience=['product'])
        response = self._send_request_to_backend(path=self.path, method='post', body=self._request_data,
                access_token=access_token,  expect_shown_fields=expect_shown_fields)
        self.assertEqual(int(response.status_code), 201)
        self._created_items = response.json()


class SaleableItemUpdateTestCase(SaleableItemUpdateBaseTestCase, SaleableItemVerificationMixin):
    def setUp(self):
        super().setUp()
        expect_usrprof = 71
        permissions = ['view_productsaleableitem', 'change_productsaleableitem']
        access_tok_payld = { 'id':expect_usrprof, 'privilege_status': priv_status_staff, 'quotas':[],
            'roles':[{'app_code':app_code_product, 'codename':codename} for codename in permissions] }
        self._access_token = self.gen_access_token(profile=access_tok_payld, audience=['product'])
        self._access_tok_payld = access_tok_payld

    def test_invalid_id(self):
        created_ids = tuple(map(lambda x:x['id'], self._created_items))
        created_objs = self.serializer_class.Meta.model.objects.filter(id__in=created_ids)
        serializer_ro = self.serializer_class(many=True, instance=created_objs)
        request_data = serializer_ro.data
        # sub case: lack id
        non_field_error_key = drf_default_settings['NON_FIELD_ERRORS_KEY']
        request_data[0].pop('id',None)
        response = self._send_request_to_backend(path=self.path, method='put', body=request_data,
                access_token=self._access_token )
        self.assertEqual(int(response.status_code), 400)
        err_info = response.json()
        err_msg = "cannot be mapped to existing instance, reason: Field 'id' expected a number but got"
        pos = err_info[0][non_field_error_key].find( err_msg )
        self.assertGreater(pos , 0)
        # sub case: invalid data type of id
        request_data[0]['id'] = 99999
        request_data[-1]['id'] = 'string_id'
        response = self._send_request_to_backend(path=self.path, method='put', body=request_data,
                access_token=self._access_token )
        self.assertEqual(int(response.status_code), 403)
        # sub case: mix of valid id and invalid id
        request_data[0]['id'] = 'wrong_id'
        request_data[-1]['id'] = 123
        response = self._send_request_to_backend(path=self.path, method='put', body=request_data,
                access_token=self._access_token )
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


    def test_bulk_ok(self):
        expect_shown_fields = ['id', 'name', 'price', 'tags', 'ingredients_applied']
        for _ in range(5):
            edit_data = self._rand_gen_edit_data()
            response = self._send_request_to_backend(path=self.path, method='put', body=edit_data,
                    access_token=self._access_token,  expect_shown_fields=expect_shown_fields)
            edited_items = response.json()
            self.assertEqual(int(response.status_code), 200)
            fn = lambda x:{key:x[key] for key in expect_shown_fields}
            expect_edited_items = list(map(fn, edit_data))
            actual_edited_items = list(map(fn, edited_items))
            expect_edited_items = sort_nested_object(obj=expect_edited_items)
            actual_edited_items = sort_nested_object(obj=actual_edited_items)
            self.assertListEqual(expect_edited_items, actual_edited_items)


    def test_permission_denied(self):
        another_usrprof = self._access_tok_payld['id'] + 1
        self._access_tok_payld['id'] = another_usrprof
        access_token = self.gen_access_token(profile=self._access_tok_payld, audience=['product'])
        edit_data = copy.deepcopy(self._request_data[:1])
        edit_data[0]['id'] = self._created_items[0]['id']
        response = self._send_request_to_backend(path=self.path, method='put', body=edit_data,
                access_token=access_token)
        edited_items = response.json()
        self.assertEqual(int(response.status_code), 403)

    def test_conflict_item_error(self):
        key = drf_default_settings['NON_FIELD_ERRORS_KEY']
        edit_data = self._rand_gen_edit_data()
        discarded_id  = edit_data[0]['id']
        edit_data[0]['id'] = edit_data[1]['id']
        response = self._send_request_to_backend(path=self.path, method='put', body=edit_data,
                access_token=self._access_token)
        self.assertEqual(int(response.status_code), 400)
        err_info = response.json()
        err_msg = err_info[key][0]
        pos = err_msg.find('duplicate item found in the list')
        self.assertGreaterEqual(pos, 0)
        pos = err_msg.find(str(discarded_id))
        self.assertLess(pos, 0)
## end of class SaleableItemUpdateTestCase


class SaleableItemDeletionTestCase(SaleableItemUpdateBaseTestCase, SaleableItemVerificationMixin, SoftDeleteCommonTestMixin):
    def setUp(self):
        super().setUp()
        expect_usrprof = 71
        permissions = ['view_productsaleableitem', 'change_productsaleableitem', 'delete_productsaleableitem']
        access_tok_payld = { 'id':expect_usrprof, 'privilege_status': priv_status_staff, 'quotas':[],
            'roles':[{'app_code':app_code_product, 'codename':codename} for codename in permissions] }
        self._access_token = self.gen_access_token(profile=access_tok_payld, audience=['product'])
        self._access_tok_payld = access_tok_payld

    def test_softdelete_permission_denied(self):
        another_usrprof = self._access_tok_payld['id'] + 1
        self._access_tok_payld['id'] = another_usrprof
        access_token = self.gen_access_token(profile=self._access_tok_payld, audience=['product'])
        deleted_ids = list(map(lambda x: {'id':x['id']}, self._created_items))
        response = self._send_request_to_backend(path=self.path, method='delete', body=deleted_ids,
                access_token=access_token )
        self.assertEqual(int(response.status_code), 403)


    def test_softdelete_ok(self):
        num_delete = 2
        deleted_ids = list(map(lambda x: {'id':x['id']}, self._created_items[:num_delete]))
        response = self._send_request_to_backend(path=self.path, method='delete', body=deleted_ids,
                access_token=self._access_token)
        self.assertEqual(int(response.status_code), 202)
        deleted_ids = list(map(lambda x: x['id'], self._created_items[:num_delete]))
        remain_ids  = list(map(lambda x: x['id'], self._created_items[num_delete:]))
        self.assert_softdelete_items_exist(testcase=self, deleted_ids=deleted_ids, remain_ids=remain_ids,
                model_cls_path='product.models.base.ProductSaleableItem',)


    def _softdelete_one_by_one(self, remain_items, deleted_items, delay_interval_sec=0):
        while any(remain_items):
            chosen_item = remain_items.pop()
            response = self._send_request_to_backend(path=self.path, method='delete',
                    body=[{'id': chosen_item['id']}], access_token=self._access_token )
            self.assertEqual(int(response.status_code), 202)
            deleted_items.append(chosen_item)  # insert(0, chosen_item)
            deleted_ids = list(map(lambda x: x['id'], deleted_items))
            remain_ids  = list(map(lambda x: x['id'], remain_items ))
            self.assert_softdelete_items_exist(testcase=self, deleted_ids=deleted_ids, remain_ids=remain_ids,
                    model_cls_path='product.models.base.ProductSaleableItem',)
            time.sleep(delay_interval_sec)

    def _verify_undeleted_items(self, undeleted_items):
        expect_usrprof = self._access_tok_payld['id']
        self.assertGreaterEqual(len(undeleted_items), 1)
        undeleted_items = sorted(undeleted_items, key=lambda x:x['id'])
        undeleted_ids  = tuple(map(lambda x:x['id'], undeleted_items))
        undeleted_objs = self.serializer_class.Meta.model.objects.filter(id__in=undeleted_ids).order_by('id')
        self.verify_objects(actual_instances=undeleted_objs, expect_data=undeleted_items,
                usrprof_id=expect_usrprof)


    def test_undelete_by_time(self):
        remain_items  = copy.copy(self._created_items)
        deleted_items = []
        # soft-delete one after another
        for _ in range(3):
            self._softdelete_one_by_one(remain_items, deleted_items, delay_interval_sec=2)
            # recover one by one based on the time at which each item was soft-deleted
            while any(deleted_items):
                chosen_item = deleted_items.pop()
                undeleted_items = self.perform_undelete(testcase=self, path=self.path, access_token=self._access_token )
                self._verify_undeleted_items(undeleted_items)
                self.assertEqual(chosen_item['id'] , undeleted_items[0]['id'])
                remain_items.append(chosen_item)
            self.assertListEqual(self._created_items, remain_items)


    def test_undelete_specific_item(self):
        remain_items  = copy.copy(self._created_items)
        deleted_items = []
        for _ in range(3):
            self._softdelete_one_by_one(remain_items, deleted_items, delay_interval_sec=0)
            undeleting_items_gen = listitem_rand_assigner(list_=deleted_items, min_num_chosen=len(deleted_items),
                    max_num_chosen=(len(deleted_items) + 1))
            undeleting_items = list(undeleting_items_gen)
            half = len(undeleting_items) >> 1
            body = {'ids':[x['id'] for x in undeleting_items[:half]]}
            affected_items = self.perform_undelete(body=body, testcase=self, path=self.path,
                    access_token=self._access_token)
            self._verify_undeleted_items(affected_items)
            body = {'ids':[x['id'] for x in undeleting_items[half:]]}
            affected_items = self.perform_undelete(body=body, testcase=self, path=self.path,
                    access_token=self._access_token)
            self._verify_undeleted_items(affected_items)
            remain_items, deleted_items = deleted_items, remain_items # internally swap 2 lists


    def test_no_softdeleted_item(self):
        kwargs = {'testcase': self, 'path':self.path, 'expect_resp_status':410,
                'expect_resp_msg':'Nothing recovered', 'access_token':self._access_token}
        self.perform_undelete(**kwargs)
        remain_items = self._created_items[:-1]
        kwargs['body'] = {'ids':[x['id'] for x in remain_items]}
        self.perform_undelete(**kwargs)


    def test_undelete_permission_denied(self):
        remain_items  = copy.copy(self._created_items)
        deleted_items = []
        self._softdelete_one_by_one(remain_items, deleted_items, delay_interval_sec=0)
        self.assertGreater(len(deleted_items), 0)
        another_usrprof = self._access_tok_payld['id'] + 1
        self._access_tok_payld['id'] = another_usrprof
        access_token = self.gen_access_token(profile=self._access_tok_payld, audience=['product'])
        kwargs = {'testcase': self, 'path':self.path, 'expect_resp_status':403, 'access_token':access_token,
                'expect_resp_msg':'user is not allowed to undelete the item(s)'}
        kwargs['body'] = {'ids':[x['id'] for x in deleted_items]}
        self.perform_undelete(**kwargs)

    def test_undelete_with_invalid_id(self):
        kwargs = {'testcase': self, 'path':self.path, 'expect_resp_status':400,
                'expect_resp_msg':'invalid data in request body', 'access_token':self._access_token}
        kwargs['body'] = {'ids':['wrong_id', 'fake_id']}
        self.perform_undelete(**kwargs)
## end of class SaleableItemDeletionTestCase


class SaleableItemQueryTestCase(SaleableItemUpdateBaseTestCase, SaleableItemVerificationMixin):
    def setUp(self):
        super().setUp()
        expect_usrprof = 71
        permissions = ['view_xxoxoxo']
        access_tok_payld = { 'id':expect_usrprof, 'privilege_status': priv_status_staff, 'quotas':[],
            'roles':[{'app_code':app_code_product, 'codename':codename} for codename in permissions] }
        self._access_token = self.gen_access_token(profile=access_tok_payld, audience=['product'])
        self._access_tok_payld = access_tok_payld

    def test_bulk_items_full_info(self):
        created_ids = tuple(map(lambda x:x['id'], self._created_items))
        response = self._send_request_to_backend(path=self.path, method='get', ids=created_ids,
                    access_token=self._access_token )
        actual_items = response.json()
        expect_items = self.serializer_class.Meta.model.objects.filter(id__in=created_ids)
        self.assertEqual(int(response.status_code), 200)
        self.assertEqual(len(self._created_items), len(actual_items))
        self.verify_objects(actual_instances=expect_items , expect_data=actual_items)


    def test_bulk_items_partial_info(self):
        expect_shown_fields = ['id','name','media_set','ingredients_applied']
        created_ids = tuple(map(lambda x:x['id'], self._created_items))
        response = self._send_request_to_backend(path=self.path, method='get', ids=created_ids,
                expect_shown_fields=expect_shown_fields, access_token=self._access_token )
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


    def test_single_items_full_info(self):
        path_single_item = '/saleableitem/%s'
        for created_item in self._created_items:
            path_with_id = path_single_item % created_item['id']
            response = self._send_request_to_backend(path=path_with_id, method='get', access_token=self._access_token)
            actual_item = response.json()
            expect_item = self.serializer_class.Meta.model.objects.get(id=created_item['id'])
            self.assertEqual(int(response.status_code), 200)
            self.assertEqual(created_item['id'], actual_item['id'])
            self.verify_objects(actual_instances=[expect_item], expect_data=[actual_item])




class SaleableItemSearchFilterTestCase(SaleableItemUpdateBaseTestCase, SaleableItemVerificationMixin):
    min_num_applied_attrs = 2
    min_num_applied_tags = 2
    min_num_applied_ingredients = 2
    rand_create = False
    _model_cls = SaleableItemVerificationMixin.serializer_class.Meta.model

    def setUp(self):
        super().setUp()
        expect_usrprof = 71
        permissions = ['view_xxoxoxo']
        access_tok_payld = { 'id':expect_usrprof, 'privilege_status': priv_status_staff, 'quotas':[],
            'roles':[{'app_code':app_code_product, 'codename':codename} for codename in permissions] }
        self._access_token = self.gen_access_token(profile=access_tok_payld, audience=['product'])
        self._access_tok_payld = access_tok_payld

    def test_simple_search(self):
        saleitem_ids = tuple(map(lambda d:d['id'], self._created_items))
        saleitem_qset = self._model_cls.objects.filter(id__in=saleitem_ids)
        keywords = ['noodle', 'PI', 'Pi', 'Dev board']
        for keyword in keywords:
            extra_query_params = {'search':keyword}
            response = self._send_request_to_backend(path=self.path, method='get',
                    extra_query_params=extra_query_params, access_token=self._access_token)
            actual_items = response.json()
            final_condition = Q(name__icontains=keyword) | Q(pkgs_applied__package__name__icontains=keyword) \
                    | Q(ingredients_applied__ingredient__name__icontains=keyword)
            expect_items = saleitem_qset.filter(final_condition).distinct().values('id','name')
            self.assertEqual(int(response.status_code), 200)
            self.assertEqual(len(actual_items), expect_items.count())
            actual_items = sorted(actual_items, key=lambda x:x['name'])
            expect_items = sorted(expect_items, key=lambda x:x['name'])
            actual_items_iter = iter(actual_items)
            for expect_item in expect_items:
                actual_item = next(actual_items_iter)
                self._assert_simple_fields(check_fields=['id','name'], exp_sale_item=expect_item,
                        ac_sale_item=actual_item)


    def _test_advanced_search_common(self, adv_cond):
        extra_query_params = {'advanced_search': 'yes', 'adv_search_cond': json.dumps(adv_cond)}
        response = self._send_request_to_backend( path=self.path, method='get',
                extra_query_params=extra_query_params, access_token=self._access_token )
        actual_items = response.json()
        self.assertEqual(int(response.status_code), 200)
        self.assertGreaterEqual(len(actual_items), 1)
        return actual_items


    def test_tags(self):
        # note that distinct() works like set() to reduce duplicate
        qset = self._model_cls.objects.filter(tags__isnull=False).values_list('tags__id', flat=True).distinct()
        tag_ids = tuple(qset[:2])
        adv_cond = {'operator': 'or', 'operands': []}
        for tag_id in tag_ids:
            adv_cond_sub_clause = {
                'operator':'==',
                'operands':['tags__id', tag_id],
                'metadata':{}
            }
            adv_cond['operands'].append(adv_cond_sub_clause)
        saleitems_found = self._test_advanced_search_common(adv_cond=adv_cond)
        expected_objs   = self._model_cls.objects.filter(tags__id__in=tag_ids).distinct()
        self.assertLessEqual(len(saleitems_found), len(self._created_items))
        self.assertEqual(len(saleitems_found), expected_objs.count())
        self.verify_objects(actual_instances=expected_objs, expect_data=saleitems_found)


    def test_ingredients_applied(self):
        qset = self._model_cls.objects.annotate(num_ingre=Count('ingredients_applied__ingredient'))
        qset = qset.filter(num_ingre__gt=0)
        values = qset.values('id','ingredients_applied')
        limit = max(values.count() >> 1, 1)
        values = values[:limit]
        adv_cond = {'operator': 'or', 'operands': []}
        for value in values:
            adv_cond_sub_clause = {
                'operator': 'and',
                'operands': [
                    {
                        'operator':'==',
                        'operands':['ingredients_applied__ingredient', value['ingredients_applied__ingredient_id']],
                        'metadata':{}
                    },
                    {
                        'operator':'==',
                        'operands':['ingredients_applied__sale_item', value['id']],
                        'metadata':{}
                    },
                ]
            }
            adv_cond['operands'].append(adv_cond_sub_clause)
        saleitems_found = self._test_advanced_search_common(adv_cond=adv_cond)
        expect_saleitems_found = list(dict.fromkeys([v['id'] for v in values]))
        actual_saleitems_found = list(map(lambda v:v['id'], saleitems_found))
        self.assertListEqual(sorted(actual_saleitems_found), sorted(expect_saleitems_found))


    def test_complex_condition(self):
        aggregates_nested_fields = {'num_ingre': Count('ingredients_applied__ingredient'),
                'num_tags': Count('tags'), }
        filter_nested_fields = dict(map(lambda ag_fd: (LOOKUP_SEP.join([ag_fd, 'gt']), 0),
                aggregates_nested_fields.keys()))
        qset = self._model_cls.objects.annotate(**aggregates_nested_fields)
        qset = qset.filter(**filter_nested_fields)
        saleitem = qset.first()
        ingredient_id = saleitem.ingredients_applied.first().ingredient.pk
        tags    = saleitem.tags.all()
        tag_ids = list(map(lambda x: x.pk, tags))
        adv_cond = {
            'operator': 'and',
            'operands': [
                {
                    'operator':'==',
                    'operands':['ingredients_applied__ingredient', ingredient_id],
                    'metadata':{}
                },
                {
                    'operator':'or',
                    'operands': [
                        {
                            'operator':'==',
                            'operands':['tags__id', tag_ids[0]],
                            'metadata':{}
                        },
                        {
                            'operator':'==',
                            'operands':['tags__id', tag_ids[1]],
                            'metadata':{}
                        },
                    ]
                }
            ]
        }
        saleitems_found = self._test_advanced_search_common(adv_cond=adv_cond)
        actual_ids = list(map(lambda x:x['id'] , saleitems_found))
        self.assertIn(saleitem.pk, actual_ids)
        for found_item in saleitems_found:
            diff = set(found_item['tags']) - set(tag_ids)
            self.assertLess(len(diff), len(found_item['tags']))
            ingredient_ids = tuple(map(lambda x:x['ingredient'] , found_item['ingredients_applied']))
            self.assertIn(ingredient_id, ingredient_ids)


