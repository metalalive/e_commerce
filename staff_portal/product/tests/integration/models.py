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


class SaleableItemCompositeCreationTestCase(TransactionTestCase):
    def setUp(self):
        self.instances = {'ProductDevIngredient': None, 'ProductSaleableItem': None}
        model_classes = [ProductDevIngredient, ProductSaleableItem]
        for model_cls in model_classes:
            bound_fn = partial(_load_init_params, model_cls=model_cls)
            model_name = model_cls.__name__
            params = _fixtures[model_name]
            self.instances[model_name] = list(map(bound_fn, params))
            model_cls.objects.bulk_create(self.instances[model_name])
        self.instances['ProductDevIngredient'] = list(ProductDevIngredient.objects.all())

    def tearDown(self):
        pass

    def test_create_one_essential_attrs_incomplete(self):
        instance = ProductSaleableItemComposite(
                sale_item  = self.instances['ProductSaleableItem'][0],
                ingredient = self.instances['ProductDevIngredient'][0],
                quantity   = 2,
                unit = UnitOfMeasurement.UNIT.value,
            )
        field_names = ['quantity', 'unit', 'sale_item', 'ingredient']
        _create_one_essential_attrs_incomplete(testcase=self,
                instance=instance, field_names=field_names)

    def test_create_one_ok(self):
        instance = ProductSaleableItemComposite(
                sale_item  = self.instances['ProductSaleableItem'][0],
                ingredient = self.instances['ProductDevIngredient'][0],
                unit = UnitOfMeasurement.UNIT.value,
                quantity   = 29,
            )
        instance.save(force_insert=True)
        # below is query test with different conditions
        pk_cond = {'ingredient': instance.ingredient, 'sale_item': instance.sale_item}
        expected_compo_pk = {'ingredient': instance.ingredient.pk, 'sale_item': instance.sale_item.pk}
        obj = ProductSaleableItemComposite.objects.get(pk=pk_cond)
        self.assertNotEqual(obj, None)
        self.assertDictEqual(obj.pk, expected_compo_pk)

        qset = ProductSaleableItemComposite.objects.filter(pk=pk_cond)
        self.assertEqual(qset.count(), 1)
        self.assertDictEqual(qset[0].pk, expected_compo_pk)

        obj = ProductSaleableItemComposite.objects.last()
        self.assertNotEqual(obj, None)
        self.assertDictEqual(obj.pk, expected_compo_pk)

        obj = ProductSaleableItemComposite.objects.first()
        self.assertNotEqual(obj, None)
        self.assertDictEqual(obj.pk, expected_compo_pk)
        # pk__isnull , the lookup option will be ignored cuz I haven't figured out how to implement this
        cond = Q(pk__isnull={
                'sale_item__gte': instance.sale_item.pk - 3,
                'ingredient__in': list(range(instance.ingredient.pk - 5 , instance.ingredient.pk + 5)),
                'sale_item__lt': instance.sale_item.pk + 3}
            )
        qset = ProductSaleableItemComposite.objects.filter(cond)
        self.assertEqual(qset.count(), 1)
        self.assertDictEqual(qset[0].pk, expected_compo_pk)


    def test_bulk_create_ok(self):
        num_compo = 6
        num_units = 3
        saleitems_composite = []
        for idx in range(num_compo):
            instance = ProductSaleableItemComposite(
                    sale_item  = self.instances['ProductSaleableItem'][idx % 2],
                    ingredient = self.instances['ProductDevIngredient'][idx],
                    unit = UnitOfMeasurement.KILOGRAM.value + (idx % num_units),
                    quantity   = 30 + idx,
                )
            saleitems_composite.append(instance)
        sale_item_ids  = list(map(lambda obj:obj.sale_item.pk,  saleitems_composite))
        ingredient_ids = list(map(lambda obj:obj.ingredient.pk, saleitems_composite))

        ProductSaleableItemComposite.objects.bulk_create(saleitems_composite)
        cond = Q(pk__isnull={'sale_item__in': sale_item_ids,  'ingredient__in': ingredient_ids})
        qset = ProductSaleableItemComposite.objects.filter(cond)
        self.assertEqual(qset.count(), num_compo)

        cond = cond & Q(unit=UnitOfMeasurement.KILOGRAM)
        qset = ProductSaleableItemComposite.objects.filter(cond)
        self.assertEqual(qset.count(), int(num_compo / num_units))


    def test_edit_one_ok(self):
        instance = ProductSaleableItemComposite(
                sale_item  = self.instances['ProductSaleableItem'][-1],
                ingredient = self.instances['ProductDevIngredient'][-1],
                unit = UnitOfMeasurement.OZ_OUNCE.value,
                quantity   = 47,
            )
        instance.save(force_insert=True)
        instance.refresh_from_db()
        self.assertEqual(instance.unit , UnitOfMeasurement.OZ_OUNCE.value)

        instance.ingredient = self.instances['ProductDevIngredient'][-2]
        instance.unit = UnitOfMeasurement.UK_TON.value
        instance.save(force_update=True, update_fields=['ingredient', 'unit'])
        instance.refresh_from_db()
        self.assertEqual(instance.unit , UnitOfMeasurement.UK_TON.value)
        self.assertEqual(instance.ingredient.pk , self.instances['ProductDevIngredient'][-2].pk)
        num_stored = ProductSaleableItemComposite.objects.count()
        self.assertEqual(num_stored , 1)

        instance.sale_item = self.instances['ProductSaleableItem'][-3]
        instance.save(update_fields=['sale_item'])
        instance.refresh_from_db()
        self.assertEqual(instance.sale_item.pk , self.instances['ProductSaleableItem'][-3].pk)
        num_stored = ProductSaleableItemComposite.objects.count()
        self.assertEqual(num_stored , 1)


    def test_bulk_edit_ok(self):
        num_units = 3
        num_saleitems   = len(self.instances['ProductSaleableItem'])
        num_ingredients = len(self.instances['ProductDevIngredient'])
        num_compo = num_ingredients
        saleitems_composite = []
        for idx in range(num_compo):
            instance = ProductSaleableItemComposite(
                    sale_item  = self.instances['ProductSaleableItem'][idx % num_saleitems],
                    ingredient = self.instances['ProductDevIngredient'][idx % num_ingredients],
                    unit = UnitOfMeasurement.KILOGRAM.value + (idx % num_units),
                    quantity   = random.randrange(1,50)
                )
            saleitems_composite.append(instance)
        ProductSaleableItemComposite.objects.bulk_create(saleitems_composite)

        num_compo = int(num_compo >> 1)
        for idx in range(1, num_compo):
            instance = saleitems_composite[idx]
            instance.sale_item  = self.instances['ProductSaleableItem'][(idx + 2) % num_saleitems]
            instance.ingredient = self.instances['ProductDevIngredient'][(idx + 3) % num_ingredients]
            # don't add comma at the end of arithmatic expression, python interpreter will
            # bite you hard to translate the whole expression into tuple
            instance.quantity   = random.randrange(instance.quantity , instance.quantity + 100)
        # When MySQL database backend is configured, Django SQL compiler attempts to apply multiple
        # CASE-WHEN-THEN clauses on each column in UPDATE statement.
        with self.assertRaises(IntegrityError):
            # error happens cuz pk column (sale_item) is placed prior to non-pk column
            # (quantity) in the `field` argument
            ProductSaleableItemComposite.objects.bulk_update(
                    saleitems_composite[1:num_compo] ,
                    fields=['sale_item','quantity']
                )
        # CASE-WHEN-THEN in MySQL database doesn't work with table indexed by multi-column
        # primary key (composite pkey). If you want to update several columns that are part of
        # the composite pkey in QuerySet.bulk_update(), you will get error because MySQL
        # processes CASE-WHEN-THEN clauses and update the status for each row one column after another
        with self.assertRaises(IntegrityError):
            ProductSaleableItemComposite.objects.bulk_update(
                    saleitems_composite[1:num_compo] ,
                    fields=['sale_item','ingredient']
                )
        for idx in range(1, num_compo) : # recover ingredient reference
            instance = saleitems_composite[idx]
            instance.ingredient = self.instances['ProductDevIngredient'][idx % num_ingredients]
        # good practice for mysql database on update statement : always insert non-pk columns
        # in the beginning part of the list to `fields` argument, append pk column(s) at the
        # end of the list .
        ProductSaleableItemComposite.objects.bulk_update(
                saleitems_composite[1:num_compo],
                fields=['quantity', 'sale_item']
            )
        for idx in range(1, num_compo):
            instance = saleitems_composite[idx]
            qset = ProductSaleableItemComposite.objects.filter(id=instance.pk)
            self.assertEqual(qset.count() , 1)
            obj_from_db = qset.first()
            self.assertDictEqual(obj_from_db.pk , instance.pk)
            self.assertEqual(obj_from_db.quantity , instance.quantity)
    ## end of test_bulk_edit_ok()

    def test_complex_query_with_composite_pk(self):
        cond = (Q(quantity__gt=2.7182) | Q(pk__isnull={'sale_item__gte': 8, 'ingredient__in': [2,13, 809], 'sale_item__lt': 29})) & Q(unit__lte=30)
        qset = ProductSaleableItemComposite.objects.filter(cond)
        self.assertEqual(qset.count() , 0)
## end of class SaleableItemCompositeCreationTestCase


class SaleableItemCompositeDeletionTestCase(TransactionTestCase):
    # the test class covers both soft-delete and hard-delete
    num_saleitems   = 2
    num_ingredients = len(_fixtures['ProductDevIngredient'])
    num_composites  = int(num_ingredients)

    def setUp(self):
        self.instances = {'ProductDevIngredient': None, 'ProductSaleableItem': None,
                'ProductSaleableItemComposite': []}
        model_classes = [(ProductDevIngredient, self.num_ingredients), (ProductSaleableItem, self.num_saleitems)]
        for model_cls, num_objs_needed in model_classes:
            bound_fn = partial(_load_init_params, model_cls=model_cls)
            model_cls_name = model_cls.__name__
            params = _fixtures[model_cls_name][:num_objs_needed]
            new_instances = list(map(bound_fn, params))
            model_cls.objects.bulk_create(new_instances)
            self.instances[model_cls_name] = list(model_cls.objects.all())

        saleitem_objs   = self.instances['ProductSaleableItem']
        ingredient_objs = self.instances['ProductDevIngredient']

        for idx in range(self.num_composites):
            composite = ProductSaleableItemComposite()
            composite.unit = UnitOfMeasurement.choices[random.randrange(num_uom)][0]
            composite.quantity = random.randrange(6,120)
            chosen_saleitem_idx   = idx % self.num_saleitems
            chosen_ingredient_idx = idx % self.num_ingredients
            composite.sale_item  = saleitem_objs[chosen_saleitem_idx]
            composite.ingredient = ingredient_objs[chosen_ingredient_idx]
            composite.save(force_insert=True)
            self.instances['ProductSaleableItemComposite'].append(composite)

    def tearDown(self):
        pass

    def _rand_choose_saleitem_instance(self):
        saleitem_objs   = self.instances['ProductSaleableItem']
        return saleitem_objs[random.randrange(self.num_saleitems)]

    def test_hard_delete_single_item_ok(self):
        saleitem = self._rand_choose_saleitem_instance()
        while True:
            applied_ingredients = saleitem.ingredients_applied.order_by('pk')
            cnt_before_delete = applied_ingredients.count()
            composite = applied_ingredients.first()
            deleted_pk = composite.pk
            composite.delete(hard=True)
            # refresh database query
            applied_ingredients = saleitem.ingredients_applied.all(with_deleted=True)
            cnt_after_delete = applied_ingredients.count()
            self.assertEqual(cnt_before_delete - 1 , cnt_after_delete)
            deleted_exist = applied_ingredients.filter(pk=deleted_pk).exists()
            self.assertEqual(deleted_exist, False)
            if cnt_after_delete == 0:
                break

    def test_hard_delete_bulk_ok(self):
        saleitem = self._rand_choose_saleitem_instance()
        applied_ingredients = saleitem.ingredients_applied.values_list('pk')
        cnt_before_delete  = applied_ingredients.count()
        num_composite_half = applied_ingredients.count() >> 1
        # note: start fetching from database once you start iterating the queryset
        deleted_pks_1 = applied_ingredients[:num_composite_half]
        applied_ingredients = saleitem.ingredients_applied.values('pk')
        deleted_pks_2 = applied_ingredients[num_composite_half:]
        # if assertion failure happenes to following code, user just needs
        # to add more fixture data for testing
        self.assertGreater(deleted_pks_1.count(), 1)
        self.assertGreater(deleted_pks_2.count(), 1)

        # currently not support query by composite pk stored in another queryset
        composites_from_db = ProductSaleableItemComposite.objects.filter(pk__in=tuple(deleted_pks_2))
        self.assertEqual(composites_from_db.count(), deleted_pks_2.count())
        composites_from_db.delete(hard=True)
        cnt_after_delete  = saleitem.ingredients_applied.all(with_deleted=True).count()
        self.assertEqual(cnt_after_delete, deleted_pks_1.count())

        composite_field_names = [f.name for f in deleted_pks_1.model._meta.pk._composite_fields]
        remain_pks = [{composite_field_names[jdx]: deleted_pks_1[idx][jdx] for jdx in \
                range(len(composite_field_names))} for idx in range(cnt_after_delete)]
        composites_from_db = ProductSaleableItemComposite.objects.filter(pk__in=remain_pks)
        self.assertEqual(composites_from_db.count(), cnt_after_delete)
        composites_from_db.delete(hard=True)
        cnt_after_delete  = saleitem.ingredients_applied.all(with_deleted=True).count()
        self.assertEqual(cnt_after_delete, 0)


    ##def test_soft_delete_single_item_ok(self):
    ##    pass

    def test_soft_delete_bulk_ok(self):
        saleitem = self._rand_choose_saleitem_instance()
        expected_num_deleted = 3
        all_composites = saleitem.ingredients_applied.order_by('id')
        origin_num_compos = all_composites.count()
        discarded_composites = all_composites[:expected_num_deleted]
        cnt_before_delete  = discarded_composites.count()
        discarded_pks = tuple(discarded_composites.values('id'))
        self.assertEqual(expected_num_deleted, cnt_before_delete)
        self.assertEqual(expected_num_deleted, len(discarded_pks))
        # perform soft-delete operation
        discarded_composites.delete(profile_id=saleitem.usrprof)
        del discarded_composites
        empty_qset = saleitem.ingredients_applied.filter(with_deleted=False, id__in=discarded_pks)
        self.assertEqual(empty_qset.exists(), False)
        softdel_qset = saleitem.ingredients_applied.filter(with_deleted=True, id__in=discarded_pks)
        self.assertEqual(softdel_qset.count(), cnt_before_delete)
        for instance in softdel_qset:
            self.assertEqual(instance.is_deleted(), True)
        # recover state of soft-deleted instances
        softdel_qset.undelete(profile_id=saleitem.usrprof)
        cnt_after_undelete = saleitem.ingredients_applied.all(with_deleted=False).count()
        self.assertEqual(origin_num_compos, cnt_after_undelete)
        undel_qset = saleitem.ingredients_applied.filter(with_deleted=False, id__in=discarded_pks)
        self.assertEqual(undel_qset.count(), cnt_before_delete)
        for instance in undel_qset:
            self.assertEqual(instance.is_deleted(), False)

## end of class SaleableItemCompositeDeletionTestCase



