import random
import copy
import json
from functools import partial, reduce

from django.test import TransactionTestCase
from django.db.models import Q
from django.db.models import Count
from django.core.exceptions import ObjectDoesNotExist
from django.db.utils import IntegrityError, DataError
from django.contrib.contenttypes.models  import ContentType

from common.models.enums   import UnitOfMeasurement
from product.models.base import ProductTag, ProductTagClosure, ProductAttributeType, _ProductAttrValueDataType, ProductSaleableItem, ProductSaleablePackage,  ProductSaleableItemMedia, ProductSaleableItemComposite, ProductSaleablePackageComposite, ProductSaleablePackageMedia,  ProductAppliedAttributePrice
from product.models.development import ProductDevIngredientType, ProductDevIngredient

from product.tests.common import _fixtures, _null_test_obj_attrs, _load_init_params, _common_instances_setup, _modelobj_list_to_map, _product_tag_closure_setup, listitem_rand_assigner, assert_field_equal
from .common import _attr_vals_fixture_map, diff_created_ingredients

num_uom = len(UnitOfMeasurement.choices)


def _init_tag_attrtype(stored_models, num_tags=None, num_attrtypes=None):
    model_fixtures = _fixtures
    if num_tags is None:
        num_tags = len(model_fixtures['ProductTag'])
    if num_attrtypes is None:
        num_attrtypes = len(model_fixtures['ProductAttributeType'])
    models_info = [
            (ProductTag, num_tags),
            (ProductAttributeType, num_attrtypes  ),
        ]
    _common_instances_setup(out=stored_models, models_info=models_info)
    tag_map = _modelobj_list_to_map(stored_models['ProductTag'])
    stored_models['ProductTagClosure'] = _product_tag_closure_setup(
        tag_map=tag_map, data=model_fixtures['ProductTagClosure'])



def _gen_attrval_dataitem(obj, enable_extra_amount=False):
    attrval_opts = _attr_vals_fixture_map[obj.dtype]
    chosen_idx = random.randrange(0, len(attrval_opts))
    item = {'type':obj, 'value': attrval_opts[chosen_idx]}
    if enable_extra_amount is True and random.randint(a=0, b=1) > 0:
        item['extra_amount'] = random.random() * 100
    return item


def _gen_common_nested_field_dataitem(actual_data):
    tags_gen = listitem_rand_assigner(list_=actual_data['ProductTag'])
    tags = list(tags_gen)
    bound_gen_attrval_dataitem = partial(_gen_attrval_dataitem, enable_extra_amount=True)
    pkg_attrtype_gen = listitem_rand_assigner(list_=actual_data['ProductAttributeType'])
    pkg_attrvals     = list(map(bound_gen_attrval_dataitem, pkg_attrtype_gen))
    media_meta_gen = listitem_rand_assigner(list_=_fixtures['ProductSaleableItemMedia'])
    media_meta     = list(media_meta_gen)
    return {'tags':tags, 'attrvals': pkg_attrvals, 'media': media_meta,}


def _gen_composite_data(list_, elm_field_name):
    out = []
    list_gen = listitem_rand_assigner(list_=list_)
    for elm in list_gen:
        chosen_idx = random.randrange(0, num_uom)
        unit = UnitOfMeasurement.choices[chosen_idx]
        quantity = random.random() * 100
        item = {elm_field_name: elm ,'unit': unit, 'quantity': quantity}
        out.append(item)
    return out


def _gen_pkg_expect_data(actual_data, num_salepkgs, num_saleitems, num_ingredients):
    out = {'ProductDevIngredient': [], 'ProductSaleablePackage': [], 'ProductSaleableItem': [],}
    ingre_data_gen = listitem_rand_assigner(list_=_fixtures['ProductDevIngredient'],
            min_num_chosen=num_ingredients, max_num_chosen=(num_ingredients + 1))
    ingre_data_change_fn = lambda d: {'id':d['id'], 'name':d['name'], 'category': d['category'].value}
    for ingre_data  in ingre_data_gen:
        ingre_data = ingre_data_change_fn(ingre_data)
        attrtype_gen = listitem_rand_assigner(list_=actual_data['ProductAttributeType'])
        attrvals     = list(map(_gen_attrval_dataitem, attrtype_gen))
        item = {'simple': ingre_data, 'nested': {'attrvals': attrvals}, 'obj':None}
        out['ProductDevIngredient'].append(item)

    saleitem_data_gen = listitem_rand_assigner(list_=_fixtures['ProductSaleableItem'],
            min_num_chosen=num_saleitems, max_num_chosen=(num_saleitems + 1))
    for saleitem_data  in saleitem_data_gen:
        ingre_composites = _gen_composite_data(out['ProductDevIngredient'], elm_field_name='ingredient')
        nested_item = _gen_common_nested_field_dataitem(actual_data)
        nested_item['composites'] = ingre_composites
        item = {'simple': saleitem_data, 'nested': nested_item, 'obj':None}
        out['ProductSaleableItem'].append(item)

    pkg_data_gen = listitem_rand_assigner(list_=_fixtures['ProductSaleablePackage'],
            min_num_chosen=num_salepkgs, max_num_chosen=(num_salepkgs + 1))
    for pkg_data in pkg_data_gen:
        pkg_composites = _gen_composite_data(out['ProductSaleableItem'], elm_field_name='sale_item')
        nested_item = _gen_common_nested_field_dataitem(actual_data)
        nested_item['composites'] = pkg_composites
        item = {'simple': pkg_data, 'nested': nested_item, 'obj':None}
        out['ProductSaleablePackage'].append(item)
    return out
## end of  _gen_pkg_expect_data()


def _gen_attrval_obj(ingredient, nested_data):
    ingredient_ct = ContentType.objects.get_for_model(ingredient)
    for data in nested_data['attrvals']:
        attrtype_ref = data['type']
        attrval_model_cls = attrtype_ref.attr_val_set.model
        init_kwargs = {'attr_type': attrtype_ref, 'value':data['value'],
                'ingredient_type':ingredient_ct, 'ingredient_id':ingredient.id}
        if data.get('extra_amount'):
            init_kwargs['extra_amount'] = data['extra_amount']
        attrval_obj = attrval_model_cls(**init_kwargs)
        attrval_obj.save()


def _gen_saleitem_mediameta_obj(saleitem, nested_data):
    model_cls = ProductSaleableItemMedia
    for data in nested_data.get('media', []):
        init_kwargs = {'sale_item': saleitem, 'media': data['media']}
        mm_obj = model_cls(**init_kwargs)
        mm_obj.save()

def _gen_saleitem_composite_obj(saleitem, nested_data):
    model_cls = ProductSaleableItemComposite
    for data in nested_data.get('composites', []):
        init_kwargs = {'sale_item':saleitem, 'ingredient': data['ingredient']['obj'],
                'unit':data['unit'][0], 'quantity':data['quantity'],}
        compo_obj = model_cls(**init_kwargs)
        compo_obj.save()

def _gen_salepkg_mediameta_obj(salepkg, nested_data):
    model_cls = ProductSaleablePackageMedia
    for data in nested_data.get('media', []):
        init_kwargs = {'sale_pkg': salepkg, 'media': data['media']}
        mm_obj = model_cls(**init_kwargs)
        mm_obj.save()

def _gen_salepkg_composite_obj(salepkg, nested_data):
    model_cls = ProductSaleablePackageComposite
    for data in nested_data.get('composites', []):
        init_kwargs = {'package':salepkg, 'sale_item': data['sale_item']['obj'],
                'unit':data['unit'][0], 'quantity':data['quantity'],}
        compo_obj = model_cls(**init_kwargs)
        compo_obj.save()


def _gen_actual_data(expect_data, actual_data, model_cls, obj_attr_gen_fn,
        gen_mm_fn=None, gen_compo_fn=None):
    ingredients = []
    for data in expect_data[model_cls.__name__]:
        init_kwargs = obj_attr_gen_fn(data)
        ingredient = model_cls(**init_kwargs)
        ingredient.save()
        data['obj'] = ingredient
        nested_data = data['nested']
        _gen_attrval_obj(ingredient, nested_data)
        data_tags = nested_data.get('tags', [])
        if any(data_tags) and hasattr(ingredient, 'tags'):
            ingredient.tags.set(data_tags)
        if gen_mm_fn and callable(gen_mm_fn):
            gen_mm_fn(ingredient, nested_data)
        if gen_compo_fn and callable(gen_compo_fn):
            gen_compo_fn(ingredient, nested_data)
        ingredient.refresh_from_db()
        ingredients.append(ingredient)
    actual_data[model_cls.__name__] = ingredients


def _salepkg_instances_setup(num_salepkgs, num_saleitems, num_ingredients):
    actual_data = {'ProductAttributeType': None, 'ProductTag':None, 'ProductTagClosure':None,
            'ProductSaleablePackage': None, 'ProductSaleableItem':None, 'ProductDevIngredient': None, }
    _init_tag_attrtype(stored_models=actual_data)
    expect_data = _gen_pkg_expect_data( actual_data, num_salepkgs=num_salepkgs,
            num_saleitems=num_saleitems, num_ingredients=num_ingredients )
    obj_attr_gen_fn = lambda d: d['simple']
    _gen_actual_data(expect_data=expect_data, actual_data=actual_data,
            model_cls=ProductDevIngredient, obj_attr_gen_fn=obj_attr_gen_fn)
    _gen_actual_data(expect_data=expect_data, actual_data=actual_data,
            model_cls=ProductSaleableItem, obj_attr_gen_fn=obj_attr_gen_fn,
            gen_mm_fn=_gen_saleitem_mediameta_obj,  gen_compo_fn=_gen_saleitem_composite_obj)
    _gen_actual_data(expect_data=expect_data, actual_data=actual_data,
            model_cls=ProductSaleablePackage, obj_attr_gen_fn=obj_attr_gen_fn,
            gen_mm_fn=_gen_salepkg_mediameta_obj,   gen_compo_fn=_gen_salepkg_composite_obj)
    return expect_data, actual_data


class SaleablePackageCreationTestCase(TransactionTestCase):
    def setUp(self):
        pass

    def tearDown(self):
        pass

    def test_null_obj_fields(self):
        field_names = ['name', 'price', 'visible']
        instance = ProductSaleablePackage(**_fixtures['ProductSaleablePackage'][0])
        _null_test_obj_attrs(testcase=self, instance=instance, field_names=field_names)

    def test_single_item_min_content(self):
        instance = ProductSaleablePackage(**_fixtures['ProductSaleablePackage'][0])
        instance.save(force_insert=True)
        expect = instance
        qset = ProductSaleablePackage.objects.filter(pk=expect.pk)
        self.assertEqual(qset.count(), 1)
        actual = qset.first()
        self.assertEqual(expect.pk, actual.pk)

    def test_bulk_create_duplicate_id(self):
        num_pkgs = 4
        bound_fn = partial(_load_init_params, model_cls=ProductSaleablePackage)
        instances = list(map(bound_fn , _fixtures['ProductSaleablePackage'][:num_pkgs]))
        dup_ids = [1234, 5678]
        instances[0].id = dup_ids[0]
        instances[1].id = dup_ids[1]
        ProductSaleablePackage.objects.bulk_create(instances[:2])
        instances[2].id = dup_ids[0]
        instances[3].id = dup_ids[1]
        ProductSaleablePackage.objects.bulk_create(instances[2:])
        ids = tuple(map(lambda instance: getattr(instance, 'id'), instances))
        ids = tuple(filter(lambda x: x is not None and x > 0, ids))
        expected = len(ids)
        actual   = len(set(ids)) # test whether all ID numbers are distinct to each other
        self.assertGreater(actual, 0)
        self.assertEqual(expected, actual)

    def test_bulk_ok(self):
        num_salepkgs = 3
        num_saleitems = 6
        num_ingredients = 8
        expect_data, actual_data = _salepkg_instances_setup(num_salepkgs=num_salepkgs,
                num_ingredients=num_ingredients, num_saleitems=num_saleitems)
        diff_created_ingredients(self, expect_data['ProductSaleablePackage'],
                actual_data['ProductSaleablePackage'], lower_elm_names=['sale_item','ingredient', None],
                lower_elm_mgr_fields=['saleitems_applied', 'ingredients_applied', None] )
## end of class SaleablePackageCreationTestCase



class SaleablePackageDeletionTestCase(TransactionTestCase):
    num_salepkgs = 4
    num_saleitems = 5
    num_ingredients = 6

    def setUp(self):
        self._expect_data, self._actual_data = _salepkg_instances_setup( num_salepkgs=self.num_salepkgs,
                num_saleitems=self.num_saleitems, num_ingredients=self.num_ingredients )
        num_delete = self.num_salepkgs >> 1
        self.expect_delete_pkgs = list(listitem_rand_assigner(list_=self._expect_data['ProductSaleablePackage'] ,
            min_num_chosen=num_delete, max_num_chosen=(num_delete + 1)))
        key_fn = lambda d:d['obj'].id
        self.expect_delete_pkgs = sorted(self.expect_delete_pkgs, key=key_fn)
        self.expect_delete_ids = list(map(key_fn, self.expect_delete_pkgs))


    def test_hard_delete(self):
        ProductSaleablePackage.objects.filter(id__in=self.expect_delete_ids).delete(hard=True)
        pkg_qset = ProductSaleablePackage.objects.filter(with_deleted=True,
                id__in=self.expect_delete_ids)
        compo_exists = ProductSaleablePackageComposite.objects.filter(with_deleted=True,
                package__id__in=self.expect_delete_ids).exists()
        mm_exists = ProductSaleablePackageMedia.objects.filter(with_deleted=True,
                sale_pkg__id__in=self.expect_delete_ids).exists()
        self.assertFalse(pkg_qset.exists())
        self.assertFalse(compo_exists)
        self.assertFalse(mm_exists)
        #for dtype_opt in _ProductAttrValueDataType:
        #    pass
        expect_remain_data = tuple(filter(lambda d: d['obj'].id not in self.expect_delete_ids, self._expect_data['ProductSaleablePackage']))
        expect_remain_objs = tuple(filter(lambda obj: obj.id not in self.expect_delete_ids,    self._actual_data['ProductSaleablePackage']))
        diff_created_ingredients(self, expect_data=expect_remain_data, actual_data=expect_remain_objs,
                lower_elm_names=['sale_item','ingredient', None],
                lower_elm_mgr_fields=['saleitems_applied', 'ingredients_applied', None] )


    def test_soft_delete(self):
        profile_id = 123
        qset = ProductSaleablePackage.objects.filter(id__in=self.expect_delete_ids).order_by('id')
        qset.delete(profile_id=profile_id)
        pkg_exists   = ProductSaleablePackage.objects.filter(id__in=self.expect_delete_ids).exists()
        compo_exists = ProductSaleablePackageComposite.objects.filter(package__id__in=self.expect_delete_ids).exists()
        mm_exists = ProductSaleablePackageMedia.objects.filter(sale_pkg__id__in=self.expect_delete_ids).exists()
        self.assertFalse(pkg_exists)
        self.assertFalse(compo_exists)
        self.assertFalse(mm_exists)
        for dtype_opt in _ProductAttrValueDataType:
            obj = qset.first()
            obj_ct = ContentType.objects.get_for_model(obj)
            model_cls = getattr(obj, dtype_opt.value[0][1]).model
            attrval_exists = model_cls.objects.filter(ingredient_type=obj_ct, ingredient_id__in=self.expect_delete_ids).exists()
            self.assertFalse(attrval_exists)
        for data in self.expect_delete_pkgs:
            data['nested']['tags'].clear()
        # -------------------------------------
        qset = ProductSaleablePackage.objects.get_deleted_set().filter(
                id__in=self.expect_delete_ids).order_by('id')
        qset.undelete(profile_id=profile_id)
        diff_created_ingredients(self, expect_data=self.expect_delete_pkgs, actual_data=qset,
                lower_elm_names=['sale_item','ingredient', None],
                lower_elm_mgr_fields=['saleitems_applied', 'ingredients_applied', None] )
## end of class SaleablePackageDeletionTestCase


class SaleableItemDeletionTestCase(TransactionTestCase):
    num_salepkgs = 3
    num_saleitems = 5
    num_ingredients = 6

    def setUp(self):
        self._expect_data, self._actual_data = _salepkg_instances_setup( num_salepkgs=self.num_salepkgs,
                num_saleitems=self.num_saleitems, num_ingredients=self.num_ingredients )
        num_delete = self.num_saleitems >> 1
        expect_delete_items = list(listitem_rand_assigner(list_=self._expect_data['ProductSaleableItem'] ,
            min_num_chosen=num_delete, max_num_chosen=(num_delete + 1)))
        key_fn = lambda d:d['obj'].id
        self.expect_delete_items = sorted(expect_delete_items, key=key_fn)
        self.expect_delete_ids = list(map(key_fn, self.expect_delete_items))

    def test_soft_delete(self):
        profile_id = 123
        qset = ProductSaleableItem.objects.filter(id__in=self.expect_delete_ids).order_by('id')
        qset.delete(profile_id=profile_id)
        item_exists   = ProductSaleableItem.objects.filter(id__in=self.expect_delete_ids).exists()
        compo_exists  = ProductSaleablePackageComposite.objects.filter(sale_item__id__in=self.expect_delete_ids).exists()
        compo2_exists = ProductSaleableItemComposite.objects.filter(sale_item__id__in=self.expect_delete_ids).exists()
        mm_exists     = ProductSaleableItemMedia.objects.filter(sale_item__id__in=self.expect_delete_ids).exists()
        self.assertFalse(item_exists)
        self.assertFalse(compo_exists)
        self.assertFalse(compo2_exists)
        self.assertFalse(mm_exists)
        for dtype_opt in _ProductAttrValueDataType:
            obj = qset.first()
            obj_ct = ContentType.objects.get_for_model(obj)
            model_cls = getattr(obj, dtype_opt.value[0][1]).model
            attrval_exists = model_cls.objects.filter(ingredient_type=obj_ct, ingredient_id__in=self.expect_delete_ids).exists()
            self.assertFalse(attrval_exists)
        for data in self.expect_delete_items:
            data['nested']['tags'].clear()
        qset = ProductSaleableItem.objects.get_deleted_set().filter(
                id__in=self.expect_delete_ids).order_by('id')
        qset.undelete(profile_id=profile_id)
        diff_created_ingredients(self, self._expect_data['ProductSaleablePackage'],
                 self._actual_data['ProductSaleablePackage'], lower_elm_names=['sale_item','ingredient', None],
                 lower_elm_mgr_fields=['saleitems_applied', 'ingredients_applied', None] )
## end of class SaleablePackageDeletionTestCase


