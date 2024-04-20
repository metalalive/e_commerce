import random
import math
import copy
import json
from functools import partial, reduce
from unittest.mock import Mock, patch

from django.test import TransactionTestCase
from django.db.models import Count
from rest_framework.exceptions import ValidationError as DRFValidationError
from rest_framework.settings import DEFAULTS as drf_default_settings

from ecommerce_common.util import sort_nested_object
from product.permissions import FabricationIngredientPermissions
from product.models.base import ProductAttributeType, _ProductAttrValueDataType

from tests.common import _MockTestClientInfoMixin,_fixtures as model_fixtures, listitem_rand_assigner, http_request_body_template, _common_instances_setup, assert_view_permission_denied, assert_view_bulk_create_with_response, assert_view_unclassified_attributes, SoftDeleteCommonTestMixin

from ..common import app_code_product, priv_status_staff
from  .common import HttpRequestDataGenDevIngredient, DevIngredientVerificationMixin


def _related_instance_setup(stored_models, num_attrtypes=None):
    if num_attrtypes is None:
        num_attrtypes = len(model_fixtures['ProductAttributeType'])
    models_info = [(ProductAttributeType, num_attrtypes),]
    _common_instances_setup(out=stored_models, models_info=models_info)


class DevIngredientBaseViewMixin(TransactionTestCase, _MockTestClientInfoMixin, HttpRequestDataGenDevIngredient):
    permission_class = FabricationIngredientPermissions

    def refresh_req_data(self, num_create=None):
        return super().refresh_req_data(fixture_source=model_fixtures['ProductDevIngredient'],
                http_request_body_template=http_request_body_template['ProductDevIngredient'],
                num_create=num_create)

    def setUp(self):
        self._setup_keystore()
        self._request_data = self.refresh_req_data(num_create=5)

    def tearDown(self):
        self._teardown_keystore()
        self._client.cookies.clear()
        self.min_num_applied_attrs = 0
        self.min_num_applied_tags = 0
        self.min_num_applied_ingredients = 0
## end of class DevIngredientBaseViewMixin


class DevIngredientCreationTestCase(DevIngredientBaseViewMixin):
    path = '/ingredients'

    def test_permission_denied(self):
        kwargs = { 'testcase':self, 'request_body_data':self._request_data, 'path':self.path,
            'permissions': self.permission_class.perms_map['POST'], 'http_method':'post',
        }
        assert_view_permission_denied(**kwargs)
        kwargs['permissions'] = self.permission_class.perms_map['PUT']
        kwargs['http_method'] = 'put'
        assert_view_permission_denied(**kwargs)


    def test_bulk_ok_with_partial_response(self):
        _related_instance_setup(self.stored_models)
        expect_shown_fields = ['id', 'name', 'attributes']
        expect_hidden_fields = ['category']
        created_items = assert_view_bulk_create_with_response(testcase=self, expect_shown_fields=expect_shown_fields,
                expect_hidden_fields=expect_hidden_fields, path=self.path, body=self._request_data,
                permissions=['view_productdevingredient', 'add_productdevingredient'], method='post')

    def test_validation_error_unclassified_attributes(self):
        # don't create any attribute type instance in this case
        self.min_num_applied_attrs = 1
        new_request_data = self.refresh_req_data()
        assert_view_unclassified_attributes(testcase=self, path=self.path, method='post',  body=new_request_data,
                permissions=['view_productdevingredient', 'add_productdevingredient'] )
## end of class DevIngredientCreationTestCase


class DevIngredientUpdateBaseTestCase(DevIngredientBaseViewMixin, DevIngredientVerificationMixin):
    path = '/ingredients'

    def setUp(self):
        super().setUp()
        _related_instance_setup(stored_models=self.stored_models)
        permissions = ['view_productdevingredient', 'add_productdevingredient']
        self._access_tok_payld = { 'id':71, 'privilege_status': priv_status_staff, 'quotas':[],
            'roles':[{'app_code':app_code_product, 'codename':codename} for codename in permissions] }
        access_token = self.gen_access_token(profile=self._access_tok_payld, audience=['product'])
        response = self._send_request_to_backend(path=self.path, body=self._request_data,
                method='post', expect_shown_fields=['id', 'name',], access_token=access_token)
        self.assertEqual(int(response.status_code), 201)
        created_items = response.json()
        created_ids = tuple(map(lambda x:x['id'], created_items))
        self._created_items = self.serializer_class.Meta.model.objects.filter(id__in=created_ids)
        serializer_ro = self.serializer_class(many=True, instance=self._created_items)
        self._old_request_data = self._request_data
        self._request_data = serializer_ro.data


class DevIngredientUpdateTestCase(DevIngredientUpdateBaseTestCase):
    # ensure there are always at least 3 attributes declared in each ingredient
    min_num_applied_attrs = 4
    max_num_applied_attrs = 6

    def setUp(self):
        super().setUp()
        permissions = ['view_productdevingredient', 'change_productdevingredient']
        self._access_tok_payld['roles'] = [{'app_code':app_code_product, 'codename':codename} for codename in permissions]
        self._access_token = self.gen_access_token(profile=self._access_tok_payld, audience=['product'])

    def test_invalid_id(self):
        edit_data = self._request_data
        expect_response_status = 400
        # sub case: lack id
        key = drf_default_settings['NON_FIELD_ERRORS_KEY']
        edit_data[0].pop('id',None)
        response = self._send_request_to_backend(path=self.path, method='put', body=edit_data,
                access_token=self._access_token)
        err_items = response.json()
        self.assertEqual(int(response.status_code), expect_response_status)
        # sub case: invalid data type of id
        edit_data[0]['id'] = 99999
        edit_data[-1]['id'] = 'string_id'
        response = self._send_request_to_backend(path=self.path, method='put', body=edit_data,
                access_token=self._access_token)
        err_items = response.json()
        self.assertEqual(int(response.status_code), expect_response_status)
        # sub case: mix of valid id and invalid id
        edit_data[0]['id'] = 'wrong_id'
        edit_data[-1]['id'] = 123
        response = self._send_request_to_backend(path=self.path, method='put', body=edit_data,
                access_token=self._access_token)
        err_items = response.json()
        self.assertEqual(int(response.status_code), expect_response_status)


    def _update_attr_val_id(self, src, dst):
        # this function works based on assumption that there is only one new attribute
        # created in the bulk update test case.
        gen_map_fn = lambda data: {d['id']: {a.get('id', None):a for a in d['attributes']}  for d in data}
        src = gen_map_fn(src)
        dst = gen_map_fn(dst)
        for k, v in dst.items():
            attr_ids_src = set(src[k].keys())
            attr_ids_dst = set(v.keys())
            new_attr_ids = list(attr_ids_src - attr_ids_dst)
            self.assertEqual(len(new_attr_ids), 1)
            edit_dst = v[None]
            edit_dst['id'] = new_attr_ids[0]

    def test_bulk_ok(self):
        expect_shown_fields = ['id', 'name', 'category', 'attributes']
        num_edit_data = len(self._request_data) >> 1
        editing_data    = self._request_data[:num_edit_data]
        unaffected_data = self._request_data[num_edit_data:]
        self.rand_gen_edit_data(editing_data=editing_data)
        response = self._send_request_to_backend(path=self.path, method='put', body=editing_data,
                expect_shown_fields=expect_shown_fields, access_token=self._access_token)
        self.assertEqual(int(response.status_code), 200)
        edited_data = response.json()
        self._update_attr_val_id(src=edited_data, dst=editing_data)
        sorted_editing_data = sort_nested_object(editing_data)
        sorted_edited_data  = sort_nested_object(edited_data)
        expect_edited_data = json.dumps(sorted_editing_data , sort_keys=True)
        actual_edited_data = json.dumps(sorted_edited_data  , sort_keys=True)
        self.assertEqual(expect_edited_data, actual_edited_data)


    def test_conflict_ingredient_id(self):
        key = drf_default_settings['NON_FIELD_ERRORS_KEY']
        editing_data = self._request_data
        self.rand_gen_edit_data(editing_data=editing_data)
        # conflict ingredient id
        discarded_id  = editing_data[0]['id']
        editing_data[0]['id'] = editing_data[1]['id']
        response = self._send_request_to_backend(path=self.path, method='put', body=editing_data,
                 access_token=self._access_token)
        self.assertEqual(int(response.status_code), 400)
        err_info = response.json()
        err_msg = err_info[key][0]
        pos = err_msg.find('duplicate item found in the list')
        self.assertGreaterEqual(pos, 0)
        pos = err_msg.find(str(discarded_id))
        self.assertLess(pos, 0)

    def test_conflict_attribute_id_different_dtypes(self):
        editing_data = self._request_data[:1]
        editing_attrs = editing_data[0]['attributes']
        discarded_attrval_id = editing_attrs[0]['id']
        discarded_attr_type  = editing_attrs[0]['type']
        discarded_attr_type_obj = ProductAttributeType.objects.get(id=discarded_attr_type)
        # there must be at least one attribute with different data type from any other attributes
        chosen_attr = tuple(filter(lambda a: ProductAttributeType.objects.get(id=a['type']).dtype \
                != discarded_attr_type_obj.dtype, editing_attrs))
        chosen_attr = chosen_attr[0]
        editing_attrs[0]['id'] = chosen_attr['id']
        response = self._send_request_to_backend(path=self.path, method='put', body=editing_data,
                access_token=self._access_token)
        # conflict attribute value id will NOT cause any error , the product backend
        # app simply skips the incorrect id
        edited_data = response.json()
        self.assertEqual(int(response.status_code), 200)
        actual_ingredient = self.serializer_class.Meta.model.objects.get(id=editing_data[0]['id'])
        attrval_field_name = _ProductAttrValueDataType.related_field_map(discarded_attr_type_obj.dtype)
        attrval_manager = getattr(actual_ingredient, attrval_field_name)
        discarded_attrval_exists = attrval_manager.filter(id=discarded_attrval_id).exists()
        self.assertFalse(discarded_attrval_exists)
        qset = attrval_manager.filter(attr_type=discarded_attr_type)
        #if not qset.exists():
        #    import pdb
        #    pdb.set_trace()
        actual_new_attrval_id = list(qset.values_list('id', flat=True))
        self.assertGreater(len(actual_new_attrval_id), 0)
        expect_new_attrval_id = filter(lambda a: a['type'] == discarded_attr_type, edited_data[0]['attributes'])
        expect_new_attrval_id = list(map(lambda a: a['id'], expect_new_attrval_id))
        self.assertGreater(len(expect_new_attrval_id), 0)
        self.assertSetEqual(set(expect_new_attrval_id), set(actual_new_attrval_id))
## end of class DevIngredientUpdateTestCase


class DevIngredientDeletionTestCase(DevIngredientUpdateBaseTestCase, SoftDeleteCommonTestMixin):

    def setUp(self):
        super().setUp()
        permissions = ['view_productdevingredient', 'change_productdevingredient', 'delete_productdevingredient']
        self._access_tok_payld['roles'] = [{'app_code':app_code_product, 'codename':codename} for codename in permissions]
        self._access_token = self.gen_access_token(profile=self._access_tok_payld, audience=['product'])

    def test_softdelete_ok(self):
        num_delete = 2
        deleted_ids = list(map(lambda x: {'id':x.id}, self._created_items[:num_delete]))
        response = self._send_request_to_backend(path=self.path, method='delete',
                body=deleted_ids, access_token=self._access_token)
        self.assertEqual(int(response.status_code), 202)
        deleted_ids = list(map(lambda x: x.id, self._created_items[:num_delete]))
        remain_ids  = list(map(lambda x: x.id, self._created_items[num_delete:]))
        self.assert_softdelete_items_exist(testcase=self, deleted_ids=deleted_ids, remain_ids=remain_ids,
               model_cls_path='product.models.development.ProductDevIngredient',)


    def _verify_undeleted_items(self, undeleted_items):
        self.assertGreaterEqual(len(undeleted_items), 1)
        undeleted_items = sorted(undeleted_items, key=lambda x:x['id'])
        undeleted_ids  = tuple(map(lambda x:x['id'], undeleted_items))
        undeleted_objs = self.serializer_class.Meta.model.objects.filter(id__in=undeleted_ids).order_by('id')
        self.verify_objects(actual_instances=undeleted_objs, expect_data=undeleted_items)

    def test_undelete_by_time(self):
        remain_items  = list(self._created_items)
        deleted_items = []
        model_cls_path = 'product.models.development.ProductDevIngredient'
        self._softdelete_by_half(remain_items, deleted_items, testcase=self, api_url=self.path,
                model_cls_path=model_cls_path, access_token=self._access_token)
        # recover based on the time at which each item was soft-deleted, there may be more
        # than one item being undeleted at one API call.
        while any(deleted_items):
            undeleted_items = self.perform_undelete(testcase=self, path=self.path, access_token=self._access_token)
            self._verify_undeleted_items(undeleted_items)
            undeleted_ids  = tuple(map(lambda x:x['id'], undeleted_items))
            moving_gen = tuple(filter(lambda obj: obj.id in undeleted_ids, deleted_items))
            for item in moving_gen:
                remain_items.append(item)
                deleted_items.remove(item)
        self.assertSetEqual(set(self._created_items), set(remain_items))

    def test_undelete_specific_item(self):
        remain_items  = list(self._created_items)
        deleted_items = []
        model_cls_path = 'product.models.development.ProductDevIngredient'
        self._softdelete_by_half(remain_items, deleted_items, testcase=self, api_url=self.path,
                model_cls_path=model_cls_path, access_token=self._access_token)
        # recover based on the time at which each item was soft-deleted, there may be more
        # than one item being undeleted at one API call.
        num_undelete = len(deleted_items) >> 1
        undeleting_items_gen = listitem_rand_assigner(list_=deleted_items, min_num_chosen=num_undelete,
                    max_num_chosen=(num_undelete + 1))
        body = {'ids':[obj.id for obj in undeleting_items_gen]}
        affected_items = self.perform_undelete(body=body, testcase=self, path=self.path, access_token=self._access_token)
        self._verify_undeleted_items(affected_items)
        expect_undel_ids = body['ids']
        actual_undel_ids = tuple(map(lambda x:x['id'], affected_items))
        self.assertSetEqual(set(expect_undel_ids), set(actual_undel_ids))
        for obj in deleted_items:
            obj.refresh_from_db()
            expect_del = obj.id not in expect_undel_ids
            self.assertEqual(obj.is_deleted(), expect_del)


    def test_no_softdeleted_item(self):
        kwargs = {'testcase': self, 'path':self.path, 'expect_resp_status':410, 'access_token':self._access_token,
                'expect_resp_msg':'Nothing recovered'}
        self.perform_undelete(**kwargs)
        remain_items = self._created_items[:2]
        kwargs['body'] = {'ids':[obj.id for obj in remain_items]}
        self.perform_undelete(**kwargs)

    def test_softdelete_permission_denied(self):
        permissions = ['delete_productdevingredient']
        self._access_tok_payld['roles'] = [{'app_code':app_code_product, 'codename':codename} for codename in permissions]
        access_token = self.gen_access_token(profile=self._access_tok_payld, audience=['product'])
        deleted_ids = list(map(lambda obj: {'id': obj.id}, self._created_items))
        response = self._send_request_to_backend(path=self.path, method='delete', body=deleted_ids,
                 access_token=access_token)
        self.assertEqual(int(response.status_code), 403)

    def test_undelete_permission_denied(self):
        permissions = ['view_productdevingredient', 'delete_productdevingredient']
        self._access_tok_payld['roles'] = [{'app_code':app_code_product, 'codename':codename} for codename in permissions]
        access_token = self.gen_access_token(profile=self._access_tok_payld, audience=['product'])
        remain_items  = list(self._created_items)
        deleted_items = []
        model_cls_path = 'product.models.development.ProductDevIngredient'
        self._softdelete_by_half(remain_items, deleted_items, testcase=self, api_url=self.path,
                model_cls_path=model_cls_path, access_token=access_token)
        self.assertGreater(len(deleted_items), 0)
        # ----------------
        kwargs = {'testcase': self, 'path':self.path, 'expect_resp_status':403, 'access_token':access_token,
                'expect_resp_msg':'You do not have permission to perform this action.'}
        kwargs['body'] = {'ids':[obj.id for obj in deleted_items]}
        self.perform_undelete(**kwargs)
## end of class DevIngredientDeletionTestCase


class DevIngredientQueryTestCase(DevIngredientUpdateBaseTestCase, DevIngredientVerificationMixin):
    def setUp(self):
        super().setUp()
        permissions = ['view_productdevingredient']
        self._access_tok_payld['roles'] = [{'app_code':app_code_product, 'codename':codename} for codename in permissions]
        self._access_token = self.gen_access_token(profile=self._access_tok_payld, audience=['product'])

    def test_bulk_items_partial_info(self):
        expect_shown_fields = ['id','name','attributes']
        num_read = len(self._created_items) - 1
        read_items_gen = listitem_rand_assigner(list_=self._created_items, max_num_chosen=(num_read + 1))
        created_ids = tuple(map(lambda obj: obj.id, read_items_gen))
        response = self._send_request_to_backend(path=self.path, method='get', ids=created_ids,
                expect_shown_fields=expect_shown_fields, access_token=self._access_token)
        actual_items = response.json()
        expect_items = self._created_items.filter(id__in=created_ids)
        self.assertEqual(int(response.status_code), 200)
        self.verify_objects(actual_instances=expect_items, expect_data=actual_items,
                non_nested_fields=['id','name'])


class DevIngredientAdvancedSearchTestCase(DevIngredientUpdateBaseTestCase, DevIngredientVerificationMixin):
    min_num_applied_attrs = 4
    _model_cls = DevIngredientVerificationMixin.serializer_class.Meta.model

    def setUp(self):
        super().setUp()
        permissions = ['view_productdevingredient']
        self._access_tok_payld['roles'] = [{'app_code':app_code_product, 'codename':codename} for codename in permissions]
        self._access_token = self.gen_access_token(profile=self._access_tok_payld, audience=['product'])

    def _test_advanced_search_common(self, adv_cond):
        extra_query_params = {'advanced_search': 'yes', 'adv_search_cond': json.dumps(adv_cond)}
        response = self._send_request_to_backend(path=self.path, method='get',
                extra_query_params=extra_query_params, access_token=self._access_token)
        actual_items = response.json()
        self.assertEqual(int(response.status_code), 200)
        self.assertGreaterEqual(len(actual_items), 1)
        return actual_items

    def test_advanced_search_attr_str(self):
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

    def test_advanced_search_attr_int(self):
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


    def test_advanced_search_attr_float(self):
        qset = self._model_cls.objects.annotate(num_float_attr=Count('attr_val_float')).filter(num_float_attr__gt=0)
        data = qset.values('id', 'attr_val_float__attr_type__id', 'attr_val_float__attr_type__dtype', 'attr_val_float__value')
        data = {v['id']: v  for v in data}
        adv_cond = {'operator': 'or', 'operands': []}
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
        items_found = self._test_advanced_search_common(adv_cond=adv_cond)
        self.assertLessEqual(len(items_found), len(self._created_items))
        self.assertEqual(len(items_found), qset.count())
        self.verify_objects(actual_instances=qset, expect_data=items_found)


    def test_advanced_nested_search_mix(self):
        qset = self._model_cls.objects.all()
        attr_choices = {ingredient.id: {dtype_opt.value[0]: \
            list(getattr(ingredient, dtype_opt.value[0][1]).values('id', 'attr_type','value')) \
            for dtype_opt in _ProductAttrValueDataType \
            if getattr(ingredient, dtype_opt.value[0][1]).count() > 0} \
            for ingredient in qset}
        attr_choices = tuple(filter(lambda kv: len(kv[1].keys()) > 1, attr_choices.items()))
        expect_ingredient_id,  expect_attrs = attr_choices[0]
        adv_cond = {'operator': 'and', 'operands': []}
        for dtype_val, attrvals in expect_attrs.items():
            attrval = attrvals[0]
            if dtype_val[0] == 1: # string
                adv_cond_sub_clause = {
                    'operator': 'and',
                    'operands':[
                        {
                            'operator':'==',
                            'operands':['attributes__type', attrval['attr_type']],
                            'metadata':{'dtype': dtype_val[0]}
                        },
                        {
                            'operator':'contains',
                            'operands':['attributes__value', attrval['value'] ],
                            'metadata':{'dtype': dtype_val[0]}
                        }
                    ]
                }
            else: # number types e.g. integer, floating-point
                variation = 1.0
                upper_bound = attrval['value'] + variation
                lower_bound = attrval['value'] - variation
                adv_cond_sub_clause = {
                    'operator': 'and',
                    'operands':[
                        {
                            'operator':'==',
                            'operands':['attributes__type', attrval['attr_type']],
                            'metadata':{'dtype': dtype_val[0]}
                        },
                        {
                            'operator': 'and',
                            'operands':[
                                {
                                    'operator':'>',
                                    'operands':['attributes__value', lower_bound],
                                    'metadata':{'dtype': dtype_val[0]}
                                },
                                {
                                    'operator':'<',
                                    'operands':['attributes__value', upper_bound],
                                    'metadata':{'dtype': dtype_val[0]}
                                }
                            ]
                        }
                    ]
                }
            # end of custom sub-clause condition
            adv_cond['operands'].append(adv_cond_sub_clause)
        items_found = self._test_advanced_search_common(adv_cond=adv_cond)
        actual_item = tuple(filter(lambda d: d['id'] == expect_ingredient_id, items_found))
        self.assertEqual(len(actual_item) , 1)
        actual_item = actual_item[0]
## end of class DevIngredientAdvancedSearchTestCase

