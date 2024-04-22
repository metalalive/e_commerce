import random

from django.test import TransactionTestCase
from django.contrib.contenttypes.models import ContentType

from product.models.base import (
    ProductAttributeType,
    ProductSaleableItem,
    ProductSaleablePackage,
)
from product.models.development import ProductDevIngredient
from ..common import _fixtures, _common_instances_setup


class AttrTypeDeletionTestCase(TransactionTestCase):

    def setUp(self):
        num_attrtypes = 6
        num_ingredients = len(_fixtures[ProductDevIngredient.__name__]) >> 1
        num_saleitems = len(_fixtures[ProductSaleableItem.__name__])
        num_salepkgs = len(_fixtures[ProductSaleablePackage.__name__])
        models_info = [
            (ProductAttributeType, num_attrtypes),
            (ProductDevIngredient, num_ingredients),
            (ProductSaleableItem, num_saleitems),
            (ProductSaleablePackage, num_salepkgs),
        ]
        primitives = {}
        _common_instances_setup(out=primitives, models_info=models_info)
        for attrtype in primitives["ProductAttributeType"]:
            manager = attrtype.attr_val_set
            for cls in (
                ProductDevIngredient,
                ProductSaleableItem,
                ProductSaleablePackage,
            ):
                content_type = ContentType.objects.get_for_model(cls)
                objs = primitives[cls.__name__]
                for obj in objs:
                    attr_value = random.choice(_fixtures[manager.model.__name__])
                    _data = {
                        "ingredient_type": content_type,
                        "ingredient_id": obj.id,
                        "value": attr_value,
                    }
                    manager.create(**_data)
        self._primitives = primitives
        expect_deleted_attrtypes = map(
            lambda _: random.choice(primitives["ProductAttributeType"]), range(3)
        )
        chosen_fields = ["id", "ingredient_id", "ingredient_type", "value"]
        self._expect_deleted_attrvals = dict(
            map(
                lambda obj: (
                    (obj.id, obj.attr_val_set),
                    list(obj.attr_val_set.order_by("id").values(*chosen_fields)),
                ),
                expect_deleted_attrtypes,
            )
        )

    def test_hard_delete(self):
        delete_ids = tuple(map(lambda d: d[0], self._expect_deleted_attrvals.keys()))
        qset = ProductAttributeType.objects.filter(id__in=delete_ids)
        qset.delete(hard=True)
        qset = ProductAttributeType.objects.get_deleted_set().filter(id__in=delete_ids)
        self.assertFalse(qset.exists())
        for key, value in self._expect_deleted_attrvals.items():
            attr_val_model = key[1].model
            attr_val_ids = list(map(lambda d: d["id"], value))
            qset = attr_val_model.objects.get_deleted_set().filter(id__in=attr_val_ids)
            self.assertFalse(qset.exists())
            attrval_related_field = attr_val_model.DATATYPE.value[0][1]
            for cls in (
                ProductDevIngredient,
                ProductSaleableItem,
                ProductSaleablePackage,
            ):
                for ingredient in self._primitives[cls.__name__]:
                    manager = getattr(ingredient, attrval_related_field)
                    qset = manager.get_deleted_set().filter(id__in=attr_val_ids)
                    self.assertFalse(qset.exists())

    def test_soft_delete(self):
        profile_id = 234
        delete_ids = set(map(lambda d: d[0], self._expect_deleted_attrvals.keys()))
        qset = ProductAttributeType.objects.filter(id__in=delete_ids)
        qset.delete(profile_id=profile_id)
        qset = ProductAttributeType.objects.filter(id__in=delete_ids)
        self.assertFalse(qset.exists())
        qset = (
            ProductAttributeType.objects.get_deleted_set()
            .filter(id__in=delete_ids)
            .values_list("id", flat=True)
        )
        self.assertSetEqual(set(qset), delete_ids)
        for key, value in self._expect_deleted_attrvals.items():
            attr_val_mgr = key[1]
            attr_val_ids = list(map(lambda d: d["id"], value))
            qset = attr_val_mgr.filter(id__in=attr_val_ids)
            self.assertFalse(qset.exists())
            qset = attr_val_mgr.get_deleted_set().filter(id__in=attr_val_ids)
            self.assertTrue(qset.exists())
            chosen_fields = ["id", "ingredient_id", "ingredient_type", "value"]
            expect_value = value
            actual_value = list(qset.order_by("id").values(*chosen_fields))
            self.assertListEqual(expect_value, actual_value)
            attrval_related_field = attr_val_mgr.model.DATATYPE.value[0][1]
            for cls in (
                ProductDevIngredient,
                ProductSaleableItem,
                ProductSaleablePackage,
            ):
                for ingredient in self._primitives[cls.__name__]:
                    manager = getattr(ingredient, attrval_related_field)
                    qset = manager.filter(id__in=attr_val_ids)
                    self.assertFalse(qset.exists())
                    qset = manager.get_deleted_set().filter(id__in=attr_val_ids)
                    self.assertTrue(qset.exists())
