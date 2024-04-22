import random

from ecommerce_common.models.enums.django import UnitOfMeasurement
from product.models.base import (
    ProductTag,
    ProductAttributeType,
)
from product.models.development import ProductDevIngredient
from product.serializers.base import SaleableItemSerializer

from tests.common import (
    _fixtures,
    _modelobj_list_to_map,
    listitem_rand_assigner,
    _common_instances_setup,
    _get_inst_attr,
    HttpRequestDataGen,
    AttributeDataGenMixin,
    BaseVerificationMixin,
    AttributeAssertionMixin,
    _product_tag_closure_setup,
)


def _saleitem_related_instance_setup(
    stored_models, num_tags=None, num_attrtypes=None, num_ingredients=None
):
    model_fixtures = _fixtures
    if num_tags is None:
        num_tags = len(model_fixtures["ProductTag"])
    if num_attrtypes is None:
        num_attrtypes = len(model_fixtures["ProductAttributeType"])
    if num_ingredients is None:
        num_ingredients = len(model_fixtures["ProductDevIngredient"])
    models_info = [
        (ProductTag, num_tags),
        (ProductAttributeType, num_attrtypes),
        (ProductDevIngredient, num_ingredients),
    ]
    _common_instances_setup(out=stored_models, models_info=models_info)
    tag_map = _modelobj_list_to_map(stored_models["ProductTag"])
    stored_models["ProductTagClosure"] = _product_tag_closure_setup(
        tag_map=tag_map, data=model_fixtures["ProductTagClosure"]
    )


class HttpRequestDataGenSaleableItem(HttpRequestDataGen, AttributeDataGenMixin):
    min_num_applied_tags = 0
    min_num_applied_media = 0
    min_num_applied_ingredients = 0

    def customize_req_data_item(self, item):
        model_fixtures = _fixtures
        applied_tag = listitem_rand_assigner(
            list_=model_fixtures["ProductTag"], min_num_chosen=self.min_num_applied_tags
        )
        applied_tag = map(lambda item: item["id"], applied_tag)
        item["tags"].extend(applied_tag)
        applied_media = listitem_rand_assigner(
            list_=model_fixtures["ProductSaleableItemMedia"],
            min_num_chosen=self.min_num_applied_media,
        )
        applied_media = map(lambda m: m["media"], applied_media)
        item["media_set"].extend(applied_media)
        item["attributes"] = self.gen_attr_vals(extra_amount_enabled=True)
        num_ingredients = random.randrange(
            self.min_num_applied_ingredients,
            len(model_fixtures["ProductDevIngredient"]),
        )
        ingredient_composite_gen = listitem_rand_assigner(
            list_=model_fixtures["ProductDevIngredient"],
            min_num_chosen=num_ingredients,
            max_num_chosen=(num_ingredients + 1),
        )
        item["ingredients_applied"] = list(
            map(self._gen_ingredient_composite, ingredient_composite_gen)
        )

    ## end of customize_req_data_item()

    def _gen_ingredient_composite(self, ingredient):
        chosen_idx = random.randrange(0, len(UnitOfMeasurement.choices))
        chosen_unit = UnitOfMeasurement.choices[chosen_idx][0]
        return {
            "ingredient": _get_inst_attr(ingredient, "id"),
            "unit": chosen_unit,
            "quantity": float(random.randrange(1, 25)),
        }


## end of class HttpRequestDataGenSaleableItem


class SaleableItemVerificationMixin(BaseVerificationMixin, AttributeAssertionMixin):
    serializer_class = SaleableItemSerializer

    def _assert_simple_fields(
        self, check_fields, exp_sale_item, ac_sale_item, usrprof_id=None
    ):
        super()._assert_simple_fields(
            check_fields=check_fields,
            exp_sale_item=exp_sale_item,
            ac_sale_item=ac_sale_item,
        )
        if usrprof_id:
            self.assertEqual(_get_inst_attr(ac_sale_item, "usrprof"), usrprof_id)

    def _assert_tag_fields(self, exp_sale_item, ac_sale_item):
        expect_vals = exp_sale_item["tags"]
        if isinstance(ac_sale_item, dict):
            actual_vals = ac_sale_item["tags"]
        else:
            actual_vals = list(ac_sale_item.tags.values_list("pk", flat=True))
        self.assertSetEqual(set(expect_vals), set(actual_vals))

    def _assert_mediaset_fields(self, exp_sale_item, ac_sale_item):
        expect_vals = exp_sale_item["media_set"]
        if isinstance(ac_sale_item, dict):
            actual_vals = ac_sale_item["media_set"]
        else:
            actual_vals = list(ac_sale_item.media_set.values_list("media", flat=True))
        self.assertSetEqual(set(expect_vals), set(actual_vals))

    def _assert_ingredients_applied_fields(self, exp_sale_item, ac_sale_item):
        def sort_key_fn(x):
            return x["ingredient"]

        expect_vals = exp_sale_item["ingredients_applied"]
        if isinstance(ac_sale_item, dict):
            actual_vals = list(
                map(lambda d: dict(d), ac_sale_item["ingredients_applied"])
            )
            tuple(map(lambda d: d.pop("sale_item", None), actual_vals))
        else:
            actual_vals = list(
                ac_sale_item.ingredients_applied.values(
                    "ingredient", "unit", "quantity"
                )
            )
        expect_vals = sorted(expect_vals, key=sort_key_fn)
        actual_vals = sorted(actual_vals, key=sort_key_fn)
        self.assertListEqual(expect_vals, actual_vals)

    def _get_non_nested_fields(self, skip_id=True, skip_usrprof=True):
        check_fields = super()._get_non_nested_fields(skip_id=skip_id)
        if skip_usrprof:
            check_fields.remove("usrprof")
        return check_fields

    def verify_objects(self, actual_instances, expect_data, usrprof_id=None):
        non_nested_fields = self._get_non_nested_fields()
        expect_data = iter(expect_data)
        for ac_sale_item in actual_instances:
            exp_sale_item = next(expect_data)
            self._assert_simple_fields(
                non_nested_fields, exp_sale_item, ac_sale_item, usrprof_id
            )
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
            self._assert_simple_fields(
                non_nested_fields, exp_sale_item, ac_sale_item, usrprof_id
            )
            self._assert_tag_fields(exp_sale_item, ac_sale_item)
            self._assert_mediaset_fields(exp_sale_item, ac_sale_item)
            self._assert_ingredients_applied_fields(exp_sale_item, ac_sale_item)
            self._assert_product_attribute_fields(exp_sale_item, ac_sale_item)

    ## end of  def verify_data()


## end of class SaleableItemVerificationMixin
