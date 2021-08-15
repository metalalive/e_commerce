import random
import math
import copy
import json
from functools import partial, reduce

from django.test import TransactionTestCase, TestCase
from django.contrib.auth.models import User as AuthUser
from django.core.exceptions    import ValidationError as DjangoValidationError
from rest_framework.exceptions import ValidationError as DRFValidationError

from common.validators     import NumberBoundaryValidator, UnprintableCharValidator
from common.models.enums   import UnitOfMeasurement
from product.serializers.base import SaleableItemSerializer
from product.models.base import ProductTag, ProductTagClosure, ProductAttributeType, _ProductAttrValueDataType, ProductSaleableItem
from product.models.development import ProductDevIngredientType, ProductDevIngredient

from .common import _fixtures as model_fixtures, listitem_rand_assigner, _common_instances_setup, _load_init_params, _modelobj_list_to_map, _product_tag_closure_setup, _dict_key_replace, _dict_kv_pair_evict

mock_request_body_template = {
    'ProductSaleableItem': {
        'name': None,  'id': None, 'visible': None, 'price': None,
        'tags':[] ,
        'media_set':[],
        'attributes':[
            #{'id':None, 'type':None, 'value': None, 'extra_amount':None},
        ],
        'ingredients_applied': [
            #{'ingredient': None, 'unit': None, 'quantity': None},
        ]
    } # end of ProductSaleableItem
} # end of mock_request_body_template


def _saleitem_related_instance_setup(stored_models):
    models_info = [
            (ProductTag, len(model_fixtures['ProductTag'])),
            (ProductAttributeType, len(model_fixtures['ProductAttributeType'])),
            (ProductDevIngredient, len(model_fixtures['ProductDevIngredient'])),
        ]
    _common_instances_setup(out=stored_models, models_info=models_info)
    tag_map = _modelobj_list_to_map(stored_models['ProductTag'])
    stored_models['ProductTagClosure'] = _product_tag_closure_setup(
            tag_map=tag_map, data=model_fixtures['ProductTagClosure'])


def rand_gen_request_body(template, customize_item_fn, data_gen):
    def rand_gen_single_req(data):
        single_req_item = copy.deepcopy(template)
        single_req_item.update(data)
        customize_item_fn(single_req_item)
        return single_req_item
    return map(rand_gen_single_req, data_gen)


def _gen_attr_val(attrtype, extra_amount_enabled):
    nested_item = {'id':None, 'type':attrtype.pk, 'value': None,}
    dtype_option = filter(lambda option: option.value[0][0] == attrtype.dtype, _ProductAttrValueDataType)
    dtype_option = tuple(dtype_option)[0]
    field_name = dtype_option.value[0][1]
    field_descriptor = getattr(ProductSaleableItem, field_name)
    attrval_cls_name = field_descriptor.field.related_model.__name__
    value_list = model_fixtures[attrval_cls_name]
    chosen_idx = random.randrange(0, len(value_list))
    nested_item['value'] = value_list[chosen_idx]
    rand_enable_extra_amount = random.randrange(0, 2)
    if extra_amount_enabled and rand_enable_extra_amount > 0:
        extra_amount_list = model_fixtures['ProductAppliedAttributePrice']
        chosen_idx = random.randrange(0, len(extra_amount_list))
        nested_item['extra_amount'] = float(extra_amount_list[chosen_idx])
    return nested_item


def _gen_ingredient_composite(ingredient):
    chosen_idx = random.randrange(0, len(UnitOfMeasurement.choices))
    chosen_unit = UnitOfMeasurement.choices[chosen_idx][0]
    return {'ingredient': ingredient.pk, 'unit': chosen_unit,
            'quantity': float(random.randrange(1,25))}


def assert_field_equal(fname, testcase, expect_dict, actual_obj):
    expect_val = expect_dict[fname]
    actual_val = getattr(actual_obj, fname)
    testcase.assertEqual(expect_val, actual_val)


class ExtendedTestCaseMixin:
    def customize_req_data_item(self, item, **kwargs):
        raise NotImplementedError()

    def gen_users(self, num=1):
        usr_gen   = listitem_rand_assigner(list_=model_fixtures['AuthUser'], min_num_chosen=num)
        new_users = list(map(lambda item: AuthUser(id=item['id'], username=item['username'],
                        password=item['password'], is_superuser=False, is_staff=item['is_staff'],
                        is_active=item['is_active']), usr_gen))
        AuthUser.objects.bulk_create(new_users)
        return tuple(AuthUser.objects.all())

    def reset_validation_result(self, serializer):
        serializer._errors.clear()
        delattr(serializer, '_validated_data')


class SaleableItemCommonMixin(ExtendedTestCaseMixin):
    stored_models = {}
    num_users = 1

    def setUp(self):
        _saleitem_related_instance_setup(self.stored_models)
        self.users = self.gen_users(num=self.num_users)
        saleitems_data_gen = listitem_rand_assigner(list_=model_fixtures['ProductSaleableItem'])
        self.request_data = rand_gen_request_body(customize_item_fn=self.customize_req_data_item,
                data_gen=saleitems_data_gen,  template=mock_request_body_template['ProductSaleableItem'])
        self.request_data = list(self.request_data)

    def tearDown(self):
        self.stored_models.clear()

    def customize_req_data_item(self, item):
        applied_tag = listitem_rand_assigner(list_=self.stored_models['ProductTag'], min_num_chosen=0)
        applied_tag = map(lambda obj:obj.pk, applied_tag)
        item['tags'].extend(applied_tag)
        applied_media = listitem_rand_assigner(list_=model_fixtures['ProductSaleableItemMedia'], min_num_chosen=0)
        applied_media = map(lambda m: m['media'], applied_media)
        item['media_set'].extend(applied_media)
        num_attrvals    = random.randrange(0, len(self.stored_models['ProductAttributeType']))
        attr_dtypes_gen = listitem_rand_assigner(list_=self.stored_models['ProductAttributeType'],
                min_num_chosen=num_attrvals, max_num_chosen=(num_attrvals + 1))
        bound_gen_attr_val = partial(_gen_attr_val, extra_amount_enabled=True)
        item['attributes'] = list(map(bound_gen_attr_val, attr_dtypes_gen))
        num_ingredients = random.randrange(0, len(self.stored_models['ProductDevIngredient']))
        ingredient_composite_gen = listitem_rand_assigner(list_=self.stored_models['ProductDevIngredient'],
                min_num_chosen=num_ingredients, max_num_chosen=(num_ingredients + 1))
        item['ingredients_applied'] = list(map(_gen_ingredient_composite, ingredient_composite_gen))
    ## end of customize_req_data_item()

    def assert_after_serializer_save(self, serializer, actual_instances, expect_data):
        expect_data = iter(expect_data)
        key_evict_condition = lambda kv: (kv[0] not in ('id', 'ingredient_type', 'ingredient_id')) \
                and not (kv[0] == 'extra_amount' and kv[1] is None)
        bound_dict_key_replace = partial(_dict_key_replace, to_='extra_amount', from_='_extra_charge__amount')
        bound_dict_kv_pair_evict = partial(_dict_kv_pair_evict,  cond_fn=key_evict_condition)
        for ac_sale_item in actual_instances:
            self.assertNotEqual(ac_sale_item.id, None)
            self.assertGreater(ac_sale_item.id, 0)
            exp_sale_item = next(expect_data)
            check_fields = copy.copy(serializer.child.Meta.fields)
            check_fields.remove('id')
            bound_assert_fn = partial(assert_field_equal, testcase=self,  expect_dict=exp_sale_item, actual_obj=ac_sale_item)
            tuple(map(bound_assert_fn, check_fields))
            expect_vals = exp_sale_item['tags']
            actual_vals = list(ac_sale_item.tags.values_list('pk', flat=True))
            self.assertSetEqual(set(expect_vals), set(actual_vals))
            expect_vals = exp_sale_item['media_set']
            actual_vals = list(ac_sale_item.media_set.values_list('media', flat=True))
            diff = set(expect_vals).symmetric_difference(actual_vals)
            self.assertSetEqual(set(expect_vals), set(actual_vals))
            expect_vals = exp_sale_item['ingredients_applied']
            actual_vals = list(ac_sale_item.ingredients_applied.values('ingredient','unit','quantity'))
            expect_vals = sorted(expect_vals, key=lambda x:x['ingredient'])
            actual_vals = sorted(actual_vals, key=lambda x:x['ingredient'])
            self.assertListEqual(expect_vals, actual_vals)
            # attributes check
            for dtype_option in _ProductAttrValueDataType:
                field_name = dtype_option.value[0][1]
                expect_vals = exp_sale_item.get(field_name, None)
                if not expect_vals:
                    continue
                expect_vals = list(map(bound_dict_kv_pair_evict, expect_vals))
                actual_vals = getattr(ac_sale_item, field_name).values('attr_type', 'value', '_extra_charge__amount')
                actual_vals = map(bound_dict_key_replace, actual_vals)
                actual_vals = list(map(bound_dict_kv_pair_evict, actual_vals))
                expect_vals = sorted(expect_vals, key=lambda x:x['attr_type'])
                actual_vals = sorted(actual_vals, key=lambda x:x['attr_type'])
                expect_vals = json.dumps(expect_vals, sort_keys=True)
                actual_vals = json.dumps(actual_vals, sort_keys=True)
                self.assertEqual(expect_vals, actual_vals)

## end of class SaleableItemCommonMixin


class SaleableItemCreationTestCase(SaleableItemCommonMixin, TransactionTestCase):
    def setUp(self):
        super().setUp()
        self.serializer_kwargs = {'data': self.request_data, 'many': True, 'account': self.users[0],}

    def test_bulk_ok(self):
        serializer = SaleableItemSerializer( **self.serializer_kwargs )
        serializer.is_valid(raise_exception=True)
        actual_instances = serializer.save()
        expect_data = self.serializer_kwargs['data']
        self.assert_after_serializer_save(serializer, actual_instances, expect_data)

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
        self.serializer_kwargs['data'] = self.request_data[:1]
        req_data = self.serializer_kwargs['data'][0]
        serializer = SaleableItemSerializer( **self.serializer_kwargs )
        for field_name, invalid_value, expect_err_msg in invalid_cases:
            origin_value = req_data[field_name]
            req_data[field_name] = invalid_value
            error_details = self._assert_serializer_validation_error(serializer)
            req_data[field_name] = origin_value
            self.assertEqual(len(error_details), 1)
            error_details = error_details[0][field_name]
            self.assertGreaterEqual(len(error_details), 1)
            actual_err_msg = str(error_details[0])
            self.assertEqual(expect_err_msg, actual_err_msg)

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
        req_data = self.serializer_kwargs['data'][0]['ingredients_applied'][0]
        serializer = SaleableItemSerializer( **self.serializer_kwargs )
        for field_name, invalid_value, expect_err_msg in invalid_cases:
            origin_value = req_data[field_name]
            req_data[field_name] = invalid_value
            error_details = self._assert_serializer_validation_error(serializer)
            req_data[field_name] = origin_value
            error_details = error_details[0]['ingredients_applied'][0][field_name]
            self.assertGreaterEqual(len(error_details), 1)
            actual_err_msg = str(error_details[0])
            self.assertEqual(expect_err_msg, actual_err_msg)

    def test_attributes_validate_error(self):
        pass

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

    def setUp(self):
        super().setUp()
        # create new instances first
        serializer_kwargs_setup = {'data': self.request_data, 'many': True, 'account': self.users[0],}
        serializer = SaleableItemSerializer( **serializer_kwargs_setup )
        serializer.is_valid(raise_exception=True)
        saved_saleitems = serializer.save()
        # prepare for later update
        def saleitems_data_gen():
            dataset = model_fixtures['ProductSaleableItem']
            dataset_limit = len(dataset)
            field_names = tuple(dataset[0].keys())
            for sale_item in saved_saleitems:
                new_item = {fname: dataset[random.randrange(0, dataset_limit)][fname] for fname in field_names}
                new_item['id'] = sale_item.id
                yield new_item
        self.new_request_data = rand_gen_request_body(customize_item_fn=self.customize_req_data_item,
                data_gen=saleitems_data_gen(),  template=mock_request_body_template['ProductSaleableItem'])
        self.new_request_data = list(self.new_request_data)
        saleitem_ids = tuple(map(lambda obj:obj.pk, saved_saleitems))
        saved_saleitems = ProductSaleableItem.objects.filter(id__in=saleitem_ids)
        self.serializer_kwargs = {'data': self.new_request_data, 'account': self.users[0],
                'instance': saved_saleitems, 'many': True}

    def test_bulk_update_ok(self):
        serializer = SaleableItemSerializer( **self.serializer_kwargs )
        serializer.is_valid(raise_exception=True)
        upadted_saleitems = serializer.save()
        expect_data = self.serializer_kwargs['data']
        self.assert_after_serializer_save(serializer, upadted_saleitems, expect_data)


## end of class SaleableItemUpdateTestCase


