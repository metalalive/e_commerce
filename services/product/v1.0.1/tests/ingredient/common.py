import random

from product.models.base import  _ProductAttrValueDataType
from product.models.development import ProductDevIngredientType
from product.serializers.development import FabricationIngredientSerializer
from tests.common import _fixtures, listitem_rand_assigner, HttpRequestDataGen, AttributeDataGenMixin, BaseVerificationMixin, AttributeAssertionMixin


_attr_vals_fixture_map = {
    _ProductAttrValueDataType.STRING.value[0][0]: _fixtures['ProductAttributeValueStr'],
    _ProductAttrValueDataType.POSITIVE_INTEGER.value[0][0]: _fixtures['ProductAttributeValuePosInt'],
    _ProductAttrValueDataType.INTEGER.value[0][0]: _fixtures['ProductAttributeValueInt'],
    _ProductAttrValueDataType.FLOAT.value[0][0]: _fixtures['ProductAttributeValueFloat'],
}

class HttpRequestDataGenDevIngredient(HttpRequestDataGen, AttributeDataGenMixin):
    def customize_req_data_item(self, item):
        item['attributes'] = self.gen_attr_vals(extra_amount_enabled=False)

    def rand_gen_edit_data(self, editing_data):
        new_name_options = ('dill oil', 'transparent water pipe', 'needle', 'cocona powder')
        num_edit_data = len(editing_data)
        name_gen = listitem_rand_assigner(list_=new_name_options, distinct=False,
                min_num_chosen=num_edit_data , max_num_chosen=(num_edit_data + 1))
        category_gen = listitem_rand_assigner(list_=ProductDevIngredientType.choices, distinct=False,
                min_num_chosen=num_edit_data , max_num_chosen=(num_edit_data + 1))
        for edit_item in editing_data:
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


class DevIngredientVerificationMixin(BaseVerificationMixin, AttributeAssertionMixin):
    serializer_class = FabricationIngredientSerializer

    def verify_objects(self, actual_instances, expect_data, extra_check_fn=None, non_nested_fields=None):
        non_nested_fields = non_nested_fields or self._get_non_nested_fields()
        expect_data = iter(expect_data)
        for ac_item in actual_instances:
            exp_item = next(expect_data)
            self._assert_simple_fields(non_nested_fields, exp_item, ac_item)
            self._assert_product_attribute_fields(exp_item, ac_item)
            if extra_check_fn and callable(extra_check_fn):
                extra_check_fn(exp_item=exp_item, ac_item=ac_item)


