import random
import copy
import json
from unittest.mock import Mock

from django.test import TransactionTestCase
from rest_framework.exceptions import ValidationError as DRFValidationError
from rest_framework.settings import DEFAULTS as drf_default_settings

from ecommerce_common.util import sort_nested_object
from product.models.base import ProductAttributeType, _ProductAttrValueDataType

from tests.common import (
    _fixtures,
    _get_inst_attr,
    _common_instances_setup,
    listitem_rand_assigner,
    rand_gen_request_body,
    http_request_body_template,
    reset_serializer_validation_result,
)
from .common import HttpRequestDataGenDevIngredient, DevIngredientVerificationMixin


class DevIngredientCommonMixin(
    HttpRequestDataGenDevIngredient, DevIngredientVerificationMixin
):
    stored_models = {}
    num_ingredients_created = 2

    def setUp(self):
        num_attrtypes = len(_fixtures["ProductAttributeType"])
        models_info = [
            (ProductAttributeType, num_attrtypes),
        ]
        _common_instances_setup(out=self.stored_models, models_info=models_info)
        ingredients_data_gen = listitem_rand_assigner(
            list_=_fixtures["ProductDevIngredient"],
            min_num_chosen=self.num_ingredients_created,
        )
        self.request_data = rand_gen_request_body(
            customize_item_fn=self.customize_req_data_item,
            data_gen=ingredients_data_gen,
            template=http_request_body_template["ProductDevIngredient"],
        )
        self.request_data = list(self.request_data)

    def tearDown(self):
        self.stored_models.clear()


## end of class DevIngredientCommonMixin


class DevIngredientCreationTestCase(DevIngredientCommonMixin, TransactionTestCase):

    def test_bulk_ok(self):
        serializer_kwargs = {"data": copy.deepcopy(self.request_data), "many": True}
        serializer = self.serializer_class(**serializer_kwargs)
        serializer.is_valid(raise_exception=True)
        self._assert_attributes_data_change(
            data_before_validate=self.request_data,
            data_after_validate=serializer_kwargs["data"],
        )
        actual_instances = serializer.save()
        expect_data = serializer_kwargs["data"]
        self.verify_objects(actual_instances, expect_data)

    def test_fields_validate_error(self):
        num_ingredients = len(self.request_data)
        invalid_cases = [
            ("name", None, "This field may not be null."),
            ("name", "", "This field may not be blank."),
            ("category", None, "This field may not be null."),
            ("category", 999, '"999" is not a valid choice.'),
            ("category", "+-+-", '"+-+-" is not a valid choice.'),
        ]
        for field_name, invalid_value, expect_err_msg in invalid_cases:
            serializer_kwargs = {"data": copy.deepcopy(self.request_data), "many": True}
            serializer = self.serializer_class(**serializer_kwargs)
            chosen_init_data_idx = random.randrange(0, num_ingredients)

            def fn_choose_edit_item(data):
                nonlocal chosen_init_data_idx
                return data[chosen_init_data_idx]

            req_data = fn_choose_edit_item(serializer.initial_data)
            self._assert_single_invalid_case(
                testcase=self,
                field_name=field_name,
                invalid_value=invalid_value,
                expect_err_msg=expect_err_msg,
                req_data=req_data,
                serializer=serializer,
                fn_choose_edit_item=fn_choose_edit_item,
            )

    def test_unclassified_attribute_error(self):
        serializer_kwargs = {"data": copy.deepcopy(self.request_data), "many": True}
        serializer = self.serializer_class(**serializer_kwargs)
        self._test_unclassified_attribute_error(
            testcase=self,
            serializer=serializer,
            request_data=self.request_data,
            assert_single_invalid_case_fn=self._assert_single_invalid_case,
        )

    def test_unclassified_attributes_error(self):
        serializer_kwargs = {"data": copy.deepcopy(self.request_data), "many": True}
        serializer = self.serializer_class(**serializer_kwargs)
        self._test_unclassified_attributes_error(
            serializer,
            self.request_data,
            testcase=self,
            assert_serializer_validation_error_fn=self._assert_serializer_validation_error,
        )

    def test_incorrect_attribute_value(self):
        serializer_kwargs = {"data": copy.deepcopy(self.request_data), "many": True}
        serializer = self.serializer_class(**serializer_kwargs)
        self._test_incorrect_attribute_value(
            serializer,
            self.request_data,
            testcase=self,
            assert_serializer_validation_error_fn=self._assert_serializer_validation_error,
        )


class DevIngredientBaseUpdateTestCase(DevIngredientCommonMixin, TransactionTestCase):
    def setUp(self):
        super().setUp()
        # create new instances first
        serializer_kwargs_setup = {
            "data": self.request_data,
            "many": True,
        }
        serializer = self.serializer_class(**serializer_kwargs_setup)
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
        self.editing_data = copy.deepcopy(self.request_data[:num_edit_data])
        self.unaffected_data = self.request_data[num_edit_data:]
        self.rand_gen_edit_data(editing_data=self.editing_data)
        editing_ids = tuple(map(lambda x: x["id"], self.editing_data))
        self.edit_objs = self.serializer_class.Meta.model.objects.filter(
            id__in=editing_ids
        )

    ## end of setUp()

    def test_bulk_ok_some_items(self):
        serializer_kwargs = {
            "data": copy.deepcopy(self.editing_data),
            "instance": self.edit_objs,
            "many": True,
        }
        serializer = self.serializer_class(**serializer_kwargs)
        serializer.is_valid(raise_exception=True)
        self._assert_attributes_data_change(
            data_before_validate=self.editing_data,
            data_after_validate=serializer_kwargs["data"],
        )
        edited_objs = serializer.save()
        self.verify_objects(
            actual_instances=edited_objs, expect_data=serializer_kwargs["data"]
        )
        # check unaffected items
        unaffected_ids = tuple(map(lambda x: x["id"], self.unaffected_data))
        unaffected_objs = self.serializer_class.Meta.model.objects.filter(
            id__in=unaffected_ids
        )
        serializer_ro = self.serializer_class(instance=unaffected_objs, many=True)
        expect_unaffected_data = sort_nested_object(self.unaffected_data)
        actual_unaffected_data = sort_nested_object(serializer_ro.data)
        expect_unaffected_data = json.dumps(expect_unaffected_data, sort_keys=True)
        actual_unaffected_data = json.dumps(actual_unaffected_data, sort_keys=True)
        self.assertEqual(expect_unaffected_data, actual_unaffected_data)

    def test_conflict_ingredient_id(self):
        self.assertGreaterEqual(len(self.editing_data), 2)
        discarded_id = self.editing_data[-2]["id"]
        self.editing_data[-2]["id"] = self.editing_data[-1]["id"]
        edit_objs = self.edit_objs.exclude(pk=discarded_id)
        serializer_kwargs = {
            "data": self.editing_data,
            "many": True,
            "instance": edit_objs,
        }
        error_caught = None
        with self.assertRaises(DRFValidationError):
            try:
                serializer = self.serializer_class(**serializer_kwargs)
                serializer.is_valid(raise_exception=True)
            except DRFValidationError as e:
                error_caught = e
                raise
        self.assertNotEqual(error_caught, None)
        non_field_err_key = drf_default_settings["NON_FIELD_ERRORS_KEY"]
        err_detail = error_caught.detail[non_field_err_key][0]
        err_info = json.loads(str(err_detail))
        self.assertEqual(err_detail.code, "conflict")
        self.assertEqual(err_info["message"], "duplicate item found in the list")
        err_ids = [e["id"] for e in err_info["value"]]
        self.assertNotIn(discarded_id, err_ids)
        self.assertEqual(err_info["value"][-2], err_info["value"][-1])


## end of class DevIngredientUpdateTestCase


class DevIngredientUpdateAttributeConflictTestCase(DevIngredientBaseUpdateTestCase):
    min_num_applied_attrs = 0
    max_num_applied_attrs = 1  # disable random-generated attribute values
    num_ingredients_created = 1

    def setUp(self):
        super().setUp()
        editing_ids = tuple(map(lambda x: x["id"], self.request_data))
        self.edit_objs = self.serializer_class.Meta.model.objects.filter(
            id__in=editing_ids
        )
        self.str_attr_type = tuple(
            filter(
                lambda x: x["dtype"] == _ProductAttrValueDataType.STRING.value[0][0],
                _fixtures["ProductAttributeType"],
            )
        )
        self.int_attr_type = tuple(
            filter(
                lambda x: x["dtype"] == _ProductAttrValueDataType.INTEGER.value[0][0],
                _fixtures["ProductAttributeType"],
            )
        )

    ## end of setUp()

    def test_same_attr_id_different_dtypes(self):
        editing_data = self.request_data[0]
        edit_objs = self.edit_objs.filter(pk=editing_data["id"])
        attrs = [
            {"type": self.str_attr_type[0]["id"], "value": "Wood cabin"},
            {"type": self.int_attr_type[0]["id"], "value": -543},
        ]
        editing_data["attributes"].extend(attrs)
        serializer = self.serializer_class(
            data=[editing_data], many=True, instance=edit_objs
        )
        serializer.is_valid(raise_exception=True)
        serializer.save()
        # start testing
        editing_data = copy.deepcopy(serializer.data)[0]
        edit_attr_val = copy.deepcopy(editing_data["attributes"][0])
        editing_data["attributes"][0]["id"] = editing_data["attributes"][1]["id"]
        reset_serializer_validation_result(serializer=serializer)
        serializer.initial_data = [copy.deepcopy(editing_data)]
        serializer.instance = edit_objs
        serializer.is_valid(raise_exception=True)
        serializer.save()
        edited_data = copy.deepcopy(serializer.data)[0]
        # original attrval id should be discarded
        attrval_manager = getattr(
            edit_objs.first(), _ProductAttrValueDataType.STRING.value[0][1]
        )
        discarded_attrval_exists = attrval_manager.filter(
            id=edit_attr_val["id"]
        ).exists()
        self.assertFalse(discarded_attrval_exists)
        # the content in edit_attr_val is deleted and created again with new attrval id
        qset = attrval_manager.filter(attr_type=edit_attr_val["type"])
        actual_new_attrval_id = list(qset.values_list("id", flat=True))
        self.assertGreater(len(actual_new_attrval_id), 0)
        expect_new_attrval_id = filter(
            lambda a: a["type"] == edit_attr_val["type"], edited_data["attributes"]
        )
        expect_new_attrval_id = list(map(lambda a: a["id"], expect_new_attrval_id))
        self.assertGreater(len(expect_new_attrval_id), 0)
        self.assertSetEqual(set(expect_new_attrval_id), set(actual_new_attrval_id))

    def test_same_attr_id_and_dtype(self):
        editing_data = self.request_data[0]
        edit_objs = self.edit_objs.filter(pk=editing_data["id"])
        attrvals = [
            {"type": self.str_attr_type[0]["id"], "value": "knowledge hoarder"},
            {"type": self.str_attr_type[1]["id"], "value": "team player"},
            {"type": self.int_attr_type[0]["id"], "value": -689},
            {"type": self.int_attr_type[1]["id"], "value": -302},
        ]
        editing_data["attributes"].extend(attrvals)
        serializer = self.serializer_class(
            data=[editing_data], many=True, instance=edit_objs
        )
        serializer.is_valid(raise_exception=True)
        serializer.save()
        # start testing
        editing_data = copy.deepcopy(serializer.data)[0]
        editing_data["attributes"][0]["id"] = editing_data["attributes"][1]["id"]
        editing_data["attributes"][2]["id"] = editing_data["attributes"][3]["id"]
        reset_serializer_validation_result(serializer=serializer)
        serializer.initial_data = [copy.deepcopy(editing_data)]
        serializer.instance = edit_objs
        error_caught = None
        with self.assertRaises(DRFValidationError):
            try:  # this case will cause data loss, expect to receive error response
                serializer.is_valid(raise_exception=True)
            except DRFValidationError as e:
                error_caught = e
                raise
        self.assertNotEqual(error_caught, None)
        non_field_err_key = drf_default_settings["NON_FIELD_ERRORS_KEY"]
        non_field_errors = error_caught.detail[0]["attributes"][non_field_err_key]
        self.assertEqual(len(non_field_errors), 2)
        str_attr_errs = tuple(
            filter(lambda e: str(e).find("string attribute") > 0, non_field_errors)
        )
        int_attr_errs = tuple(
            filter(lambda e: str(e).find("integer attribute") > 0, non_field_errors)
        )
        self.assertEqual(len(str_attr_errs), 1)
        self.assertEqual(len(int_attr_errs), 1)
        dup_id = str(editing_data["attributes"][1]["id"])
        self.assertGreater(str(str_attr_errs[0]).find(dup_id), 0)
        dup_id = str(editing_data["attributes"][3]["id"])
        self.assertGreater(str(int_attr_errs[0]).find(dup_id), 0)

    def test_same_attr_id_and_attrtype(self):
        editing_data = self.request_data[0]
        edit_objs = self.edit_objs.filter(pk=editing_data["id"])
        attrvals = [
            {"type": self.str_attr_type[0]["id"], "value": "knowledge hoarder"},
            {"type": self.str_attr_type[1]["id"], "value": "team player"},
            {"type": self.str_attr_type[0]["id"], "value": "critical thinker"},
        ]
        editing_data["attributes"].extend(attrvals)
        serializer = self.serializer_class(
            data=[editing_data], many=True, instance=edit_objs
        )
        # duplicate attribute types are allowed if they have distinct attribute value ID
        serializer.is_valid(raise_exception=True)
        serializer.save()
        editing_data = copy.deepcopy(serializer.data)[0]
        editing_data["attributes"][0]["id"] = editing_data["attributes"][2]["id"]
        reset_serializer_validation_result(serializer=serializer)
        serializer.initial_data = [copy.deepcopy(editing_data)]
        serializer.instance = edit_objs
        error_caught = None
        with self.assertRaises(DRFValidationError):
            try:
                serializer.is_valid(raise_exception=True)
            except DRFValidationError as e:
                error_caught = e
                raise
        self.assertNotEqual(error_caught, None)
        non_field_err_key = drf_default_settings["NON_FIELD_ERRORS_KEY"]
        non_field_errors = error_caught.detail[0]["attributes"][non_field_err_key]
        str_attr_errs = tuple(
            filter(lambda e: str(e).find("string attribute") > 0, non_field_errors)
        )
        self.assertEqual(len(str_attr_errs), 1)
        dup_id = str(editing_data["attributes"][1]["id"])
        self.assertGreater(str(str_attr_errs[0]).find(dup_id), 0)


## end of class DevIngredientUpdateAttributeConflictTestCase


class DevIngredientRepresentationTestCase(DevIngredientBaseUpdateTestCase):
    def test_represent_all(self):
        created_ids = tuple(map(lambda x: x.id, self._created_items))
        created_objs = self.serializer_class.Meta.model.objects.filter(
            id__in=created_ids
        )
        serializer_ro = self.serializer_class(instance=created_objs, many=True)
        actual_data = serializer_ro.data
        expect_data = self.request_data
        self.verify_objects(
            actual_data, expect_data, extra_check_fn=self._assert_attributes_field
        )

    def test_represent_partial(self):
        expect_fields = ["id", "category", "attributes"]
        mocked_request = Mock()
        mocked_request.query_params = {"fields": ",".join(expect_fields)}
        created_ids = tuple(map(lambda x: x.id, self._created_items))
        created_objs = self.serializer_class.Meta.model.objects.filter(
            id__in=created_ids
        )
        serializer_ro = self.serializer_class(instance=created_objs, many=True)
        serializer_ro.context["request"] = mocked_request
        actual_data = serializer_ro.data
        expect_data = self.request_data
        self.verify_objects(
            actual_data,
            expect_data,
            extra_check_fn=self._assert_attributes_field,
            non_nested_fields=["id", "category"],
        )

    def _assert_attributes_field(self, exp_item, ac_item):
        # compare attribute values in `attributes` field
        exp_attrs = _get_inst_attr(exp_item, "attributes", [])
        ac_attrs = _get_inst_attr(ac_item, "attributes", [])
        exp_attrs = sort_nested_object(obj=exp_attrs)
        ac_attrs = sort_nested_object(obj=ac_attrs)
        exp_attrs = json.dumps(exp_attrs, sort_keys=True)
        ac_attrs = json.dumps(ac_attrs, sort_keys=True)
        self.assertEqual(exp_attrs, ac_attrs)


## end of class DevIngredientRepresentationTestCase
