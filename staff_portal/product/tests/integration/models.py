import random
from functools import partial

from django.test import TransactionTestCase, TestCase
from django.db import DEFAULT_DB_ALIAS
from django.db.models import Q
from django.db.utils import IntegrityError

from common.models.enums   import UnitOfMeasurement
from product.models.base import ProductTag, ProductTagClosure, ProductAttributeType, ProductSaleableItem, ProductSaleableItemComposite
from product.models.development import ProductDevIngredientType, ProductDevIngredient

_fixtures = {
    'ProductDevIngredient': [
        {'category':ProductDevIngredientType.RAW_MATERIAL    , 'name':'tomato'},
        {'category':ProductDevIngredientType.RAW_MATERIAL    , 'name':'all-purpose flour'},
        {'category':ProductDevIngredientType.RAW_MATERIAL    , 'name':'bread flour'},
        {'category':ProductDevIngredientType.RAW_MATERIAL    , 'name':'quail egg'},
        {'category':ProductDevIngredientType.RAW_MATERIAL    , 'name':'dry yeast powder'},
        {'category':ProductDevIngredientType.WORK_IN_PROGRESS, 'name':'poolish'},
        {'category':ProductDevIngredientType.WORK_IN_PROGRESS, 'name':'tomato puree'},
        {'category':ProductDevIngredientType.WORK_IN_PROGRESS, 'name':'LiPo Battery'},
        {'category':ProductDevIngredientType.CONSUMABLES     , 'name':'bio gas'},
        {'category':ProductDevIngredientType.EQUIPMENTS      , 'name':'oven'},
        {'category':ProductDevIngredientType.EQUIPMENTS      , 'name':'RISC-V development board'},
        {'category':ProductDevIngredientType.EQUIPMENTS  , 'name':'Raspberry PI'},
        {'category':ProductDevIngredientType.EQUIPMENTS  , 'name':'GPS sensor'},
        {'category':ProductDevIngredientType.EQUIPMENTS  , 'name':'Pixhawk flight controller'},
    ],
    'ProductSaleableItem': [
        {'name':'fruit fertilizer',    'price':3.88,  'usrprof':19},
        {'name':'rough rice noddle',   'price':0.18,  'usrprof':22},
        {'name':'RISC-V programming course', 'price':11.30, 'usrprof':28},
        {'name':'Mozzarella pizza', 'price':13.93, 'usrprof':79},
        {'name':'Pita dough', 'price':3.08, 'usrprof':79},
        {'name':'quad drone', 'price':17.02, 'usrprof':12},
    ],
}

_load_init_params = lambda init_params, model_cls: model_cls(**init_params)
num_uom = len(UnitOfMeasurement.choices)


def _create_one_essential_attrs_incomplete(testcase, instance, field_names):
    for fname in field_names:
        old_value = getattr(instance, fname)
        setattr(instance, fname, None)
        with testcase.assertRaises(IntegrityError) as e:
            instance.save(force_insert=True)
        setattr(instance, fname, old_value)


class SimpleSaleableItemCreationTestCase(TransactionTestCase):
    databases = {DEFAULT_DB_ALIAS}

    def __init__(self, *args, **kwargs):
        super().__init__(*args, **kwargs)
        self._created_ids = []

    def setUp(self):
        bound_fn = partial(_load_init_params, model_cls=ProductSaleableItem)
        params = _fixtures[ProductSaleableItem.__name__]
        self.instances = list(map(bound_fn, params))

    def tearDown(self):
        fn = lambda obj: obj.pk
        created_objs = tuple(filter(fn, self.instances))
        created_ids = tuple(map(fn, created_objs))
        if any(created_ids):
            removing_qset = ProductSaleableItem.objects.filter(pk__in=created_ids)
            removing_qset.delete(hard=True)
            self.instances.clear()

    def test_create_one_essential_attrs_incomplete(self):
        field_names = ['name', 'usrprof', 'visible', 'price']
        instance = self.instances[0]
        _create_one_essential_attrs_incomplete(testcase=self,
                instance=instance, field_names=field_names)

    def test_create_onebyone_ok(self):
        self.assertEqual(self.instances[0].id, None)
        self.instances[0].save(force_insert=True)
        self.assertNotEqual(self.instances[0].id, None)
        self.assertGreater(self.instances[0].id , 0)

    def test_create_onebyone_duplicate_id(self):
        self.instances[0].save(force_insert=True)
        self.instances[1].id = self.instances[0].id
        self.instances[1].save(force_insert=True)
        self.check_instances_id()

    def test_bulk_create_ok(self):
        ProductSaleableItem.objects.bulk_create(self.instances[:2])
        self.check_instances_id()

    def check_instances_id(self):
        self.assertNotEqual(self.instances[0].id, None)
        self.assertNotEqual(self.instances[1].id, None)
        self.assertGreater(self.instances[0].id, 0)
        self.assertGreater(self.instances[1].id, 0)
        self.assertNotEqual(self.instances[0].id, self.instances[1].id)

    def test_bulk_create_duplicate_id_1(self):
        dup_id = 1234
        self.instances[0].id = dup_id
        self.instances[2].id = dup_id
        self.instances[3].id = 1235
        with self.assertRaises(ValueError):
            ProductSaleableItem.objects.bulk_create(self.instances)

    def test_bulk_create_duplicate_id_2(self):
        dup_ids = [1234, 5678]
        self.instances[0].id = dup_ids[0]
        self.instances[1].id = dup_ids[1]
        ProductSaleableItem.objects.bulk_create(self.instances[:2])
        self.instances[2].id = dup_ids[0]
        self.instances[3].id = dup_ids[1]
        ProductSaleableItem.objects.bulk_create(self.instances[2:])
        ids = tuple(map(lambda instance: getattr(instance, 'id'), self.instances))
        ids = tuple(filter(lambda x: x is not None and x > 0, ids))
        expected = len(ids)
        actual   = len(set(ids)) # test whether all ID numbers are distinct to each other
        self.assertGreater(actual, 0)
        self.assertEqual(expected, actual)
## end of class SimpleSaleableItemCreationTestCase


class SimpleSaleableItemDeletionTestCase(TransactionTestCase):
    # the test class covers both soft-delete and hard-delete
    def setUp(self):
        bound_fn = partial(_load_init_params, model_cls=ProductSaleableItem)
        params = _fixtures[ProductSaleableItem.__name__]
        self.instances = list(map(bound_fn, params))
        ProductSaleableItem.objects.bulk_create(self.instances)

    def tearDown(self):
        pass

    def test_hard_delete_single_item_ok(self):
        chosen_id = self.instances[0].id
        exist = ProductSaleableItem.objects.filter(pk=chosen_id).exists()
        self.assertEqual(exist, True)
        self.instances[0].delete(hard=True)
        exist = ProductSaleableItem.objects.filter(with_deleted=True, pk=chosen_id).exists()
        self.assertEqual(exist, False)

    def test_soft_delete_single_item_ok(self):
        chosen_id = self.instances[0].id
        exist = ProductSaleableItem.objects.filter(pk=chosen_id).exists()
        self.assertEqual(exist, True)
        # report error if not specifying `profile_id`  when soft-deleting an instance
        with self.assertRaises(IntegrityError) as e:
            # expect to receive erro because caller does NOT provide `profile_id` argument
            self.instances[0].delete()
        self.instances[0].delete(profile_id=self.instances[0].usrprof)
        exist = ProductSaleableItem.objects.filter(id=chosen_id).exists()
        self.assertEqual(exist, False)
        exist = ProductSaleableItem.objects.filter(pk=chosen_id).exists()
        self.assertEqual(exist, True)
        exist = ProductSaleableItem.objects.filter(with_deleted=True, id=chosen_id).exists()
        self.assertEqual(exist, True)

    def _choose_ids_to_delete(self):
        total_num_stored = len(self.instances)
        num_deleting = int(total_num_stored / 2)
        num_remained = total_num_stored - num_deleting
        chosen_ids = list(map(lambda obj: obj.id, self.instances[:num_deleting]))
        return total_num_stored, num_deleting, num_remained, chosen_ids

    def test_hard_delete_bulk_ok(self):
        total_num_stored, num_deleting, num_remained, chosen_ids =  self._choose_ids_to_delete()
        qset = ProductSaleableItem.objects.filter(id__in=chosen_ids)
        self.assertEqual(num_deleting, qset.count())
        qset.delete(hard=True)
        qset = ProductSaleableItem.objects.filter(with_deleted=True, id__in=chosen_ids)
        self.assertEqual(qset.exists(), False)
        self.assertEqual(ProductSaleableItem.objects.count(), num_remained)

    def test_soft_delete_bulk_ok(self):
        total_num_stored, num_deleting, num_remained, chosen_ids =  self._choose_ids_to_delete()
        qset = ProductSaleableItem.objects.filter(id__in=chosen_ids)
        qset.delete(profile_id=self.instances[0].usrprof)
        exist = ProductSaleableItem.objects.filter(id__in=chosen_ids).exists()
        self.assertEqual(exist, False)
        cnt  = ProductSaleableItem.objects.filter(pk__in=chosen_ids).count()
        self.assertEqual(cnt, 0)
        cnt  = ProductSaleableItem.objects.filter(with_deleted=True, id__in=chosen_ids).count()
        self.assertEqual(cnt, num_deleting)
## end of class SimpleSaleableItemDeletionTestCase


