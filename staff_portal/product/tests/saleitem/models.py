import random
import math
import copy
import json
from functools import partial, reduce

from django.test import TransactionTestCase
from django.db import DEFAULT_DB_ALIAS
from django.db.models import Q
from django.db.models import Count
from django.core.exceptions import ObjectDoesNotExist
from django.db.utils import IntegrityError, DataError
from django.contrib.contenttypes.models  import ContentType

from common.models.enums   import UnitOfMeasurement
from common.util.python import flatten_nested_iterable, sort_nested_object

from product.models.base import ProductTag, ProductTagClosure, ProductAttributeType, _ProductAttrValueDataType, ProductSaleableItem, ProductSaleableItemComposite, ProductAppliedAttributePrice, ProductSaleableItemMedia
from product.models.development import ProductDevIngredientType, ProductDevIngredient

from product.tests.common import _null_test_obj_attrs, _gen_ingredient_attrvals, _ingredient_attrvals_common_setup, SoftDeleteCommonTestMixin
from .common import _fixtures, listitem_rand_assigner, _common_instances_setup, _load_init_params, _modelobj_list_to_map, _product_tag_closure_setup, _dict_key_replace


num_uom = len(UnitOfMeasurement.choices)


class SaleableItemSimpleCreationTestCase(TransactionTestCase):
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

    def test_null_test_obj_attrs(self):
        field_names = ['name', 'usrprof', 'visible', 'price']
        instance = self.instances[0]
        _null_test_obj_attrs(testcase=self,  instance=instance, field_names=field_names)

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
## end of class SaleableItemSimpleCreationTestCase


class SaleableItemSimpleDeletionTestCase(TransactionTestCase):
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
        with self.assertRaises(KeyError) as e:
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
## end of class SaleableItemSimpleDeletionTestCase


class SaleableItemCompositeCreationMixin:
    def setUp(self):
        self.instances = {'ProductDevIngredient': None, 'ProductSaleableItem': None}
        models_info = [ (ProductDevIngredient, len(_fixtures['ProductDevIngredient'])),
                (ProductSaleableItem, len(_fixtures['ProductSaleableItem'])) ]
        _common_instances_setup(out=self.instances, models_info=models_info)

    def tearDown(self):
        pass

    def _bulk_create(self, num_compo):
        min_num_chosen = num_compo << 4
        max_num_chosen = min_num_chosen + 1
        uom_gen = listitem_rand_assigner(list_=UnitOfMeasurement.choices, distinct=False,
            min_num_chosen=min_num_chosen, max_num_chosen=max_num_chosen)
        saleitem_gen = listitem_rand_assigner(list_=self.instances['ProductSaleableItem'],
                distinct=False, min_num_chosen=min_num_chosen, max_num_chosen=max_num_chosen)
        ingredient_gen = listitem_rand_assigner(list_=self.instances['ProductDevIngredient'],
                distinct=False, min_num_chosen=min_num_chosen, max_num_chosen=max_num_chosen)
        saleitems_composite = []
        while len(saleitems_composite) < num_compo:
            try:
                instance = ProductSaleableItemComposite(
                    sale_item  = next(saleitem_gen),
                    ingredient = next(ingredient_gen),
                    unit = next(uom_gen)[0],
                    quantity   = random.randrange(1,30)
                )
                instance.save(force_insert=True)
                saleitems_composite.append(instance)
            except  IntegrityError as e:
                pass
        return saleitems_composite

    def _assert_query_dict_equal(self, actual, expect):
        self.assertGreater(len(expect), 0)
        self.assertEqual(actual.count(), len(expect))
        actual_dict = {tuple(obj.pk.values()): None for obj in actual}
        expect_dict = {tuple(obj.pk.values()): None for obj in expect}
        self.assertDictEqual(actual_dict, expect_dict)
## end of class SaleableItemCompositeCreationMixin


class SaleableItemCompositeCreationTestCase(SaleableItemCompositeCreationMixin, TransactionTestCase):
    def test_null_test_obj_attrs(self):
        instance = ProductSaleableItemComposite(
                sale_item  = self.instances['ProductSaleableItem'][0],
                ingredient = self.instances['ProductDevIngredient'][0],
                quantity   = 2,
                unit = UnitOfMeasurement.UNIT.value,
            )
        field_names = ['quantity', 'unit', 'sale_item', 'ingredient']
        _null_test_obj_attrs(testcase=self,  instance=instance, field_names=field_names)

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
        num_compo = 20
        saleitems_composite = self._bulk_create(num_compo)
        sale_item_ids  = list(map(lambda obj:obj.sale_item.pk,  saleitems_composite))
        ingredient_ids = list(map(lambda obj:obj.ingredient.pk, saleitems_composite))
        cond = Q(pk__isnull={'sale_item__in': sale_item_ids,  'ingredient__in': ingredient_ids})
        qset = ProductSaleableItemComposite.objects.filter(cond)
        self.assertEqual(qset.count(), num_compo)
        self._assert_query_case_1(saleitems_composite, cond)
        self._assert_query_case_2(saleitems_composite, cond)
    ## end of test_bulk_create_ok()


    def _assert_query_case_1(self, saleitems_composite, cond):
        chosen_unit = saleitems_composite[-1].unit
        cond = cond & Q(unit=chosen_unit)
        actual = ProductSaleableItemComposite.objects.filter(cond)
        expect = list(filter(lambda obj: obj.unit == chosen_unit, saleitems_composite))
        self._assert_query_dict_equal(actual=actual, expect=expect)

    def _assert_query_case_2(self, saleitems_composite, cond):
        chosen_unit = saleitems_composite[0].unit
        cond = cond & Q(unit__lte=chosen_unit)
        actual = ProductSaleableItemComposite.objects.filter(cond)
        expect = list(filter(lambda obj: obj.unit <= chosen_unit, saleitems_composite))
        self._assert_query_dict_equal(actual=actual, expect=expect)

    ##def test_complex_query_with_composite_pk(self):
    ##    cond = (Q(quantity__gt=2.7182) | Q(pk__isnull={'sale_item__gte': 8, 'ingredient__in': [2,13, 809], 'sale_item__lt': 29})) & Q(unit__lte=30)
    ##    qset = ProductSaleableItemComposite.objects.filter(cond)
    ##    self.assertEqual(qset.count() , 0)


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
## end of class SaleableItemCompositeCreationTestCase


class SaleableItemCompositeQueryTestCase(SaleableItemCompositeCreationMixin, TransactionTestCase):
    def setUp(self):
        super().setUp()
        self._saleitems_composite = self._bulk_create(num_compo=15)

    def test_id_in_syntax(self):
        saleitems_composite = self._saleitems_composite
        num_chosen_composites = len(saleitems_composite) >> 1
        composite_gen = listitem_rand_assigner(list_=saleitems_composite, distinct=True,
            min_num_chosen=num_chosen_composites, max_num_chosen=(num_chosen_composites + 1))
        chosen_composites = list(composite_gen)
        quantity_gen = map(lambda obj: obj.quantity, chosen_composites)
        avg_quantity = reduce(lambda a,b: a+b, quantity_gen) / len(chosen_composites)
        compo_ids = list(map(lambda obj: obj.pk, chosen_composites))
        cond = Q(id__in=compo_ids) & Q(quantity__gt=avg_quantity)
        actual = ProductSaleableItemComposite.objects.filter(cond)
        expect = list(filter(lambda obj: obj.quantity > avg_quantity , chosen_composites))
        self._assert_query_dict_equal(actual=actual, expect=expect)

    def test_multi_columns_compare(self):
        saleitems_composite = self._saleitems_composite
        compo_ids = list(map(lambda obj:  obj.pk, saleitems_composite))
        sale_item_ids  = list(map(lambda item: item['sale_item'],   compo_ids))
        ingredient_ids = list(map(lambda item: item['ingredient'] , compo_ids))
        avg_saleitem_id   = reduce(lambda a,b: a+b, sale_item_ids ) / len(sale_item_ids )
        avg_ingredient_id = reduce(lambda a,b: a+b, ingredient_ids) / len(ingredient_ids)
        avg_saleitem_id   = math.ceil(avg_saleitem_id  )
        avg_ingredient_id = math.ceil(avg_ingredient_id)
        chosen_unit = saleitems_composite[-1].unit
        cond = Q(pk={'sale_item__gt': avg_saleitem_id, 'ingredient__lt':avg_ingredient_id})
        cond = cond | Q(unit=chosen_unit)
        actual = ProductSaleableItemComposite.objects.filter(cond)
        _filter_fn = lambda obj: (obj.unit == chosen_unit) or (obj.sale_item_id > avg_saleitem_id \
                 and obj.ingredient_id < avg_ingredient_id)
        expect = list(filter(_filter_fn, saleitems_composite))
        self._assert_query_dict_equal(actual=actual, expect=expect)


    def test_ingredients_applied_value(self):
        field_names = ['ingredients_applied', 'ingredients_applied__unit', 'ingredients_applied__quantity']
        # note composite pk is not fully supported,
        # following use cases will lead to error on resolving aggregate expression
        # * Count('ingredients_applied')
        # * QuerySet.values('ingredients_applied__id')
        # * QuerySet.values('ingredients_applied__pk')
        actual_qset = ProductSaleableItem.objects.annotate(num_ingre=Count('ingredients_applied__ingredient')
                ).filter(num_ingre__gt=0).values(*field_names)
        for value in actual_qset:
            compo_pk = {'ingredient': value['ingredients_applied__ingredient_id'],
                    'sale_item': value['ingredients_applied__sale_item_id']}
            saleitem = ProductSaleableItem.objects.get(id=value['ingredients_applied__sale_item_id'])
            expect_composite = saleitem.ingredients_applied.filter(pk=compo_pk)
            self.assertEqual(expect_composite.count(), 1)
            expect_composite = expect_composite.first()
            self.assertEqual(expect_composite.unit     , value['ingredients_applied__unit'])
            self.assertEqual(expect_composite.quantity , value['ingredients_applied__quantity'])

    def test_ingredients_applied_valuelist(self):
        field_names = ['ingredients_applied__unit', 'ingredients_applied__quantity', 'ingredients_applied', ]
        # note composite pk is not fully supported,
        # following use cases will lead to error on resolving aggregate expression
        # * QuerySet.values_list('ingredients_applied', flat=True) <-- `flat` is NOT
        #   supported since  `ingredients_applied` is composite pk field
        actual_qset = ProductSaleableItem.objects.annotate(num_ingre=Count('ingredients_applied__ingredient')
                ).filter(num_ingre__gt=0).values_list(*field_names)
        compo_fields = ProductSaleableItemComposite._meta.pk._composite_fields
        compo_fields = [f.name for f in compo_fields]
        for value in actual_qset:
            compo_pk = {compo_fields[idx]: value[2 + idx] for idx in range(len(compo_fields))}
            expect_composite = ProductSaleableItemComposite.objects.filter(id=compo_pk)
            self.assertEqual(expect_composite.count(), 1)
            expect_composite = expect_composite.first()
            self.assertEqual(expect_composite.unit     , value[0])
            self.assertEqual(expect_composite.quantity , value[1])

    def test_ingredients_applied_filter(self):
        base_qset = ProductSaleableItem.objects.annotate(num_ingre=Count('ingredients_applied__ingredient')
                ).filter(num_ingre__gt=0)
        for expect_compo in self._saleitems_composite:
            actual_qset = base_qset.filter(ingredients_applied=expect_compo.id)
            self.assertEqual(actual_qset.count(), 1)
            actual_saleitem = actual_qset.first()
            self.assertEqual(expect_compo.sale_item.id, actual_saleitem.id)
## end of class SaleableItemCompositeQueryTestCase


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


def _saleitem_attrvals_refresh_from_db(saleitems):
    fresh_attrvals = [getattr(saleitem, dtype_item[0][1]).all() for saleitem in \
            saleitems for dtype_item in _ProductAttrValueDataType]
    fresh_attrvals = flatten_nested_iterable(list_=fresh_attrvals)
    return tuple(fresh_attrvals)



class SaleableItemAttributeCreationTestCase(TransactionTestCase):
    num_saleitems   = 3
    num_attr_types  = len(_fixtures['ProductAttributeType'])
    instances = {'ProductAttributeType': None, 'ProductSaleableItem': None,}

    def setUp(self):
        models_info = [
                (ProductAttributeType, self.num_attr_types),
                (ProductSaleableItem, self.num_saleitems)
            ]
        _common_instances_setup(out=self.instances , models_info=models_info)

    def tearDown(self):
        pass

    def _choose_attr_type(self, dtype:int):
        _fn = lambda x: x.dtype == dtype
        filtered = tuple(filter(_fn, self.instances['ProductAttributeType']))
        assert any(filtered), 'attribute type not found due to incorrect data type %s' % dtype
        return filtered[0]

    def test_create_one_with_invalid_attr_value(self):
        saleitem = self.instances['ProductSaleableItem'][-1]
        data_types = [
            (_ProductAttrValueDataType.STRING.value[0], 'sticky'),
            (_ProductAttrValueDataType.FLOAT.value[0], 2.7811),
            (_ProductAttrValueDataType.INTEGER.value[0], -13),
            (_ProductAttrValueDataType.POSITIVE_INTEGER.value[0], 29),
        ]
        attrval_objs = {}
        for dtype_id, corresponding_test_value in data_types:
            attrtype_ref = self._choose_attr_type(dtype=dtype_id[0])
            instance = attrtype_ref.attr_val_set.model(
                    ingredient_type = ContentType.objects.get_for_model(saleitem),
                    ingredient_id = saleitem.pk,
                    attr_type = attrtype_ref,
                    value = corresponding_test_value,
                )
            field_names = ['ingredient_type', 'ingredient_id', 'attr_type', 'value']
            _null_test_obj_attrs(testcase=self, instance=instance, field_names=field_names)
            attrval_objs[dtype_id[0]] = instance

        instance = attrval_objs[_ProductAttrValueDataType.INTEGER.value[0][0]]
        # test integer sttribute with string value
        with self.assertRaises(ValueError):
            instance.value = 'xyz'
            instance.save()
        # since MariaDB is applied to this project, it implicitly converts 
        # * float to integer for integer column
        # * any number to string for varchar column
        # so I am not able to test these cases
        ##with self.assertRaises(IntegrityError):
        ##    instance.value = 13.0838045
        ##    instance.save()
        instance = attrval_objs[_ProductAttrValueDataType.POSITIVE_INTEGER.value[0][0]]
        with self.assertRaises(DataError) as e:
            instance.value = -2
            instance.save()
        with self.assertRaises(ValueError):
            instance.value = 'qwe'
            instance.save()
    ## end of test_create_one_with_invalid_attr_value()


    def test_create_one_with_extra_charge(self):
        saleitem = self.instances['ProductSaleableItem'][0]
        def assert_new_object(attrtype):
            limit = len(_fixtures['ProductAppliedAttributePrice'])
            idx = random.randrange(0,1000)
            extra_charge = _fixtures['ProductAppliedAttributePrice'][idx % limit]
            self.assertNotEqual(extra_charge, None)
            attrval = _gen_ingredient_attrvals(attrtype, saleitem, idx, extra_charge=extra_charge)
            attrval.save(force_insert=True)
            self.assertEqual(extra_charge, attrval.extra_charge)
        tuple(map(assert_new_object, self.instances['ProductAttributeType']))


    def test_create_bulk_ok(self):
        saleitems = self.instances['ProductSaleableItem'][:self.num_saleitems]
        attrtypes = self.instances['ProductAttributeType']
        attrtypes_gen_fn = partial(listitem_rand_assigner, list_=attrtypes)
        _attrval_objs = _ingredient_attrvals_common_setup(ingredients=saleitems,
                attrtypes_gen_fn=attrtypes_gen_fn)
        for saleitem in saleitems:
            for dtype_item in _ProductAttrValueDataType:
                expect_objs = _attrval_objs[dtype_item[0][0]].get(saleitem.pk, [])
                if not expect_objs:
                    continue
                related_field_mgr = getattr(saleitem, dtype_item[0][1])
                actual_objs = related_field_mgr.all()
                expected_cnt = len(expect_objs)
                actual_cnt = actual_objs.count()
                self.assertGreater(actual_cnt, 0)
                self.assertEqual(expected_cnt, actual_cnt)
                expected_values = [{'value':x.value, 'type':x.attr_type.pk, 'saleitem':x.ingredient_id} for x in expect_objs]
                actual_values   = [{'value':x.value, 'type':x.attr_type.pk, 'saleitem':x.ingredient_id} for x in actual_objs]
                expected_values = json.dumps(expected_values, sort_keys=True)
                actual_values   = json.dumps(actual_values  , sort_keys=True)
                self.assertEqual(expected_values, actual_values)
    ## end of test_create_bulk_ok()
## end of class SaleableItemAttributeCreationTestCase


def _saleitem_attrvals_extracharge_setup(attrvals, retrieve_id=False):
    attrvals_gen = listitem_rand_assigner(list_=attrvals, min_num_chosen=(len(attrvals) >> 1))
    extra_charge_objs = [
        ProductAppliedAttributePrice(
            attrval_type = ContentType.objects.get_for_model(attrval),
            attrval_id = attrval.pk,
            amount = random.random() * 100
        ) for attrval in attrvals_gen
    ]
    ProductAppliedAttributePrice.objects.bulk_create(extra_charge_objs)
    if retrieve_id:
        extra_charge_objs = list(ProductAppliedAttributePrice.objects.all())
    return  extra_charge_objs



class SaleableItemAttributeDeletionTestCase(TransactionTestCase):
    num_saleitems   = len(_fixtures['ProductSaleableItem'])
    num_attr_types  = len(_fixtures['ProductAttributeType'])
    attrval_field_names = ['ingredient_type','ingredient_id','attr_type','value','id']
    instances = {'ProductAttributeType': None, 'ProductSaleableItem': None,
            'BaseProductAttributeValue':None, 'ProductAppliedAttributePrice':None}

    def setUp(self):
        models_info = [
                (ProductAttributeType, self.num_attr_types),
                (ProductSaleableItem, self.num_saleitems)
            ]
        _common_instances_setup(out=self.instances , models_info=models_info)
        saleitems = self.instances['ProductSaleableItem']
        attrtypes = self.instances['ProductAttributeType']
        attrtypes_gen_fn = partial(listitem_rand_assigner, list_=attrtypes)
        _ingredient_attrvals_common_setup(ingredients=saleitems, attrtypes_gen_fn=attrtypes_gen_fn)
        self.instances['BaseProductAttributeValue'] = _saleitem_attrvals_refresh_from_db(saleitems)
        self.instances['ProductAppliedAttributePrice'] = _saleitem_attrvals_extracharge_setup(
                attrvals=self.instances['BaseProductAttributeValue'], )

    def tearDown(self):
        pass

    def _retrieve_expect_deleted_data(self, qset):
        model_cls = qset.model
        deleted_ids = qset.values_list('pk', flat=True)
        delated_ct = ContentType.objects.get_for_model(model_cls)
        attrval_field_names_clone = ['extra_charge']
        attrval_field_names_clone.extend(self.attrval_field_names)
        def filter_fn(obj):
            id_matched = obj.pk in deleted_ids
            ct_matched = delated_ct.pk == ContentType.objects.get_for_model(obj).pk
            return ct_matched and id_matched
        def serialize_fn(obj):
            return obj.serializable(present=attrval_field_names_clone, present_null=True)
        out = filter(filter_fn, self.instances['BaseProductAttributeValue'])
        out = map(serialize_fn, out)
        return list(out)

    def _assert_before_delete(self):
        bound_dict_key_replace = partial(_dict_key_replace, to_='extra_charge', from_='_extra_charge__amount')
        for saleitem in self.instances['ProductSaleableItem']:
            for dtype_item in _ProductAttrValueDataType:
                manager = getattr(saleitem, dtype_item[0][1])
                qset = manager.all()
                cnt_before_delete = qset.count()
                if cnt_before_delete == 0:
                    continue
                expect_deleted_data = self._retrieve_expect_deleted_data(qset)
                actual_deleted_data = qset.values('_extra_charge__amount',*self.attrval_field_names)
                actual_deleted_data = list(map(bound_dict_key_replace, actual_deleted_data))
                expect_deleted_data = sorted(expect_deleted_data, key=lambda x: x['id'])
                actual_deleted_data = sorted(actual_deleted_data, key=lambda x: x['id'])
                # list assertion function expects the items from both lists are
                # in the same order by some kind of key value
                self.assertListEqual(expect_deleted_data, actual_deleted_data)
                yield manager, qset, cnt_before_delete, expect_deleted_data

    def test_hard_delete_bulk_ok(self):
        for manager, qset, _, _ in self._assert_before_delete():
            qset.delete(hard=True)
            qset = manager.all(with_deleted=True)
            cnt_after_delete = qset.count()
            self.assertEqual(cnt_after_delete, 0)

    def test_soft_delete_bulk_ok(self):
        profile_id = 345
        bound_dict_key_replace = partial(_dict_key_replace, to_='extra_charge', from_='_extra_charge__amount')
        for manager, qset, cnt_before_delete, expect_deleted_data in self._assert_before_delete():
            qset.delete(profile_id=profile_id)
            qset2 = manager.all()
            self.assertEqual(qset2.count(), 0)
            qset3 = manager.all(with_deleted=True)
            cnt_after_delete = qset3.count()
            self.assertGreater(cnt_after_delete, 0)
            self.assertEqual(cnt_after_delete, cnt_before_delete)
            # double-check soft-deleted instances
            actual_deleted_data = qset3.values('_extra_charge__amount',*self.attrval_field_names)
            actual_deleted_data = list(map(bound_dict_key_replace, actual_deleted_data))
            actual_deleted_data = sorted(actual_deleted_data, key=lambda x: x['id'])
            self.assertListEqual(expect_deleted_data, actual_deleted_data)
            # un-delete
            qset3.undelete(profile_id=profile_id)
            qset4 = manager.all()
            self.assertEqual(qset4.count(), cnt_before_delete)
## end of class SaleableItemAttributeDeletionTestCase


def _gen_saleitem_composite(idx, uom_gen, saleitem, ingredient_gen):
    composite = ProductSaleableItemComposite()
    composite.unit = next(uom_gen)[0]
    composite.quantity = random.randrange(6,120)
    composite.sale_item  = saleitem
    composite.ingredient = next(ingredient_gen)
    return composite

def _saleitem_composites_common_setup(saleitem, out:dict, ingredients):
    num_ingredients = len(ingredients)
    num_composites = random.randrange(3, num_ingredients)
    uom_gen = listitem_rand_assigner(list_=UnitOfMeasurement.choices, distinct=False,
            min_num_chosen=num_composites, max_num_chosen=(num_composites + 1))
    ingredient_gen = listitem_rand_assigner(list_=ingredients,  min_num_chosen=num_ingredients)
    gen_compo_fn = partial(_gen_saleitem_composite, uom_gen=uom_gen, saleitem=saleitem,
            ingredient_gen=ingredient_gen)
    composites = list(map(gen_compo_fn, range(num_composites)))
    ProductSaleableItemComposite.objects.bulk_create(composites)
    stored_composites = ProductSaleableItemComposite.objects.filter(sale_item=saleitem.id
            ).values('id', 'unit', 'quantity')
    out[saleitem.pk] = list(stored_composites)


class SaleableItemAdvancedDeletionTestCase(TransactionTestCase, SoftDeleteCommonTestMixin):
    num_saleitems = random.randrange(3, len(_fixtures['ProductSaleableItem']))
    num_ingredients = len(_fixtures['ProductDevIngredient'])
    num_attr_types  = len(_fixtures['ProductAttributeType'])
    num_tags = len(_fixtures['ProductTag'])
    instances = {'ProductAttributeType': None, 'ProductSaleableItem': None,
            'ProductDevIngredient': None, 'ProductTag':None,  'ProductTagClosure':None,
            'BaseProductAttributeValue':None,  'ProductAppliedAttributePrice':None,
            'ProductSaleableItemComposite':{}, 'ProductSaleableItemMedia': {},
            'tagged_saleitems': {},
        }

    def setUp(self):
        models_info = [
                (ProductTag,  self.num_tags),
                (ProductAttributeType, self.num_attr_types),
                (ProductSaleableItem, self.num_saleitems),
                (ProductDevIngredient, self.num_ingredients)
            ]
        _common_instances_setup(out=self.instances , models_info=models_info)
        # attribute values & extra charge
        attrtypes_gen_fn = partial(listitem_rand_assigner, list_=self.instances['ProductAttributeType'])
        _ingredient_attrvals_common_setup(ingredients=self.instances['ProductSaleableItem'], attrtypes_gen_fn=attrtypes_gen_fn,)
        self.instances['BaseProductAttributeValue'] = _saleitem_attrvals_refresh_from_db(saleitems=self.instances['ProductSaleableItem'])
        self.instances['ProductAppliedAttributePrice'] = _saleitem_attrvals_extracharge_setup(
                attrvals=self.instances['BaseProductAttributeValue'], retrieve_id=True)
        # composite & ingredient
        bound_composites_setup = partial(_saleitem_composites_common_setup,
                ingredients=self.instances['ProductDevIngredient'],
                out=self.instances['ProductSaleableItemComposite'], )
        tuple(map(bound_composites_setup, self.instances['ProductSaleableItem']))
        # tag
        self.instances['ProductTag'] = _modelobj_list_to_map(self.instances['ProductTag'])
        self.instances['ProductTagClosure'] = _product_tag_closure_setup(
                tag_map=self.instances['ProductTag'], data=_fixtures['ProductTagClosure'])
        for saleitem in self.instances['ProductSaleableItem']:
            tags_gen = listitem_rand_assigner(list_=self.instances['ProductTag'].values())
            applied_tags = list(tags_gen)
            saleitem.tags.set(applied_tags)
            self.instances['tagged_saleitems'][saleitem.pk] = list(saleitem.tags.all())
        # media link
        for saleitem in self.instances['ProductSaleableItem']:
            _media_meta_gen = listitem_rand_assigner(list_=_fixtures['ProductSaleableItemMedia'])
            [saleitem.media_set.create(media=m['media'])  for m in _media_meta_gen]
            ##saleitem.media_set.set(media_meta_objs, bulk=False, clear=True) # like update_or_create()
            # cannot use set() or add() cuz they actually perform update operations
            qset = saleitem.media_set.values('id','sale_item','media')
            self.instances['ProductSaleableItemMedia'][saleitem.pk] = list(qset)


    def tearDown(self):
        self.instances['ProductSaleableItemMedia'].clear()
        self.instances['ProductSaleableItemComposite'].clear()
        self.instances['tagged_saleitems'].clear()

    def _check_remaining_saleitems_to_ingredients(self, saleitem_ids, is_deleted=False):
        chosen_composites = dict(filter(lambda  kv: kv[0] in saleitem_ids,
            self.instances['ProductSaleableItemComposite'].items()))
        chosen_composites = [v2 for v in chosen_composites.values() for v2 in v]
        # check saleable items which include the deleted ingredient
        for ingredient in self.instances['ProductDevIngredient']:
            init_qset = ingredient.saleitems_applied.get_deleted_set() if is_deleted \
                    else ingredient.saleitems_applied.all()
            qset = init_qset.filter(ingredient=ingredient.id, sale_item__in=saleitem_ids)
            actual = list(qset.values('sale_item', 'unit', 'quantity'))
            remain_composites = filter(lambda v: v['ingredient_id'] == ingredient.id, chosen_composites)
            expect = list(map(lambda v: {'sale_item': v['sale_item_id'], 'unit': v['unit'],
                'quantity': float(v['quantity']),}, remain_composites))
            expect = sort_nested_object(obj=expect)
            actual = sort_nested_object(obj=actual)
            #if expect != actual:
            #    import pdb
            #    pdb.set_trace()
            self.assertListEqual(expect, actual)


    def _assert_attr_fields_existence(self, saleitem_pk, _assert_fn):
        filter_attrval_fn = lambda attrval: attrval.ingredient_id == saleitem_pk
        filtered_attrvals = filter(filter_attrval_fn, self.instances['BaseProductAttributeValue'])
        filter_extmnt_fn = lambda extracharge, attrval: (extracharge.attrval_type.pk == \
                ContentType.objects.get_for_model(attrval).pk) and (extracharge.attrval_id == attrval.pk)
        for attrval in filtered_attrvals:
            _assert_fn(attrval)
            bound_filter_extmnt_fn = partial(filter_extmnt_fn, attrval=attrval)
            filtered_extmnts = filter(bound_filter_extmnt_fn, self.instances['ProductAppliedAttributePrice'])
            for extracharge in filtered_extmnts:
                _assert_fn(extracharge)


    def _assert_composite_fields_existence(self, saleitem_pk, _assert_fn, with_deleted):
        composites = self.instances['ProductSaleableItemComposite'][saleitem_pk]
        composites_ids = list(map(lambda x: {'ingredient': x['ingredient_id'], 'sale_item':x['sale_item_id']}, composites))
        qset = ProductSaleableItemComposite.objects.filter(with_deleted=with_deleted, id__in=composites_ids)
        _assert_fn(actual_objs=qset, expect_objs=composites)

    def _assert_media_link_fields_existence(self, saleitem_pk, _assert_fn, with_deleted):
        media_meta = self.instances['ProductSaleableItemMedia'][saleitem_pk]
        media_meta_ids = list(map(lambda x: x['id'] , media_meta))
        qset = ProductSaleableItemMedia.objects.filter(with_deleted=with_deleted, id__in=media_meta_ids)
        _assert_fn(actual_objs=qset, expect_objs=media_meta)


    def test_hard_delete_bulk(self):
        backup_saleitem_pks = list(map(lambda saleitem: saleitem.pk , self.instances['ProductSaleableItem']))
        qset = ProductSaleableItem.objects.filter(pk__in=backup_saleitem_pks)
        self.assertEqual(qset.count(), self.num_saleitems)
        qset = qset[:(self.num_saleitems - 1)]
        qset.delete(hard=True)
        qset = ProductSaleableItem.objects.filter(with_deleted=True, pk__in=backup_saleitem_pks)
        self.assertEqual(qset.count(), 1)
        self.assertEqual((qset.first().pk in  backup_saleitem_pks), True)
        backup_saleitem_pks.remove( qset.first().pk )
        def _assert_att_fn(obj):
            self.assertNotEqual(obj.pk, None)
            with self.assertRaises(ObjectDoesNotExist):
                obj.refresh_from_db()
        def _assert_compo_fn(actual_objs, expect_objs):
            self.assertEqual(actual_objs.exists(), False)
        def _assert_media_fn(actual_objs, expect_objs):
            self.assertEqual(actual_objs.exists(), False)
        for saleitem_pk in backup_saleitem_pks:
            self._assert_attr_fields_existence(saleitem_pk, _assert_att_fn)
            self._assert_composite_fields_existence(saleitem_pk, _assert_compo_fn, with_deleted=True)
            self._assert_media_link_fields_existence(saleitem_pk, _assert_media_fn, with_deleted=True)


    def test_soft_delete_bulk(self):
        def _assert_del_status_fn(obj):
            self.assertNotEqual(obj.pk, None)
            self.assertFalse(obj.is_deleted())
            obj.refresh_from_db()
            self.assertTrue(obj.is_deleted())

        def _assert_undel_status_fn(obj):
            self.assertNotEqual(obj.pk, None)
            self.assertTrue(obj.is_deleted())
            obj.refresh_from_db()
            self.assertFalse(obj.is_deleted())

        def _assert_tag_fn(obj):
            tags = obj.tags.all()
            self.assertFalse(tags.exists())

        def _assert_compo_fn(actual_objs, expect_objs):
            self.assertGreater(actual_objs.count(), 0)
            self.assertEqual(actual_objs.count(), len(expect_objs))
            actual_objs = actual_objs.values('id','unit', 'quantity')
            actual_objs = sorted(actual_objs, key=lambda x: x['ingredient_id'])
            expect_objs = sorted(expect_objs, key=lambda x: x['ingredient_id'])
            self.assertListEqual(actual_objs, expect_objs)

        def _assert_media_fn(actual_objs, expect_objs):
            self.assertGreater(actual_objs.count(), 0)
            self.assertEqual(actual_objs.count(), len(expect_objs))
            actual_objs = actual_objs.values('id','sale_item','media')
            actual_objs = sorted(actual_objs, key=lambda x: x['id'])
            expect_objs = sorted(expect_objs, key=lambda x: x['id'])
            self.assertListEqual(actual_objs, expect_objs)

        profile_id = 91
        backup_saleitem_pks = list(map(lambda saleitem: saleitem.pk , self.instances['ProductSaleableItem']))
        qset = ProductSaleableItem.objects.filter(pk__in=backup_saleitem_pks)
        self.assertEqual(qset.count(), self.num_saleitems)
        remain_id = qset.last().pk
        qset = qset[:(self.num_saleitems - 1)]
        deleted_ids = list(qset.values_list('id', flat=True))
        remain_ids  = [remain_id]
        # soft-delete
        qset.delete(profile_id=profile_id)
        tuple(map(lambda obj:self.assertTrue(obj.is_deleted()), qset)) # auto marked as deleted
        tuple(map(_assert_tag_fn, qset))
        self.assert_softdelete_items_exist(testcase=self, deleted_ids=deleted_ids, remain_ids=remain_ids,
                model_cls_path='product.models.base.ProductSaleableItem',)
        self._check_remaining_saleitems_to_ingredients(saleitem_ids=deleted_ids, is_deleted=True)
        self._check_remaining_saleitems_to_ingredients(saleitem_ids=remain_ids)
        for saleitem_pk in deleted_ids:
            self._assert_attr_fields_existence(saleitem_pk, _assert_del_status_fn)
            self._assert_composite_fields_existence(saleitem_pk, _assert_compo_fn, with_deleted=True)
            self._assert_media_link_fields_existence(saleitem_pk, _assert_media_fn, with_deleted=True)
        # un-delete
        self.assertEqual(qset.count(), len(deleted_ids))
        qset.undelete(profile_id=profile_id)
        tuple(map(_assert_undel_status_fn , qset))
        tuple(map(_assert_tag_fn, qset))
        self._check_remaining_saleitems_to_ingredients(saleitem_ids=deleted_ids)
        for saleitem_pk in  deleted_ids:
            self._assert_attr_fields_existence(saleitem_pk, _assert_undel_status_fn)
            self._assert_composite_fields_existence(saleitem_pk, _assert_compo_fn, with_deleted=False)
            self._assert_media_link_fields_existence(saleitem_pk, _assert_media_fn, with_deleted=False)
    ## end of test_soft_delete_bulk()
## end of class SaleableItemAdvancedDeletionTestCase



