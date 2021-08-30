import copy
import json
import random
from functools import partial

from django.db.models.constants import LOOKUP_SEP

from common.models.enums   import UnitOfMeasurement
from common.util.python  import import_module_string
from product.models.base import ProductTag, ProductTagClosure, ProductAttributeType, _ProductAttrValueDataType, ProductSaleableItem
from product.models.development import ProductDevIngredientType, ProductDevIngredient
from product.serializers.base import SaleableItemSerializer

from product.tests.common import _fixtures, http_request_body_template, _load_init_params, _modelobj_list_to_map, _dict_key_replace, _dict_kv_pair_evict, listitem_rand_assigner, _common_instances_setup, rand_gen_request_body, _get_inst_attr, assert_field_equal, HttpRequestDataGen



def _product_tag_closure_setup(tag_map, data):
    _gen_closure_node = lambda d :ProductTagClosure(
            id    = d['id'],  depth = d['depth'],
            ancestor   = tag_map[d['ancestor']]  ,
            descendant = tag_map[d['descendant']]
        )
    filtered_data = filter(lambda d: tag_map.get(d['ancestor']) , data)
    nodes = list(map(_gen_closure_node, filtered_data))
    ProductTagClosure.objects.bulk_create(nodes)
    return nodes


def _saleitem_related_instance_setup(stored_models, num_tags=None, num_attrtypes=None, num_ingredients=None):
    model_fixtures = _fixtures
    if num_tags is None:
        num_tags = len(model_fixtures['ProductTag'])
    if num_attrtypes is None:
        num_attrtypes = len(model_fixtures['ProductAttributeType'])
    if num_ingredients is None:
        num_ingredients = len(model_fixtures['ProductDevIngredient'])
    models_info = [
            (ProductTag, num_tags),
            (ProductAttributeType, num_attrtypes  ),
            (ProductDevIngredient, num_ingredients),
        ]
    _common_instances_setup(out=stored_models, models_info=models_info)
    tag_map = _modelobj_list_to_map(stored_models['ProductTag'])
    stored_models['ProductTagClosure'] = _product_tag_closure_setup(
        tag_map=tag_map, data=model_fixtures['ProductTagClosure'])


def assert_softdelete_items_exist(testcase, deleted_ids, remain_ids, model_cls_path, id_label='id'):
    model_cls = import_module_string(dotted_path=model_cls_path)
    changeset = model_cls.SOFTDELETE_CHANGESET_MODEL
    cset = changeset.objects.filter(object_id__in=deleted_ids)
    testcase.assertEqual(cset.count(), len(deleted_ids))
    all_ids = []
    all_ids.extend(deleted_ids)
    all_ids.extend(remain_ids)
    query_id_key = LOOKUP_SEP.join([id_label, 'in'])
    lookup_kwargs = {'with_deleted':True, query_id_key: all_ids}
    qset = model_cls.objects.filter(**lookup_kwargs)
    testcase.assertEqual(qset.count(), len(all_ids))
    lookup_kwargs.pop('with_deleted')
    qset = model_cls.objects.filter(**lookup_kwargs)
    testcase.assertEqual(qset.count(), len(remain_ids))
    testcase.assertSetEqual(set(qset.values_list(id_label, flat=True)), set(remain_ids))
    qset = model_cls.objects.get_deleted_set()
    testcase.assertSetEqual(set(deleted_ids) , set(qset.values_list(id_label, flat=True)))


class HttpRequestDataGenSaleableItem(HttpRequestDataGen):
    min_num_applied_tags = 0
    min_num_applied_media = 0
    min_num_applied_attrs = 0
    min_num_applied_ingredients = 0

    def customize_req_data_item(self, item):
        model_fixtures = _fixtures
        applied_tag = listitem_rand_assigner(list_=model_fixtures['ProductTag'],
                min_num_chosen=self.min_num_applied_tags)
        applied_tag = map(lambda item:item['id'], applied_tag)
        item['tags'].extend(applied_tag)
        applied_media = listitem_rand_assigner(list_=model_fixtures['ProductSaleableItemMedia'],
                min_num_chosen=self.min_num_applied_media)
        applied_media = map(lambda m: m['media'], applied_media)
        item['media_set'].extend(applied_media)
        num_attrvals    = random.randrange(self.min_num_applied_attrs, len(model_fixtures['ProductAttributeType']))
        attr_dtypes_gen = listitem_rand_assigner(list_=model_fixtures['ProductAttributeType'],
                min_num_chosen=num_attrvals, max_num_chosen=(num_attrvals + 1))
        bound_gen_attr_val = partial(self._gen_attr_val, extra_amount_enabled=True)
        item['attributes'] = list(map(bound_gen_attr_val, attr_dtypes_gen))
        num_ingredients = random.randrange(self.min_num_applied_ingredients,
                len(model_fixtures['ProductDevIngredient']))
        ingredient_composite_gen = listitem_rand_assigner(list_=model_fixtures['ProductDevIngredient'],
                min_num_chosen=num_ingredients, max_num_chosen=(num_ingredients + 1))
        item['ingredients_applied'] = list(map(self._gen_ingredient_composite, ingredient_composite_gen))
    ## end of customize_req_data_item()


    def _gen_attr_val(self, attrtype, extra_amount_enabled):
        model_fixtures = _fixtures
        nested_item = {'id':None, 'type':_get_inst_attr(attrtype,'id'), 'value': None,}
        _fn = lambda option: option.value[0][0] == _get_inst_attr(attrtype,'dtype')
        dtype_option = filter(_fn, _ProductAttrValueDataType)
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

    def _gen_ingredient_composite(self, ingredient):
        chosen_idx = random.randrange(0, len(UnitOfMeasurement.choices))
        chosen_unit = UnitOfMeasurement.choices[chosen_idx][0]
        return {'ingredient': _get_inst_attr(ingredient,'id'), 'unit': chosen_unit,
                'quantity': float(random.randrange(1,25))}

## end of class HttpRequestDataGenSaleableItem

class BaseVerificationMixin:
    serializer_class = None
    def verify_objects(self, actual_instances, expect_data,  **kwargs):
        raise NotImplementedError()

    def verify_data(self, actual_data, expect_data, **kwargs):
        raise NotImplementedError()


class SaleableItemVerificationMixin(BaseVerificationMixin):
    serializer_class = SaleableItemSerializer

    def _assert_simple_fields(self, check_fields,  exp_sale_item, ac_sale_item, usrprof_id=None):
        self.assertNotEqual(_get_inst_attr(ac_sale_item,'id'), None)
        self.assertGreater(_get_inst_attr(ac_sale_item,'id'), 0)
        bound_assert_fn = partial(assert_field_equal, testcase=self,  expect_obj=exp_sale_item, actual_obj=ac_sale_item)
        tuple(map(bound_assert_fn, check_fields))
        if usrprof_id:
            self.assertEqual(_get_inst_attr(ac_sale_item,'usrprof'), usrprof_id)

    def _assert_product_attribute_fields(self, exp_sale_item, ac_sale_item):
        key_evict_condition = lambda kv: (kv[0] not in ('id', 'ingredient_type', 'ingredient_id')) \
                and not (kv[0] == 'extra_amount' and kv[1] is None)
        bound_dict_kv_pair_evict = partial(_dict_kv_pair_evict,  cond_fn=key_evict_condition)
        bound_dict_key_replace = partial(_dict_key_replace, to_='extra_amount', from_='_extra_charge__amount')
        for dtype_option in _ProductAttrValueDataType:
            field_name = dtype_option.value[0][1]
            expect_vals = exp_sale_item.get(field_name, None)
            if not expect_vals:
                continue
            expect_vals = list(map(bound_dict_kv_pair_evict, expect_vals))
            manager = _get_inst_attr(ac_sale_item, field_name)
            actual_vals = manager.values('attr_type', 'value', '_extra_charge__amount')
            actual_vals = map(bound_dict_key_replace, actual_vals)
            actual_vals = list(map(bound_dict_kv_pair_evict, actual_vals))
            expect_vals = sorted(expect_vals, key=lambda x:x['attr_type'])
            actual_vals = sorted(actual_vals, key=lambda x:x['attr_type'])
            expect_vals = json.dumps(expect_vals, sort_keys=True)
            actual_vals = json.dumps(actual_vals, sort_keys=True)
            self.assertEqual(expect_vals, actual_vals)

    def _assert_tag_fields(self, exp_sale_item, ac_sale_item):
        expect_vals = exp_sale_item['tags']
        if isinstance(ac_sale_item, dict):
            actual_vals = ac_sale_item['tags']
        else:
            actual_vals = list(ac_sale_item.tags.values_list('pk', flat=True))
        self.assertSetEqual(set(expect_vals), set(actual_vals))

    def _assert_mediaset_fields(self, exp_sale_item, ac_sale_item):
        expect_vals = exp_sale_item['media_set']
        if isinstance(ac_sale_item, dict):
            actual_vals = ac_sale_item['media_set']
        else:
            actual_vals = list(ac_sale_item.media_set.values_list('media', flat=True))
        self.assertSetEqual(set(expect_vals), set(actual_vals))

    def _assert_ingredients_applied_fields(self, exp_sale_item, ac_sale_item):
        sort_key_fn = lambda x:x['ingredient']
        expect_vals = exp_sale_item['ingredients_applied']
        if isinstance(ac_sale_item, dict):
            actual_vals = list(map(lambda d: dict(d), ac_sale_item['ingredients_applied']))
            tuple(map(lambda d: d.pop('sale_item', None), actual_vals))
        else:
            actual_vals = list(ac_sale_item.ingredients_applied.values('ingredient','unit','quantity'))
        expect_vals = sorted(expect_vals, key=sort_key_fn)
        actual_vals = sorted(actual_vals, key=sort_key_fn)
        self.assertListEqual(expect_vals, actual_vals)

    def _get_non_nested_fields(self, skip_id=True, skip_usrprof=True):
        check_fields = copy.copy(self.serializer_class.Meta.fields)
        if skip_id:
            check_fields.remove('id')
        if skip_usrprof:
            check_fields.remove('usrprof')
        return check_fields

    def verify_objects(self, actual_instances, expect_data, usrprof_id=None):
        non_nested_fields = self._get_non_nested_fields()
        expect_data = iter(expect_data)
        for ac_sale_item in actual_instances:
            exp_sale_item = next(expect_data)
            self._assert_simple_fields(non_nested_fields, exp_sale_item, ac_sale_item, usrprof_id)
            self._assert_tag_fields(exp_sale_item, ac_sale_item)
            self._assert_mediaset_fields(exp_sale_item, ac_sale_item)
            self._assert_ingredients_applied_fields(exp_sale_item, ac_sale_item)
            self._assert_product_attribute_fields(exp_sale_item, ac_sale_item)
    ## end of  def verify_objects()


    def verify_data(self, actual_data, expect_data, usrprof_id=None):
        non_nested_fields = self._get_non_nested_fields()
        expect_data = iter(expect_data)
        for ac_sale_item in actual_data:
            exp_sale_item = next(expect_data)
            self._assert_simple_fields(non_nested_fields, exp_sale_item, ac_sale_item, usrprof_id)
            self._assert_tag_fields(exp_sale_item, ac_sale_item)
            self._assert_mediaset_fields(exp_sale_item, ac_sale_item)
            self._assert_ingredients_applied_fields(exp_sale_item, ac_sale_item)
            self._assert_product_attribute_fields(exp_sale_item, ac_sale_item)
    ## end of  def verify_data()
## end of class SaleableItemVerificationMixin


