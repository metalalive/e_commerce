import string
import random
from unittest.mock import patch

from celery import states as CeleryStates
from django.conf import settings as django_settings
from django.test import TransactionTestCase

from ecommerce_common.models.enums.django import UnitOfMeasurement

from product.models.base import ProductSaleableItem, ProductSaleablePackage
from product.async_tasks  import get_product
from tests.common import  _common_instances_setup

class GetProductCase(TransactionTestCase):
    default_profile_id = 212

    def setUp(self):
        get_product.app.conf.task_always_eager = True
        self._data = {
            'ProductSaleableItem':[
                {'visible':True, 'unit':random.choice(UnitOfMeasurement.choices)[0],  'price': random.randrange(1, 40),
                    'name':''.join(random.choices(string.ascii_letters, k=14)), 'usrprof':self.default_profile_id}
                for _ in range(10)
            ],
            'ProductSaleablePackage':[
                {'visible':True, 'name':''.join(random.choices(string.ascii_letters, k=14)), 'usrprof':self.default_profile_id,
                    'price': random.randrange(10, 60) } for _ in range(10)
            ],
        }
        self._primitives = {}
        models_info = [
            (ProductSaleableItem,    len(self._data['ProductSaleableItem'])),
            (ProductSaleablePackage, len(self._data['ProductSaleablePackage']))
        ]
        _common_instances_setup(out=self._primitives, data=self._data, models_info=models_info)


    def tearDown(self):
        get_product.app.conf.task_always_eager = False

    def test_ok(self):
        chosen_items = random.choices(self._primitives['ProductSaleableItem'], k=3)
        chosen_pkgs  = random.choices(self._primitives['ProductSaleablePackage'], k=3)
        item_ids = list(map(lambda obj:obj.id, chosen_items))
        pkg_ids  = list(map(lambda obj:obj.id, chosen_pkgs ))
        item_fields = ['id','unit','price']
        pkg_fields  = ['id','name']
        input_kwargs = { 'item_ids':item_ids, 'pkg_ids':pkg_ids, 'item_fields':item_fields,
            'pkg_fields':pkg_fields, 'profile': self.default_profile_id }
        eager_result = get_product.apply_async(kwargs=input_kwargs)
        self.assertEqual(eager_result.state, CeleryStates.SUCCESS)
        expect_val_items = list(map(lambda obj:{k:getattr(obj,k) for k in item_fields} , chosen_items))
        expect_val_pkgs  = list(map(lambda obj:{k:getattr(obj,k) for k in pkg_fields } , chosen_pkgs ))
        actual_val_items = eager_result.result['item']
        actual_val_pkgs  = eager_result.result['pkg']
        expect_val_items = sorted(expect_val_items, key=lambda d:d['id'])
        expect_val_pkgs  = sorted(expect_val_pkgs , key=lambda d:d['id'])
        actual_val_items = sorted(actual_val_items, key=lambda d:d['id'])
        actual_val_pkgs  = sorted(actual_val_pkgs , key=lambda d:d['id'])
        self.assertListEqual(expect_val_items, actual_val_items)
        self.assertListEqual(expect_val_pkgs , actual_val_pkgs )


    def test_skip_invisible_item(self):
        chosen_items = random.choices(self._primitives['ProductSaleableItem'], k=3)
        chosen_pkgs  = random.choices(self._primitives['ProductSaleablePackage'], k=3)
        for obj in (chosen_items + chosen_pkgs):
            setattr(obj, 'visible', False)
            obj.save(update_fields=['visible'])
        item_ids = list(map(lambda obj:obj.id, chosen_items))
        pkg_ids  = list(map(lambda obj:obj.id, chosen_pkgs ))
        fields_present = ['id','name']
        input_kwargs = { 'item_ids':item_ids, 'pkg_ids':pkg_ids, 'item_fields':fields_present,
                'pkg_fields':fields_present, 'profile': self.default_profile_id }
        eager_result = get_product.apply_async(kwargs=input_kwargs)
        self.assertEqual(eager_result.state, CeleryStates.SUCCESS)
        actual_val_items = eager_result.result['item']
        actual_val_pkgs  = eager_result.result['pkg']
        self.assertFalse(any(actual_val_items))
        self.assertFalse(any(actual_val_pkgs))


    def test_different_users(self):
        new_profile_id = self.default_profile_id + 1
        chosen_items = random.choices(self._primitives['ProductSaleableItem'], k=3)
        chosen_pkgs  = random.choices(self._primitives['ProductSaleablePackage'], k=3)
        for obj in (chosen_items + chosen_pkgs):
            setattr(obj, 'usrprof', new_profile_id)
            obj.save(update_fields=['usrprof'])
        item_ids = list(map(lambda obj:obj.id, self._primitives['ProductSaleableItem']   ))
        pkg_ids  = list(map(lambda obj:obj.id, self._primitives['ProductSaleablePackage']))
        fields_present = ['id','name']
        input_kwargs = { 'item_ids':item_ids, 'pkg_ids':pkg_ids, 'item_fields':fields_present,
                'pkg_fields':fields_present, 'profile': self.default_profile_id }
        eager_result = get_product.apply_async(kwargs=input_kwargs)
        self.assertEqual(eager_result.state, CeleryStates.SUCCESS)
        fetched_items  = set(map(lambda d:d['id'], eager_result.result['item']))
        fetched_pkgs   = set(map(lambda d:d['id'], eager_result.result['pkg']))
        excluded_items = set(map(lambda obj:obj.id, chosen_items))
        excluded_pkgs  = set(map(lambda obj:obj.id, chosen_pkgs))
        self.assertFalse(any(fetched_items & excluded_items))
        self.assertFalse(any(fetched_pkgs  & excluded_pkgs ))


