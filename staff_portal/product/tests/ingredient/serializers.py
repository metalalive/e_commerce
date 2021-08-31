import random
import copy
import json
from functools import partial, reduce
from unittest.mock import Mock

from django.test import TransactionTestCase
from django.core.exceptions    import ValidationError as DjangoValidationError
from rest_framework.exceptions import ValidationError as DRFValidationError
from rest_framework.settings import DEFAULTS as drf_default_settings

from common.util.python import sort_nested_object
from product.models.base import ProductAttributeType, _ProductAttrValueDataType
from product.models.development import ProductDevIngredientType, ProductDevIngredient

from product.tests.common import _fixtures, _common_instances_setup, listitem_rand_assigner, rand_gen_request_body, http_request_body_template, reset_serializer_validation_result
from .common import HttpRequestDataGenDevIngredient, DevIngredientVerificationMixin

_attr_vals_fixture_map = {
    _ProductAttrValueDataType.STRING.value[0][0]: _fixtures['ProductAttributeValueStr'],
    _ProductAttrValueDataType.POSITIVE_INTEGER.value[0][0]: _fixtures['ProductAttributeValuePosInt'],
    _ProductAttrValueDataType.INTEGER.value[0][0]: _fixtures['ProductAttributeValueInt'],
    _ProductAttrValueDataType.FLOAT.value[0][0]: _fixtures['ProductAttributeValueFloat'],
}


class DevIngredientCommonMixin(HttpRequestDataGenDevIngredient, DevIngredientVerificationMixin):
    stored_models = {}
    num_ingredients_created = 2

    def setUp(self):
        num_attrtypes = len(_fixtures['ProductAttributeType'])
        models_info = [(ProductAttributeType, num_attrtypes),]
        _common_instances_setup(out=self.stored_models, models_info=models_info)
        ingredients_data_gen = listitem_rand_assigner(list_=_fixtures['ProductDevIngredient'],
                min_num_chosen=self.num_ingredients_created)
        self.request_data = rand_gen_request_body(customize_item_fn=self.customize_req_data_item,
                data_gen=ingredients_data_gen,  template=http_request_body_template['ProductDevIngredient'])
        self.request_data = list(self.request_data)

    def tearDown(self):
        self.stored_models.clear()
## end of class DevIngredientCommonMixin


class DevIngredientCreationTestCase(DevIngredientCommonMixin, TransactionTestCase):

    def test_bulk_ok(self):
        serializer_kwargs = {'data': copy.deepcopy(self.request_data), 'many': True}
        serializer = self.serializer_class( **serializer_kwargs )
        serializer.is_valid(raise_exception=True)
        self._assert_attributes_data_change(data_before_validate=self.request_data,
                data_after_validate=serializer_kwargs['data'])
        actual_instances = serializer.save()
        expect_data = serializer_kwargs['data']
        self.verify_objects(actual_instances, expect_data)


    def test_fields_validate_error(self):
        num_ingredients = len(self.request_data)
        invalid_cases = [
            ('name', None, 'This field may not be null.'),
            ('name', '',   'This field may not be blank.'),
            ('category', None, 'This field may not be null.'),
            ('category', 999,  '"999" is not a valid choice.'),
            ('category', '+-+-', '"+-+-" is not a valid choice.'),
        ]
        for field_name, invalid_value, expect_err_msg in invalid_cases:
            serializer_kwargs = {'data': copy.deepcopy(self.request_data), 'many': True}
            serializer = self.serializer_class( **serializer_kwargs )
            chosen_init_data_idx = random.randrange(0, num_ingredients)
            fn_choose_edit_item = lambda data: data[chosen_init_data_idx]
            req_data = fn_choose_edit_item(serializer.initial_data)
            self._assert_single_invalid_case(testcase=self, field_name=field_name, invalid_value=invalid_value,
                    expect_err_msg=expect_err_msg,  req_data=req_data, serializer=serializer,
                    fn_choose_edit_item=fn_choose_edit_item)


    def test_unclassified_attribute_error(self):
        serializer_kwargs = {'data': copy.deepcopy(self.request_data), 'many': True}
        serializer = self.serializer_class( **serializer_kwargs )
        self._test_unclassified_attribute_error(testcase=self, serializer=serializer, request_data=self.request_data,
            assert_single_invalid_case_fn=self._assert_single_invalid_case )

    def test_unclassified_attributes_error(self):
        serializer_kwargs = {'data': copy.deepcopy(self.request_data), 'many': True}
        serializer = self.serializer_class( **serializer_kwargs )
        self._test_unclassified_attributes_error(serializer, self.request_data, testcase=self,
                assert_serializer_validation_error_fn=self._assert_serializer_validation_error)

    def test_incorrect_attribute_value(self):
        serializer_kwargs = {'data': copy.deepcopy(self.request_data), 'many': True}
        serializer = self.serializer_class( **serializer_kwargs )
        self._test_incorrect_attribute_value(serializer, self.request_data, testcase=self,
                assert_serializer_validation_error_fn=self._assert_serializer_validation_error )


class DevIngredientBaseUpdateTestCase(DevIngredientCommonMixin, TransactionTestCase):
    def setUp(self):
        super().setUp()
        # create new instances first
        serializer_kwargs_setup = {'data': self.request_data, 'many': True,}
        serializer = self.serializer_class( **serializer_kwargs_setup )
        serializer.is_valid(raise_exception=True)
        self._created_items = serializer.save()
        self.request_data = list(map(dict, serializer.data))


class DevIngredientUpdateTestCase(DevIngredientBaseUpdateTestCase):
    max_num_applied_attrs = 4
    min_num_applied_attrs = 2
    num_ingredients_created = 4

    def setUp(self):
        super().setUp()
        num_edit_data = len(self.request_data) >> 1
        self.editing_data    = copy.deepcopy(self.request_data[:num_edit_data])
        self.unaffected_data = self.request_data[num_edit_data:]
        new_name_options = ('dill oil', 'transparent water pipe', 'needle', 'cocona powder')
        name_gen = listitem_rand_assigner(list_=new_name_options, distinct=False,
                min_num_chosen=num_edit_data , max_num_chosen=(num_edit_data + 1))
        category_gen = listitem_rand_assigner(list_=ProductDevIngredientType.choices, distinct=False,
                min_num_chosen=num_edit_data , max_num_chosen=(num_edit_data + 1))
        for edit_item in self.editing_data:
            edit_item['name'] = next(name_gen)
            edit_item['category'] = next(category_gen)[0]
            edit_attr = edit_item['attributes'][0]
            old_attr_value = edit_attr['value']
            is_text = isinstance(old_attr_value, str)
            edit_attr['value'] = 'new value is a text' if is_text else (old_attr_value * 2)
            added_attrtype_ids  = tuple(map(lambda x:x['type'], edit_item['attributes']))
            unadded_attrtypes   = tuple(filter(lambda x:x['id'] not in added_attrtype_ids, _fixtures['ProductAttributeType']))
            new_attrtype = unadded_attrtypes[0]
            new_value_opts = _attr_vals_fixture_map[new_attrtype['dtype']]
            new_attr = {'type':new_attrtype['id'], 'value': new_value_opts[0]}
            edit_item['attributes'].pop()
            edit_item['attributes'].append(new_attr)

        editing_ids = tuple(map(lambda x:x['id'], self.editing_data))
        self.edit_objs =  self.serializer_class.Meta.model.objects.filter(id__in=editing_ids)
    ## end of setUp()


    def test_bulk_ok_some_items(self):
        serializer_kwargs = {'data': copy.deepcopy(self.editing_data),  'instance': self.edit_objs, 'many': True,}
        serializer = self.serializer_class( **serializer_kwargs )
        serializer.is_valid(raise_exception=True)
        self._assert_attributes_data_change(data_before_validate=self.editing_data,
                data_after_validate=serializer_kwargs['data'])
        edited_objs = serializer.save()
        self.verify_objects(actual_instances=edited_objs, expect_data=serializer_kwargs['data'])
        # check unaffected items
        unaffected_ids = tuple(map(lambda x:x['id'], self.unaffected_data))
        unaffected_objs =  self.serializer_class.Meta.model.objects.filter(id__in=unaffected_ids)
        serializer_ro = self.serializer_class(instance=unaffected_objs, many=True)
        expect_unaffected_data = sort_nested_object( self.unaffected_data )
        actual_unaffected_data = sort_nested_object( serializer_ro.data )
        expect_unaffected_data = json.dumps(expect_unaffected_data, sort_keys=True)
        actual_unaffected_data = json.dumps(actual_unaffected_data, sort_keys=True)
        self.assertEqual(expect_unaffected_data, actual_unaffected_data)

    def test_conflict_items(self):
        self.assertGreaterEqual(len(self.editing_data) , 2)
        discarded_id = self.editing_data[-2]['id']
        self.editing_data[-2]['id'] = self.editing_data[-1]['id']
        edit_objs = self.edit_objs.exclude(pk=discarded_id)
        serializer_kwargs = {'data': self.editing_data, 'many': True, 'instance': edit_objs}
        error_caught = None
        with self.assertRaises(DRFValidationError):
            try:
                serializer = self.serializer_class( **serializer_kwargs )
                serializer.is_valid(raise_exception=True)
            except DRFValidationError as e:
                error_caught = e
                raise
        self.assertNotEqual(error_caught, None)
        non_field_err_key = drf_default_settings['NON_FIELD_ERRORS_KEY']
        err_detail = error_caught.detail[non_field_err_key][0]
        err_info = json.loads(str(err_detail))
        self.assertEqual(err_detail.code, 'conflict')
        self.assertEqual(err_info['message'], 'duplicate item found in the list')
        self.assertEqual(err_info['field'], 'id')
        self.assertNotIn(discarded_id, err_info['value'])
        self.assertEqual(err_info['value'][-2], err_info['value'][-1])
## end of class DevIngredientUpdateTestCase


class DevIngredientRepresentationTestCase(DevIngredientBaseUpdateTestCase):
    def test_represent_all(self):
        created_ids  = tuple(map(lambda x:x.id, self._created_items))
        created_objs = self.serializer_class.Meta.model.objects.filter(id__in=created_ids)
        serializer_ro = self.serializer_class(instance=created_objs, many=True)
        actual_data = serializer_ro.data
        expect_data = self.request_data
        self.verify_data(actual_data, expect_data)



