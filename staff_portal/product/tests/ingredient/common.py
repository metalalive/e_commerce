import random

from product.serializers.development import FabricationIngredientSerializer
from product.tests.common import _fixtures, HttpRequestDataGen, AttributeDataGenMixin, BaseVerificationMixin, AttributeAssertionMixin

class HttpRequestDataGenDevIngredient(HttpRequestDataGen, AttributeDataGenMixin):
    def customize_req_data_item(self, item):
        item['attributes'] = self.gen_attr_vals(extra_amount_enabled=False)


class DevIngredientVerificationMixin(BaseVerificationMixin, AttributeAssertionMixin):
    serializer_class = FabricationIngredientSerializer

    def verify_objects(self, actual_instances, expect_data):
        non_nested_fields = self._get_non_nested_fields()
        expect_data = iter(expect_data)
        for ac_sale_item in actual_instances:
            exp_sale_item = next(expect_data)
            self._assert_simple_fields(non_nested_fields, exp_sale_item, ac_sale_item)
            self._assert_product_attribute_fields(exp_sale_item, ac_sale_item)

    def verify_data(self, actual_data, expect_data):
        return self.verify_objects(actual_instances=actual_data, expect_data=expect_data)


