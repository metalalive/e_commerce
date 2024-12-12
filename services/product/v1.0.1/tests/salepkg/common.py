import random
import json
from functools import partial

from ecommerce_common.util import sort_nested_object
from product.models.base import (
    _ProductAttrValueDataType,
    ProductTag,
    ProductAttributeType,
    ProductSaleableItem,
    UnitOfMeasurement,
)
from product.serializers.base import SaleablePackageSerializer

from tests.common import (
    _fixtures,
    listitem_rand_assigner,
    _common_instances_setup,
    _get_inst_attr,
    assert_field_equal,
    HttpRequestDataGen,
    AttributeDataGenMixin,
    BaseVerificationMixin,
    AttributeAssertionMixin,
)


_attr_vals_fixture_map = {
    _ProductAttrValueDataType.STRING.value[0][0]: _fixtures["ProductAttributeValueStr"],
    _ProductAttrValueDataType.POSITIVE_INTEGER.value[0][0]: _fixtures[
        "ProductAttributeValuePosInt"
    ],
    _ProductAttrValueDataType.INTEGER.value[0][0]: _fixtures[
        "ProductAttributeValueInt"
    ],
    _ProductAttrValueDataType.FLOAT.value[0][0]: _fixtures[
        "ProductAttributeValueFloat"
    ],
}


def diff_attrvals(testcase, expect_d, actual_d):
    for dtype_opt in _ProductAttrValueDataType:
        expect_attrs = filter(
            lambda d: d["type"].dtype == dtype_opt.value[0][0], expect_d
        )
        expect_value = [exp.copy() for exp in expect_attrs]
        for exp in expect_value:
            exp["type"] = exp["type"].id
        mgr_field_name = dtype_opt.value[0][1]
        field_names = ["attr_type", "value", "_extra_charge__amount"]
        if any(expect_value) and "id" in expect_value[0].keys():
            field_names.append("id")
        actual_value = list(getattr(actual_d, mgr_field_name).values(*field_names))
        for ac in actual_value:
            ac["type"] = ac.pop("attr_type")
            extra_amount = ac.pop("_extra_charge__amount")
            if extra_amount:
                ac["extra_amount"] = extra_amount
        expect_value = sort_nested_object(expect_value)
        actual_value = sort_nested_object(actual_value)
        expect_value = json.dumps(expect_value)
        actual_value = json.dumps(actual_value)
        testcase.assertEqual(actual_value, expect_value)


def diff_composite(testcase, expect_d, actual_d, lower_elm_name, lower_elm_mgr_field):
    if not lower_elm_name or not lower_elm_mgr_field:
        return  # skip
    expect_compos = expect_d
    expect_value = [exp.copy() for exp in expect_compos]
    for exp in expect_value:
        if isinstance(exp["unit"], (tuple, list)):
            exp["unit"] = exp["unit"][0]
        lower_elm = exp[lower_elm_name]
        if isinstance(lower_elm, int):
            exp[lower_elm_name] = lower_elm  # might be object id
        else:
            exp[lower_elm_name] = lower_elm["obj"].id
    if any(expect_value):
        actual_value = getattr(actual_d, lower_elm_mgr_field).values(
            "unit", "quantity", lower_elm_name
        )
        expect_value = sort_nested_object(expect_value)
        actual_value = sort_nested_object(list(actual_value))
        expect_value = json.dumps(expect_value)
        actual_value = json.dumps(actual_value)
        testcase.assertEqual(actual_value, expect_value)


def diff_created_ingredients(
    testcase, expect_data, actual_data, lower_elm_names, lower_elm_mgr_fields
):
    expect_d_iter = iter(expect_data)
    lower_elm_name = lower_elm_names[0]
    lower_elm_names = lower_elm_names[1:]
    lower_elm_mgr_field = lower_elm_mgr_fields[0]
    lower_elm_mgr_fields = lower_elm_mgr_fields[1:]
    for actual_d in actual_data:
        expect_d = next(expect_d_iter)
        testcase.assertTrue(actual_d == expect_d["obj"])
        bound_assert_fn = partial(
            assert_field_equal,
            testcase=testcase,
            actual_obj=actual_d,
            expect_obj=expect_d["simple"],
        )
        tuple(map(bound_assert_fn, expect_d["simple"].keys()))
        expect_value = expect_d["nested"].get("tags", [])
        if any(expect_value):
            actual_value = actual_d.tags.all()
            testcase.assertSetEqual(set(actual_value), set(expect_value))
        expect_value = tuple(
            map(lambda d: d["media"], expect_d["nested"].get("media", []))
        )
        if any(expect_value):
            actual_value = actual_d.media_set.values_list("media", flat=True)
            testcase.assertSetEqual(set(actual_value), set(expect_value))
        diff_attrvals(testcase, expect_d["nested"].get("attrvals", []), actual_d)
        diff_composite(
            testcase,
            expect_d["nested"].get("composites", []),
            actual_d,
            lower_elm_name,
            lower_elm_mgr_field,
        )
        # -------------------------------
        # import pdb
        # pdb.set_trace()
        if not any(lower_elm_names):
            continue
        expect_compos = expect_d["nested"].get("composites", [])
        lower_elm_expect_data = list(
            map(lambda exp: exp[lower_elm_name], expect_compos)
        )
        lower_elm_expect_data = sorted(lower_elm_expect_data, key=lambda d: d["obj"].id)
        lower_elm_actual_data = getattr(actual_d, lower_elm_mgr_field).order_by("id")
        lower_elm_actual_data = list(
            map(lambda d: getattr(d, lower_elm_name), lower_elm_actual_data)
        )
        diff_created_ingredients(
            testcase,
            expect_data=lower_elm_expect_data,
            actual_data=lower_elm_actual_data,
            lower_elm_names=lower_elm_names,
            lower_elm_mgr_fields=lower_elm_mgr_fields,
        )


## end of class diff_created_ingredients


class HttpRequestDataGenSaleablePackage(HttpRequestDataGen, AttributeDataGenMixin):
    min_num_applied_tags = 0
    min_num_applied_media = 0
    min_num_applied_saleitems = 0
    _stored_models = {}

    def _refresh_tags(self, num):
        return self._refresh_prerequisite_elements(num=num, model_cls=ProductTag)

    def _refresh_attrtypes(self, num):
        return self._refresh_prerequisite_elements(
            num=num, model_cls=ProductAttributeType
        )

    def _refresh_saleitems(self, num):
        return self._refresh_prerequisite_elements(
            num=num, model_cls=ProductSaleableItem
        )

    def _refresh_prerequisite_elements(self, num, model_cls):
        stored_insts = self._stored_models.pop(model_cls.__name__, None)
        if stored_insts:
            stored_insts.clear()
        self._stored_models[model_cls.__name__] = None
        models_info = [(model_cls, num)]
        _common_instances_setup(self._stored_models, models_info)
        return self._stored_models[model_cls.__name__]

    def customize_req_data_item(self, item, **kwargs):
        applied_tags = listitem_rand_assigner(
            list_=self._stored_models["ProductTag"],
            min_num_chosen=self.min_num_applied_tags,
        )
        applied_tag_ids = map(lambda obj: obj.id, applied_tags)
        item["tags"].extend(applied_tag_ids)
        applied_media = listitem_rand_assigner(
            list_=_fixtures["ProductSaleableItemMedia"],
            min_num_chosen=self.min_num_applied_media,
        )
        applied_media = map(lambda m: m["media"], applied_media)
        item["media_set"].extend(applied_media)
        item["attributes"] = self.gen_attr_vals(
            extra_amount_enabled=True,
            attr_type_src=self._stored_models["ProductAttributeType"],
        )
        composite_gen = listitem_rand_assigner(
            list_=self._stored_models["ProductSaleableItem"],
            min_num_chosen=self.min_num_applied_saleitems,
        )
        item["saleitems_applied"] = list(
            map(self._gen_saleitem_composite, composite_gen)
        )

    def _gen_saleitem_composite(self, sale_item):
        chosen_idx = random.randrange(0, len(UnitOfMeasurement.choices))
        chosen_unit = UnitOfMeasurement.choices[chosen_idx][0]
        return {
            "sale_item": _get_inst_attr(sale_item, "id"),
            "unit": chosen_unit,
            "quantity": float(random.randrange(1, 25)),
        }

    def rand_gen_edit_data(self, editing_data):
        for item in editing_data:
            item["price"] = item["price"] * random.random() * 2
            applied_tags = listitem_rand_assigner(
                list_=self._stored_models["ProductTag"], min_num_chosen=0
            )
            item["tags"] = list(map(lambda obj: obj.id, applied_tags))
            applied_media = listitem_rand_assigner(
                list_=_fixtures["ProductSaleableItemMedia"], min_num_chosen=1
            )
            item["media_set"] = list(map(lambda d: d["media"], applied_media))
            composite_gen = listitem_rand_assigner(
                list_=self._stored_models["ProductSaleableItem"]
            )
            item["saleitems_applied"] = list(
                map(self._gen_saleitem_composite, composite_gen)
            )
            if any(item["attributes"]):
                item["attributes"].pop()


## end of  class HttpRequestDataGenSaleablePackage


class SaleablePackageVerificationMixin(BaseVerificationMixin, AttributeAssertionMixin):
    serializer_class = SaleablePackageSerializer

    def verify_data(self, actual_data, expect_data, usrprof_id, verify_id=False):
        _expect_data = {
            "ProductSaleablePackage": [],
            "ProductSaleableItem": [],
        }
        simple_field_names = [
            "visible",
            "name",
            "price",
            "usrprof",
        ]
        if verify_id is True:
            simple_field_names.append("id")
        for saleitem in self._stored_models["ProductSaleableItem"]:
            simple_fields = {
                fname: getattr(saleitem, fname) for fname in simple_field_names
            }
            nested_fields = {
                "tags": [],
                "attrvals": [],
                "media": [],
                "composites": [],
            }
            item = {"simple": simple_fields, "nested": nested_fields, "obj": saleitem}
            _expect_data["ProductSaleableItem"].append(item)

        actual_data_iter = iter(actual_data)
        for pkg_data in expect_data:
            pkg_obj = next(actual_data_iter)
            simple_fields = {
                fname: pkg_data[fname]
                for fname in simple_field_names
                if pkg_data.get(fname)
            }
            simple_fields["usrprof"] = usrprof_id
            tags = list(
                filter(
                    lambda obj: obj.id in pkg_data.get("tags", []),
                    self._stored_models["ProductTag"],
                )
            )
            media_meta = list(
                map(lambda v: {"media": v}, pkg_data.get("media_set", []))
            )
            attrvals = [d.copy() for d in pkg_data.get("attributes", [])]
            for attrval in attrvals:
                if verify_id is False:
                    attrval.pop("id", None)
                attrtype_objs = tuple(
                    filter(
                        lambda obj: obj.id == attrval["type"],
                        self._stored_models["ProductAttributeType"],
                    )
                )
                attrval["type"] = attrtype_objs[0]
            composites = [d.copy() for d in pkg_data.get("saleitems_applied", [])]
            for composite in composites:
                saleitem_data = tuple(
                    filter(
                        lambda d: d["obj"].id == composite["sale_item"],
                        _expect_data["ProductSaleableItem"],
                    )
                )
                composite["sale_item"] = saleitem_data[0]
            nested_fields = {
                "tags": tags,
                "media": media_meta,
                "attrvals": attrvals,
                "composites": composites,
            }
            item = {"simple": simple_fields, "nested": nested_fields, "obj": pkg_obj}
            _expect_data["ProductSaleablePackage"].append(item)
        diff_created_ingredients(
            testcase=self,
            expect_data=_expect_data["ProductSaleablePackage"],
            actual_data=actual_data,
            lower_elm_names=["sale_item", None],
            lower_elm_mgr_fields=["saleitems_applied", None],
        )


## end of class SaleablePackageVerificationMixin
