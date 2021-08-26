import random
import math
import copy
import json
from functools import partial, reduce
from unittest.mock import Mock

from django.test import TransactionTestCase, TestCase
from django.core.exceptions    import ValidationError as DjangoValidationError
from rest_framework.exceptions import ValidationError as DRFValidationError
from rest_framework.settings import DEFAULTS as drf_default_settings

from common.validators     import NumberBoundaryValidator, UnprintableCharValidator
from product.serializers.base import SaleableItemSerializer
from product.models.base import ProductTag, ProductTagClosure, ProductAttributeType, _ProductAttrValueDataType, ProductSaleableItem
from product.models.development import ProductDevIngredientType, ProductDevIngredient

from .common import _fixtures as model_fixtures, listitem_rand_assigner, _load_init_params, _dict_key_replace, _dict_kv_pair_evict, _get_inst_attr, assert_field_equal, rand_gen_request_body, http_request_body_template, HttpRequestDataGenSaleableItem, _saleitem_related_instance_setup, SaleableItemVerificationMixin



class ExtendedTestCaseMixin:
    def reset_validation_result(self, serializer):
        serializer._errors.clear()
        delattr(serializer, '_validated_data')


class SaleableItemCommonMixin(ExtendedTestCaseMixin, HttpRequestDataGenSaleableItem, SaleableItemVerificationMixin):
    stored_models = {}
    num_users = 1

    def setUp(self):
        _saleitem_related_instance_setup(self.stored_models)
        self.profile_ids = [random.randrange(1,15) for _ in range(self.num_users)]
        saleitems_data_gen = listitem_rand_assigner(list_=model_fixtures['ProductSaleableItem'])
        self.request_data = rand_gen_request_body(customize_item_fn=self.customize_req_data_item,
                data_gen=saleitems_data_gen,  template=http_request_body_template['ProductSaleableItem'])
        self.request_data = list(self.request_data)

    def tearDown(self):
        self.stored_models.clear()
## end of class SaleableItemCommonMixin


class SaleableItemCreationTestCase(SaleableItemCommonMixin, TransactionTestCase):
    def setUp(self):
        super().setUp()
        self.serializer_kwargs = {'data': self.request_data, 'many': True, 'usrprof_id': self.profile_ids[0],}

    def test_bulk_ok(self):
        serializer = SaleableItemSerializer( **self.serializer_kwargs )
        serializer.is_valid(raise_exception=True)
        actual_instances = serializer.save()
        expect_data = self.serializer_kwargs['data']
        self.verify_objects(actual_instances, expect_data, usrprof_id=serializer.child.usrprof_id)


    def test_skip_given_id(self):
        invalid_cases = (12,)
        self.serializer_kwargs['data'] = self.request_data[:1]
        self.serializer_kwargs['data'][0]['id'] = invalid_cases[0]
        self.assertEqual(self.serializer_kwargs['data'][0]['id'] , invalid_cases[0])
        serializer = SaleableItemSerializer( **self.serializer_kwargs )
        serializer.is_valid(raise_exception=True)
        with self.assertRaises(KeyError):
            validated_id = serializer.validated_data[0]['id']
            self.assertEqual(validated_id , invalid_cases[0])
        with self.assertRaises(KeyError):
            validated_id = self.serializer_kwargs['data'][0]['id']
            self.assertEqual(validated_id , invalid_cases[0])


    def test_fields_validate_error(self):
        invalid_cases = [
            ('name', None, 'This field may not be null.'),
            ('name', '',   'This field may not be blank.'),
            ('price', None,   'This field may not be null.'),
            ('price', -0.3,  NumberBoundaryValidator._error_msg_pattern % (-0.3, 0.0, 'gt')),
            ('price', -0.0,  NumberBoundaryValidator._error_msg_pattern % (-0.0, 0.0, 'gt')),
            ('price',  0.0,  NumberBoundaryValidator._error_msg_pattern % ( 0.0, 0.0, 'gt')),
            ('price',  '',    'A valid number is required.'),
            ('price',  '19g', 'A valid number is required.'),
            ('visible', None, 'This field may not be null.'),
            ('tags',    None, 'This field may not be null.'),
            ('tags',   'xxx', 'Expected a list of items but got type "str".'),
            ('tags', [34, 37, 1234, 39, 5678, 33],  'Invalid pk "1234" - object does not exist.'),
            ('media_set',   None, 'This field may not be null.'),
            ('media_set', ['x8Ej','9u 2'], UnprintableCharValidator._error_msg_pattern % ('9u 2')),
            ('media_set', ['x8Ej','9u 2', '8u%G', 'd8\'w'], UnprintableCharValidator._error_msg_pattern % ', '.join(['9u 2', 'd8\'w'])),
            ('media_set', ['8Ej\\','9u2L', '@"$%', 'halo'], UnprintableCharValidator._error_msg_pattern % ', '.join(['8Ej\\', '@"$%'])),
            # no need to verify `usrprof` field
        ]
        self.serializer_kwargs['data'] = self.request_data[:]
        for idx in range(len(self.serializer_kwargs['data'])):
            fn_choose_edit_item = lambda x : x[idx]
            self._loop_through_invalid_cases_common(fn_choose_edit_item, invalid_cases)


    def test_ingredients_applied_validate_error(self):
        invalid_cases = [
            ('ingredient', None, 'This field may not be null.'),
            ('ingredient', '',   'This field may not be null.'),
            ('ingredient', -123, 'Invalid pk "-123" - object does not exist.'),
            ('ingredient',  999, 'Invalid pk "999" - object does not exist.'),
            ('ingredient', 'Gui','Incorrect type. Expected pk value, received str.'),
            ('unit',       None, 'This field may not be null.'),
            ('unit',        999, '"999" is not a valid choice.'),
            ('unit',       '+-+-', '"+-+-" is not a valid choice.'),
            ('quantity',   None, 'This field may not be null.'),
            ('quantity', -0.3,  NumberBoundaryValidator._error_msg_pattern % (-0.3, 0.0, 'gt')),
            ('quantity', -0.0,  NumberBoundaryValidator._error_msg_pattern % (-0.0, 0.0, 'gt')),
            ('quantity',  0.0,  NumberBoundaryValidator._error_msg_pattern % ( 0.0, 0.0, 'gt')),
        ]
        self.serializer_kwargs['data'] = list(filter(lambda d: any(d['ingredients_applied']), self.request_data))
        for idx in range(len(self.serializer_kwargs['data'])):
            rand_chosen_idx_2 = random.randrange(0, len(self.serializer_kwargs['data'][idx]['ingredients_applied']))
            fn_choose_edit_item = lambda x : x[idx]['ingredients_applied'][rand_chosen_idx_2]
            self._loop_through_invalid_cases_common(fn_choose_edit_item, invalid_cases)


    def test_skip_given_attribute_id(self):
        invalid_cases = (12, '12')
        self.request_data = list(filter(lambda d: any(d['attributes']), self.request_data))
        serializer = SaleableItemSerializer( **self.serializer_kwargs )
        for invalid_case in invalid_cases:
            serializer.initial_data = copy.deepcopy(self.request_data[:1])
            serializer.initial_data[0]['attributes'][0]['id'] = invalid_case
            self.assertEqual(serializer.initial_data[0]['attributes'][0]['id'] , invalid_case)
            serializer.is_valid(raise_exception=True)
            with self.assertRaises(KeyError):
                serializer.validated_data[0]['attributes']
            for dtype_opt in  _ProductAttrValueDataType:
                field_name = dtype_opt.value[0][1]
                validated_attrs = serializer.validated_data[0].get(field_name, None)
                if validated_attrs:
                    with self.assertRaises(KeyError):
                        validated_id = validated_attrs[0]['id']


    def test_incorrect_attribute_value(self):
        _attr_fixture = {
         'null' :( None, 'unclassified attribute type `None`'),
         _ProductAttrValueDataType.STRING.value[0][0]           :'Lo0p',
         _ProductAttrValueDataType.INTEGER.value[0][0]          : -999 ,
         _ProductAttrValueDataType.POSITIVE_INTEGER.value[0][0] : 9999 ,
         _ProductAttrValueDataType.FLOAT.value[0][0]            : 99.9 ,
        }
        _allowed_type_transitions = [
            (_ProductAttrValueDataType.FLOAT.value[0][0], _ProductAttrValueDataType.INTEGER.value[0][0]),
            (_ProductAttrValueDataType.FLOAT.value[0][0], _ProductAttrValueDataType.POSITIVE_INTEGER.value[0][0]),
            (_ProductAttrValueDataType.INTEGER[0][0], _ProductAttrValueDataType.POSITIVE_INTEGER.value[0][0]),
            (_ProductAttrValueDataType.STRING.value[0][0], _ProductAttrValueDataType.INTEGER.value[0][0]         ),
            (_ProductAttrValueDataType.STRING.value[0][0], _ProductAttrValueDataType.POSITIVE_INTEGER.value[0][0]),
            (_ProductAttrValueDataType.STRING.value[0][0], _ProductAttrValueDataType.FLOAT.value[0][0]           ),
        ]
        num_rounds = 10
        field_name = 'value'
        expect_err_code = ('null', 'invalid', 'min_value')
        num_attr_fixture = len(_attr_fixture)
        self.request_data = list(filter(lambda d: any(d['attributes']), self.request_data))
        serializer = SaleableItemSerializer( **self.serializer_kwargs )
        for _ in range(num_rounds):
            serializer.initial_data = copy.deepcopy(self.request_data)
            idx_to_attrs = {}
            while len(idx_to_attrs.keys()) < num_attr_fixture:
                idx = random.randrange(0, len(serializer.initial_data))
                jdx = random.randrange(0, len(serializer.initial_data[idx]['attributes']))
                if idx_to_attrs.get((idx, jdx)) is None:
                    attrtype_id = serializer.initial_data[idx]['attributes'][jdx]['type']
                    attrtype = filter(lambda obj: obj.id == attrtype_id, self.stored_models['ProductAttributeType'])
                    attrtype = tuple(attrtype)[0]
                    dtype_keys = list(_attr_fixture.keys())
                    dtype_keys.remove(attrtype.dtype) # create invalid case by giving different data type of value
                    chosen_key = random.choice(dtype_keys)
                    if (attrtype.dtype, chosen_key) not in _allowed_type_transitions:
                        idx_to_attrs[(idx, jdx)] = (attrtype.dtype, chosen_key)
            fn_choose_edit_item = lambda x, idx, jdx : x[idx]['attributes'][jdx]
            for key, invalid_value in idx_to_attrs.items():
                req_data = fn_choose_edit_item(serializer.initial_data, key[0], key[1])
                req_data[field_name] = _attr_fixture[invalid_value[1]]
            error_details = self._assert_serializer_validation_error(serializer)
            for key, transition in idx_to_attrs.items():
                error_detail = fn_choose_edit_item(error_details, key[0], key[1])
                #if not error_detail:
                #    import pdb
                #    pdb.set_trace()
                self.assertTrue(any(error_detail))
                error_detail = error_detail[field_name]
                self.assertGreaterEqual(len(error_detail), 1)
                actual_err_code = error_detail[0].code
                self.assertIn(actual_err_code, expect_err_code)


    def test_unclassified_attribute_error(self):
        invalid_cases = [
            ('type',  None, 'unclassified attribute type `None`'),
            ('type', 'Lo0p','unclassified attribute type `Lo0p`'),
            ('type',  9999, 'unclassified attribute type `9999`'),
            ('type',  99.9, 'unclassified attribute type `99.9`'),
        ]
        self.request_data = list(filter(lambda d: any(d['attributes']), self.request_data))
        serializer = SaleableItemSerializer( **self.serializer_kwargs )
        for field_name, invalid_value, expect_err_msg in invalid_cases:
            for idx in range(len(self.request_data)):
                # serializer data has to be entirely reset for next iteration because it
                # reports the validation error for all list items in one go
                serializer.initial_data = copy.deepcopy(self.request_data)
                jdx = random.randrange(0, len(serializer.initial_data[idx]['attributes']))
                fn_choose_edit_item = lambda x : x[idx]['attributes'][jdx]
                req_data = fn_choose_edit_item(serializer.initial_data)
                self._assert_single_invalid_case(field_name, invalid_value, expect_err_msg,
                        req_data, serializer, fn_choose_edit_item)


    def test_unclassified_attributes_error(self):
        num_rounds = 10
        field_name = 'type'
        invalid_cases = ( 9999, '9q98', 9997,)
        expect_err_msg_pattern = 'unclassified attribute type `%s`'
        num_invalid_cases = len(invalid_cases)
        self.request_data = list(filter(lambda d: any(d['attributes']), self.request_data))
        serializer = SaleableItemSerializer( **self.serializer_kwargs )
        for _ in range(num_rounds):
            serializer.initial_data = copy.deepcopy(self.request_data)
            invalid_cases_iter = iter(invalid_cases)
            idx_to_attrs = {}
            while len(idx_to_attrs.keys()) < num_invalid_cases:
                idx = random.randrange(0, len(self.request_data))
                jdx = random.randrange(0, len(self.request_data[idx]['attributes']))
                if idx_to_attrs.get((idx,jdx)) is None:
                    idx_to_attrs[(idx,jdx)] = next(invalid_cases_iter)
            fn_choose_edit_item = lambda x, idx, jdx : x[idx]['attributes'][jdx]
            for key, invalid_value in idx_to_attrs.items():
                req_data = fn_choose_edit_item(serializer.initial_data, key[0], key[1])
                req_data[field_name] = invalid_value
            error_details = self._assert_serializer_validation_error(serializer)
            # the number of error details varies because django reports only one error
            # at a time even there are multiple errors in the serialized data , this test
            # only ensures at least one error(s) can be reported by Django.
            num_errors_catched = 0
            for key, invalid_value in idx_to_attrs.items():
                error_detail = fn_choose_edit_item(error_details, key[0], key[1])
                if not error_detail:
                    continue
                error_detail = error_detail[field_name]
                self.assertGreaterEqual(len(error_detail), 1)
                actual_err_msg = str(error_detail[0])
                expect_err_msg = expect_err_msg_pattern % invalid_value
                self.assertEqual(expect_err_msg, actual_err_msg)
                num_errors_catched += 1
            self.assertGreaterEqual(num_errors_catched, 1)
            self.assertLessEqual(num_errors_catched, num_invalid_cases)


    def _loop_through_invalid_cases_common(self, fn_choose_edit_item, invalid_cases, **kwargs):
        serializer = SaleableItemSerializer( **self.serializer_kwargs )
        req_data = fn_choose_edit_item( serializer.initial_data )
        for field_name, invalid_value, expect_err_msg in invalid_cases:
            self._assert_single_invalid_case(field_name, invalid_value, expect_err_msg,
                    req_data, serializer, fn_choose_edit_item)

    def _assert_single_invalid_case(self, field_name, invalid_value, expect_err_msg,
            req_data, serializer, fn_choose_edit_item):
        origin_value = req_data[field_name]
        req_data[field_name] = invalid_value
        error_details = self._assert_serializer_validation_error(serializer)
        req_data[field_name] = origin_value
        error_details = fn_choose_edit_item(error_details)
        error_details = error_details[field_name]
        self.assertGreaterEqual(len(error_details), 1)
        actual_err_msg = str(error_details[0])
        self.assertEqual(expect_err_msg, actual_err_msg)

    def _assert_serializer_validation_error(self, serializer):
        error_details = None
        possible_exception_classes = (DjangoValidationError, DRFValidationError, AssertionError)
        with self.assertRaises(possible_exception_classes):
            try:
                serializer.is_valid(raise_exception=True)
            except possible_exception_classes as e:
                error_details = e.detail
                raise
            finally:
                self.reset_validation_result(serializer=serializer)
        self.assertNotEqual(error_details, None)
        return error_details

## end of class SaleableItemCreationTestCase


class SaleableItemUpdateTestCase(SaleableItemCommonMixin, TransactionTestCase):
    def saleitems_data_gen(self, saved_items, dataset):
        dataset_limit = len(dataset)
        field_names = tuple(dataset[0].keys())
        for sale_item in saved_items:
            new_item = {fname: dataset[random.randrange(0, dataset_limit)][fname] for fname in field_names}
            new_item['id'] = sale_item.id
            yield new_item

    def setUp(self):
        super().setUp()
        # create new instances first
        serializer_kwargs_setup = {'data': self.request_data, 'many': True, 'usrprof_id': self.profile_ids[0],}
        serializer = SaleableItemSerializer( **serializer_kwargs_setup )
        serializer.is_valid(raise_exception=True)
        self._created_items = serializer.save()
        req_data_iter = iter(self.request_data)
        for instance in self._created_items:
            item = next(req_data_iter)
            item['id'] = instance.id


    def _refresh_edit_data(self, num_edit_items:int):
        data_gen = self.saleitems_data_gen(saved_items=self._created_items[:num_edit_items],
                    dataset = model_fixtures['ProductSaleableItem'])
        new_request_data = rand_gen_request_body(
                customize_item_fn=self.customize_req_data_item,  data_gen=data_gen,
                template=http_request_body_template['ProductSaleableItem'])
        return list(new_request_data)

    def _refresh_saved_instance(self, num_edit_items:int):
        saleitem_ids = tuple(map(lambda obj:obj.pk, self._created_items[:num_edit_items]))
        return ProductSaleableItem.objects.filter(id__in=saleitem_ids)

    def test_bulk_ok_all_items(self):
        num_all_items = len(self.request_data)
        self._test_bulk_ok_certain_num_of_items(num_edit_items=num_all_items)

    def test_bulk_ok_some_items(self):
        num_edit_items = random.randrange(1, len(self.request_data))
        serializer, upadted_saleitems, _ = self._test_bulk_ok_certain_num_of_items(num_edit_items=num_edit_items)
        # double-check the instances which were not updated
        edited_ids = tuple(map(lambda x: x.pk, upadted_saleitems))
        unedited_objs  = tuple(filter(lambda x: x.id not in edited_ids, self._created_items))
        unedited_items = tuple(filter(lambda x: x['id'] not in edited_ids, self.request_data))
        tuple(map(lambda obj:obj.refresh_from_db(), unedited_objs))
        self.assertGreater(len(unedited_objs), 0)
        self.assertGreater(len(unedited_items), 0)
        self.assertEqual(len(unedited_items), len(unedited_objs))
        self.verify_objects(unedited_objs, unedited_items, usrprof_id=serializer.child.usrprof_id)

    def _test_bulk_ok_certain_num_of_items(self, num_edit_items):
        new_request_data =  self._refresh_edit_data(num_edit_items=num_edit_items)
        saved_items = self._refresh_saved_instance(num_edit_items=num_edit_items)
        serializer_kwargs = {'data': new_request_data, 'usrprof_id': self.profile_ids[0],
                'instance': saved_items, 'many': True}
        serializer = SaleableItemSerializer( **serializer_kwargs )
        serializer.is_valid(raise_exception=True)
        upadted_saleitems = serializer.save()
        expect_data = serializer_kwargs['data']
        upadted_saleitems_iter = iter(upadted_saleitems)
        for expect_item in expect_data:
            actual_obj = next(upadted_saleitems_iter)
            self.assertEqual(expect_item['id'], actual_obj.id)
        self.verify_objects(upadted_saleitems, expect_data, usrprof_id=serializer.child.usrprof_id)
        return serializer, upadted_saleitems, expect_data


    def test_edit_nested_field(self):
        model_cls = type(self._created_items[0])
        edit_objs = model_cls.objects.filter(pk=self._created_items[0].pk)
        serializer_ro_kwargs = {'instance': edit_objs, 'many': True}
        serializer_ro = SaleableItemSerializer( **serializer_ro_kwargs )
        edit_data = dict(copy.deepcopy(serializer_ro.data[0]))
        unadded = list(filter(lambda obj:obj.pk not in edit_data['tags'], self.stored_models['ProductTag']))
        if len(edit_data['tags']) > 1:
            edit_data['tags'].pop()
        if any(unadded):
            new_tag = unadded.pop()
            edit_data['tags'].append(new_tag.pk)
        unadded = list(filter(lambda obj:obj.pk not in edit_data['ingredients_applied'],  self.stored_models['ProductDevIngredient']))
        if any(edit_data['ingredients_applied']):
            edit_data['ingredients_applied'][0]['quantity'] = random.randrange(5,50)
        if len(edit_data['ingredients_applied']) > 1:
            edit_data['ingredients_applied'].pop()
        if any(unadded):
            new_item = self._gen_ingredient_composite(ingredient=unadded.pop())
            edit_data['ingredients_applied'].append(new_item)
        edit_data = [edit_data]
        serializer_kwargs = {'data': copy.deepcopy(edit_data),  'instance': edit_objs,
                'many': True, 'usrprof_id': self.profile_ids[0],}
        serializer = SaleableItemSerializer( **serializer_kwargs )
        serializer.is_valid(raise_exception=True)
        edited_objs = serializer.save()
        self.verify_objects(edited_objs, edit_data, usrprof_id=serializer.child.usrprof_id)


    def test_editdata_instances_not_matched(self):
        non_field_err_key = drf_default_settings['NON_FIELD_ERRORS_KEY']
        num_all_items = len(self.request_data)
        new_request_data =  self._refresh_edit_data(num_edit_items=num_all_items)
        saved_items = self._refresh_saved_instance(num_edit_items=num_all_items)
        self.assertGreaterEqual(len(new_request_data) , 2)
        discarded_id = new_request_data[-2]['id']
        new_request_data[-2]['id'] = new_request_data[-1]['id']
        serializer_kwargs = {'data': new_request_data, 'many': True,
                'usrprof_id': self.profile_ids[0],  'instance': saved_items}
        error_caught = None
        with self.assertRaises(DRFValidationError):
            try:
                serializer = SaleableItemSerializer( **serializer_kwargs )
                serializer.is_valid(raise_exception=True)
            except DRFValidationError as e:
                error_caught = e
                raise
        self.assertNotEqual(error_caught, None)
        err_detail = error_caught.detail[non_field_err_key][0]
        err_msg = str(err_detail)
        self.assertGreater(err_msg.find(str(discarded_id)), 0)


    def test_conflict_items(self):
        # error handling when multiple edit items with the same ID
        # are received at backend
        non_field_err_key = drf_default_settings['NON_FIELD_ERRORS_KEY']
        num_all_items = len(self.request_data)
        new_request_data =  self._refresh_edit_data(num_edit_items=num_all_items)
        saved_items = self._refresh_saved_instance(num_edit_items=num_all_items)
        self.assertGreaterEqual(len(new_request_data) , 2)
        discarded_id = new_request_data[-2]['id']
        new_request_data[-2]['id'] = new_request_data[-1]['id']
        saved_items = saved_items.exclude(pk=discarded_id)
        serializer_kwargs = {'data': new_request_data, 'many': True,
                'usrprof_id': self.profile_ids[0],  'instance': saved_items}
        error_caught = None
        with self.assertRaises(DRFValidationError):
            try:
                serializer = SaleableItemSerializer( **serializer_kwargs )
                serializer.is_valid(raise_exception=True)
            except DRFValidationError as e:
                error_caught = e
                raise
        self.assertNotEqual(error_caught, None)
        self.assertEqual(error_caught.status_code, 400)
        err_detail = error_caught.detail[non_field_err_key][0]
        err_info = json.loads(str(err_detail))
        self.assertEqual(err_detail.code, 'conflict')
        self.assertEqual(err_info['message'], 'duplicate item found in the list')
        self.assertEqual(err_info['field'], 'id')
        self.assertNotIn(discarded_id, err_info['value'])
        self.assertEqual(err_info['value'][-2], err_info['value'][-1])
## end of class SaleableItemUpdateTestCase


class SaleableItemRepresentationTestCase(SaleableItemCommonMixin, TransactionTestCase):
    def setUp(self):
        super().setUp()
        # create new instances
        serializer_kwargs_setup = {'data': copy.deepcopy(self.request_data),
                'many': True, 'usrprof_id': self.profile_ids[0],}
        serializer = SaleableItemSerializer( **serializer_kwargs_setup )
        serializer.is_valid(raise_exception=True)
        self.saved_saleitems = serializer.save()
        self._serializer = serializer

    def test_represent_all(self):
        actual_data = self._serializer.data
        expect_data = self.request_data
        self.verify_data(actual_data, expect_data, usrprof_id=self._serializer.child.usrprof_id)
        # create another serializer with saved instance of saleable item
        for idx in range(1, len(self.saved_saleitems)):
            selected_instances = self.saved_saleitems[:idx]
            serializer_kwargs = {'many': True, 'instance':selected_instances}
            serializer_ro = SaleableItemSerializer( **serializer_kwargs )
            actual_data = serializer_ro.data
            expect_data = self.request_data[:idx] # serailizer should keep the order
            self.verify_data(actual_data, expect_data, usrprof_id=serializer_ro.child.usrprof_id)

    def test_represent_partial_1(self):
        def field_check(field_name, value):
            if field_name in ('id', 'price'):
                self.assertGreater(value, 0)
        expect_fields = ['id', 'price', 'usrprof']
        self._test_represent_partial(expect_fields=expect_fields, field_check_fn=field_check)

    def test_represent_partial_2(self):
        def field_check(field_name, value):
            if field_name == 'tags':
                actual_cnt = ProductTag.objects.filter(pk__in=value).count()
                self.assertEqual(len(value), actual_cnt)
            elif  field_name == 'media':
                actual_resource_ids = tuple(filter(lambda rid: len(rid) > 1, value))
                self.assertEqual(len(value), actual_resource_ids)
        expect_fields = ['name', 'tags', 'media_set']
        self._test_represent_partial(expect_fields=expect_fields, field_check_fn=field_check)

    def test_represent_partial_3(self):
        def item_check(value):
            ingredient_ids = list(map(lambda d:d['ingredient'] , value['ingredients_applied']))
            actual_cnt = ProductDevIngredient.objects.filter(pk__in=ingredient_ids).count()
            self.assertEqual(len(value['ingredients_applied']), actual_cnt)
            sale_item = ProductSaleableItem.objects.get(id=value['id'])
            expect_composite = sale_item.ingredients_applied.values('quantity', 'unit', 'ingredient')
            _sort_key_fn = lambda d: d['ingredient']
            expect_composite = sorted(expect_composite, key=_sort_key_fn)
            actual_composite = sorted(value['ingredients_applied'], key=_sort_key_fn)
            self.assertListEqual(expect_composite, actual_composite)
        expect_fields = ['id', 'ingredients_applied']
        self._test_represent_partial(expect_fields=expect_fields, item_check_fn=item_check)

    def test_represent_partial_4(self):
        def item_check(value):
            attrtype_ids = map(lambda d:d['type'] , value['attributes'])
            actual_qset = ProductAttributeType.objects.filter(pk__in=attrtype_ids)
            self.assertEqual(len(value['attributes']), actual_qset.count())
            sale_item = ProductSaleableItem.objects.get(id=value['id'])
            for actual_attrval in value['attributes']:
                expect_attrval = None
                for dtype_option in _ProductAttrValueDataType:
                    field_name = dtype_option.value[0][1]
                    manager = getattr(sale_item, field_name, None)
                    if not manager:
                        continue
                    qset = manager.filter(id=actual_attrval['id'])
                    if qset.exists():
                        expect_attrval = qset.first()
                        break
                self.assertNotEqual(expect_attrval, None)
                self.assertEqual(expect_attrval.ingredient_id, value['id'])
                self.assertEqual(expect_attrval.attr_type.id, actual_attrval['type'])
                self.assertEqual(expect_attrval.value,        actual_attrval['value'])
                self.assertEqual(expect_attrval.extra_amount, actual_attrval.get('extra_amount', None))
            #import pdb
            #pdb.set_trace()
        expect_fields = ['id', 'attributes']
        self._test_represent_partial(expect_fields=expect_fields, item_check_fn=item_check)


    def _test_represent_partial(self, expect_fields, field_check_fn=None, item_check_fn=None):
        serializer_ro = SaleableItemSerializer(many=True, instance=self.saved_saleitems)
        mocked_request = Mock()
        mocked_request.query_params = {}
        serializer_ro.context['request'] = mocked_request
        serializer_ro.context['request'].query_params['fields'] = ','.join(expect_fields)
        actual_data = serializer_ro.data
        for ac_item in actual_data:
            ac_item_cp = copy.deepcopy(ac_item)
            for field_name in expect_fields:
                value = ac_item_cp.pop(field_name, None)
                self.assertNotEqual(value, None)
                if field_check_fn and callable(field_check_fn):
                    field_check_fn(field_name=field_name, value=value)
            self.assertDictEqual(ac_item_cp, {})
            if item_check_fn and callable(item_check_fn):
                ac_item_cp = copy.deepcopy(ac_item)
                item_check_fn(value=ac_item_cp)
## end of class SaleableItemRepresentationTestCase

