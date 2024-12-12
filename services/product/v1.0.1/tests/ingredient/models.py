import random
from functools import partial

from django.test import TransactionTestCase
from django.db.utils import DataError

from ecommerce_common.util import sort_nested_object
from product.models.base import (
    ProductAttributeType,
    _ProductAttrValueDataType,
    ProductSaleableItem,
    UnitOfMeasurement,
)
from product.models.development import ProductDevIngredient

from tests.common import (
    _fixtures,
    listitem_rand_assigner,
    _common_instances_setup,
    _null_test_obj_attrs,
    _ingredient_attrvals_common_setup,
)


num_uom = len(UnitOfMeasurement.choices)


def _validate_attr_vals(
    dtype_opt, testcase, ingredient, fixtures, expect_attrvals, is_deleted=False
):
    manager = getattr(ingredient, dtype_opt.value[0][1])
    qset = manager.all(with_deleted=is_deleted)
    for attrval in qset:
        expect = type(ingredient)
        actual = attrval.ingredient_type.model_class()
        testcase.assertEqual(expect, actual)
        expect = ingredient.id
        actual = attrval.ingredient_id
        testcase.assertEqual(expect, actual)
        actual = attrval.attr_type
        expect = tuple(filter(lambda d: d["id"] == actual.id, fixtures))
        testcase.assertGreater(len(expect), 0)
    dtype_value = dtype_opt.value[0][0]
    ingre_id = ingredient.pk
    attrval_set = expect_attrvals[dtype_value].get(ingre_id, [])
    expect = list(
        map(
            lambda obj: {"attr_type": obj.attr_type.pk, "value": obj.value}, attrval_set
        )
    )
    actual = list(qset.values("attr_type", "value"))
    expect = sort_nested_object(obj=expect)
    actual = sort_nested_object(obj=actual)
    testcase.assertListEqual(expect, actual)


class IngredientCreationTestCase(TransactionTestCase):
    instances = {
        "ProductAttributeType": None,
    }
    num_attrtypes = len(_fixtures["ProductAttributeType"])

    def setUp(self):
        models_info = [
            (ProductAttributeType, self.num_attrtypes),
        ]
        _common_instances_setup(out=self.instances, models_info=models_info)

    def test_null_obj_fields(self):
        field_names = [
            "name",
            "category",
        ]
        instance = ProductDevIngredient(**_fixtures["ProductDevIngredient"][0])
        _null_test_obj_attrs(testcase=self, instance=instance, field_names=field_names)
        with self.assertRaises(DataError):
            instance.category = -999
            instance.save(force_insert=True)

    def test_single_item_min_content(self):
        instance = ProductDevIngredient(**_fixtures["ProductDevIngredient"][0])
        instance.save(force_insert=True)
        expect = instance
        qset = ProductDevIngredient.objects.filter(pk=expect.pk)
        self.assertEqual(qset.count(), 1)
        actual = qset.first()
        self.assertEqual(expect.pk, actual.pk)

    def test_single_item_with_attrs(self):
        instance = ProductDevIngredient(**_fixtures["ProductDevIngredient"][0])
        instance.save(force_insert=True)

        def attrtypes_gen_fn():
            return self.instances["ProductAttributeType"]

        attrval_objs = _ingredient_attrvals_common_setup(
            ingredients=[instance],
            attrtypes_gen_fn=attrtypes_gen_fn,
        )
        instance.refresh_from_db()
        bound_fn = partial(
            _validate_attr_vals,
            testcase=self,
            ingredient=instance,
            fixtures=_fixtures["ProductAttributeType"],
            expect_attrvals=attrval_objs,
        )
        tuple(map(bound_fn, _ProductAttrValueDataType))

    def test_bulk_with_attrs(self):
        instances = {"ProductDevIngredient": None}
        models_info = [
            (ProductDevIngredient, 3),
        ]
        _common_instances_setup(out=instances, models_info=models_info)

        def attrtypes_gen_fn():
            return self.instances["ProductAttributeType"]

        attrval_objs = _ingredient_attrvals_common_setup(
            attrtypes_gen_fn=attrtypes_gen_fn,
            ingredients=instances["ProductDevIngredient"],
        )
        for instance in instances["ProductDevIngredient"]:
            bound_fn = partial(
                _validate_attr_vals,
                testcase=self,
                ingredient=instance,
                fixtures=_fixtures["ProductAttributeType"],
                expect_attrvals=attrval_objs,
            )
            tuple(map(bound_fn, _ProductAttrValueDataType))


## end of class IngredientCreationTestCase


class IngredientDeletionTestCase(TransactionTestCase):
    instances = {
        "ProductAttributeType": None,
        "ProductDevIngredient": None,
        "ProductSaleableItem": None,
        "ProductSaleableItemComposite": [],
    }
    num_attrtypes = len(_fixtures["ProductAttributeType"])
    num_ingredients = 6
    num_saleitems = 2

    def setUp(self):
        models_info = [
            (ProductAttributeType, self.num_attrtypes),
            (ProductDevIngredient, self.num_ingredients),
            (ProductSaleableItem, self.num_saleitems),
        ]
        _common_instances_setup(out=self.instances, models_info=models_info)
        attrtypes_gen_fn = partial(
            listitem_rand_assigner, list_=self.instances["ProductAttributeType"]
        )
        self.attrval_objs = _ingredient_attrvals_common_setup(
            attrtypes_gen_fn=attrtypes_gen_fn,
            ingredients=self.instances["ProductDevIngredient"],
        )
        # create sale items which require some ingredients
        for saleitem in self.instances["ProductSaleableItem"]:
            for ingredient in self.instances["ProductDevIngredient"]:
                compo_attrs = {
                    "unit": UnitOfMeasurement.choices[random.randrange(num_uom)][0],
                    "quantity": random.randrange(1, 120),
                    "ingredient": ingredient,
                    "sale_item": saleitem,
                }
                composite = saleitem.ingredients_applied.create(**compo_attrs)
                self.instances["ProductSaleableItemComposite"].append(composite)

    def tearDown(self):
        for instances in self.instances.values():
            instances.clear()

    def _pre_bulk_delete(self):
        num_ingredients_deleting = self.num_ingredients >> 1
        num_remained = self.num_ingredients - num_ingredients_deleting
        self.assertTrue(num_remained > 0)
        delete_pks = list(
            map(
                lambda obj: obj.pk,
                self.instances["ProductDevIngredient"][:num_ingredients_deleting],
            )
        )
        remain_pks = list(
            map(
                lambda obj: obj.pk,
                self.instances["ProductDevIngredient"][num_ingredients_deleting:],
            )
        )
        qset = ProductDevIngredient.objects.filter(pk__in=delete_pks)
        return delete_pks, remain_pks, qset

    def _post_bulk_delete(self, remain_pks):
        qset = ProductDevIngredient.objects.filter(pk__in=remain_pks)
        self.assertSetEqual(set(qset.values_list("id", flat=True)), set(remain_pks))
        for ingredient in qset:
            bound_fn = partial(
                _validate_attr_vals,
                testcase=self,
                ingredient=ingredient,
                fixtures=_fixtures["ProductAttributeType"],
                expect_attrvals=self.attrval_objs,
            )
            tuple(map(bound_fn, _ProductAttrValueDataType))
        self._check_remaining_ingredients_in_saleitem(ingre_ids=remain_pks)

    def _check_remaining_ingredients_in_saleitem(self, ingre_ids, is_deleted=False):
        chosen_composites = tuple(
            filter(
                lambda obj: obj.ingredient.id in ingre_ids,
                self.instances["ProductSaleableItemComposite"],
            )
        )
        # check saleable items which include the deleted ingredient
        for saleitem in self.instances["ProductSaleableItem"]:
            init_qset = (
                saleitem.ingredients_applied.get_deleted_set()
                if is_deleted
                else saleitem.ingredients_applied.all()
            )
            actual = list(
                init_qset.filter(
                    sale_item=saleitem.id, ingredient__in=ingre_ids
                ).values("ingredient", "unit", "quantity")
            )
            remain_composites = filter(
                lambda obj: obj.sale_item.id == saleitem.id, chosen_composites
            )
            expect = list(
                map(
                    lambda obj: {
                        "ingredient": obj.ingredient.id,
                        "unit": obj.unit,
                        "quantity": float(obj.quantity),
                    },
                    remain_composites,
                )
            )
            expect = sort_nested_object(obj=expect)
            actual = sort_nested_object(obj=actual)
            # if expect != actual:
            #    import pdb
            #    pdb.set_trace()
            self.assertListEqual(expect, actual)

    def test_hard_delete_bulk(self):
        delete_pks, remain_pks, qset = self._pre_bulk_delete()
        qset.delete(hard=True)
        deleted_set = ProductDevIngredient.objects.get_deleted_set()
        self.assertFalse(deleted_set.exists())
        qset = ProductDevIngredient.objects.filter(with_deleted=True, pk__in=delete_pks)
        self.assertFalse(deleted_set.exists())
        self._post_bulk_delete(remain_pks)

    ## end of test_hard_delete_bulk()

    def test_soft_delete_bulk(self):
        profile_id = 1234
        delete_pks, remain_pks, qset = self._pre_bulk_delete()
        qset.delete(profile_id=profile_id)
        deleted_set = ProductDevIngredient.objects.get_deleted_set()
        self.assertTrue(deleted_set.exists())
        self.assertEqual(deleted_set.count(), len(delete_pks))
        self._post_bulk_delete(remain_pks)
        self._check_remaining_ingredients_in_saleitem(
            ingre_ids=delete_pks, is_deleted=True
        )
        for deleted_ingredient in deleted_set:
            bound_fn = partial(
                _validate_attr_vals,
                testcase=self,
                ingredient=deleted_ingredient,
                fixtures=_fixtures["ProductAttributeType"],
                expect_attrvals=self.attrval_objs,
                is_deleted=True,
            )
            tuple(map(bound_fn, _ProductAttrValueDataType))

        deleted_set.undelete(profile_id=profile_id)
        deleted_set = ProductDevIngredient.objects.get_deleted_set()
        self.assertFalse(deleted_set.exists())
        self._post_bulk_delete(remain_pks=delete_pks)


## end of class IngredientDeletionTestCase
