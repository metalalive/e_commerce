import random
import copy
import json
from functools import partial, reduce

from django.test import TransactionTestCase
from django.db.models import Q

from product.models.base import ProductTag, ProductTagClosure

from product.tests.common import _fixtures, _null_test_obj_attrs, _common_instances_setup, _modelobj_list_to_map, _product_tag_closure_setup

class TagCreationTestCase(TransactionTestCase):
    def setUp(self):
        pass

    def test_null_obj_fields(self):
        data = _fixtures['ProductTag'][0]
        self.instance = ProductTag(**data)
        field_names = ['name', 'usrprof',]
        _null_test_obj_attrs(testcase=self, instance=self.instance, field_names=field_names)
        expect_node_id = 1
        closure_node = ProductTagClosure(id=expect_node_id, depth=0)
        err_caught = None
        with self.assertRaises(ValueError) as e:
            try:
                closure_node.save()
            except ValueError as e:
                err_caught = e
                raise
        self.assertNotEqual(err_caught, None)
        expect_err_msg = 'Null closure node not allowed'
        actual_err_msg = ''.join(err_caught.args)
        self.assertEqual(expect_err_msg, actual_err_msg)
        closure_node.save(accept_null_node=True)
        closure_node.refresh_from_db()
        self.assertEqual(closure_node.id, expect_node_id)
        self.assertEqual(closure_node.ancestor  , None)
        self.assertEqual(closure_node.descendant, None)

    def test_bulk_ok(self):
        num_tags = 5
        data = _fixtures['ProductTag'][:num_tags]
        tags = [ProductTag(**d) for d in data]
        ProductTag.objects.bulk_create(tags)
        closure_nodes = [ProductTagClosure(depth=0, ancestor=tag, descendant=tag) for tag in tags]
        ProductTagClosure.objects.bulk_create(closure_nodes)
        expect_tag_ids = list(map(lambda d:d['id'], data))
        actual_tag_ids = list(ProductTag.objects.values_list('id', flat=True))
        self.assertSetEqual(set(expect_tag_ids), set(actual_tag_ids))
        actual_closure_ascs = list(ProductTagClosure.objects.values_list('ancestor__id', flat=True))
        self.assertSetEqual(set(expect_tag_ids), set(actual_closure_ascs))


class TagDeletionTestCase(TransactionTestCase):
    instances = {'ProductTag':None , 'ProductTagClosure':None}

    def setUp(self):
        num_tags = len(_fixtures['ProductTag'])
        models_info = [(ProductTag, num_tags),]
        _common_instances_setup(out=self.instances, models_info=models_info)
        tag_map = _modelobj_list_to_map(self.instances['ProductTag'])
        self.instances['ProductTagClosure'] = _product_tag_closure_setup(
                tag_map=tag_map, data=_fixtures['ProductTagClosure'])

    def tearDown(self):
        self.instances['ProductTag'].clear()
        self.instances['ProductTagClosure'].clear()

    def test_hard_delete_one(self):
        del_id = 33
        expect_descs = [33, 36, 37]
        condition = Q(ancestor__id__in=expect_descs) | Q(descendant__id__in=expect_descs)
        del_node = ProductTag.objects.get(id=del_id)
        actual_descs = del_node.descendants.values_list('descendant', flat=True)
        self.assertSetEqual(set(expect_descs), set(actual_descs))
        num_all_nodes = ProductTagClosure.objects.count()
        num_del_nodes = ProductTagClosure.objects.filter(condition).count()
        self.assertGreater(num_del_nodes, 0)
        del_node.delete() # all its descendants are deleted automatically
        qset_after_delete = ProductTag.objects.filter(id__in=expect_descs)
        self.assertFalse(qset_after_delete.exists())
        qset_after_delete = ProductTagClosure.objects.filter(condition)
        self.assertFalse(qset_after_delete.exists())
        self.assertEqual((num_all_nodes - num_del_nodes), ProductTagClosure.objects.count())


    def test_hard_delete_bulk(self):
        expect_descs = {38:[38,40,41,42,49,50,51], 39:[39,43,44,45,46]}
        del_ids = list(expect_descs.keys())
        del_nodes = ProductTag.objects.filter(id__in=del_ids)
        expect_del_descs = []
        for v in expect_descs.values():
            expect_del_descs.extend(v)
        condition = Q(ancestor__id__in=expect_del_descs) | Q(descendant__id__in=expect_del_descs)
        actual_descs = del_nodes.values_list('descendants__descendant', flat=True)
        self.assertSetEqual(set(expect_del_descs), set(actual_descs))
        num_all_nodes = ProductTagClosure.objects.count()
        num_del_nodes = ProductTagClosure.objects.filter(condition).count()
        self.assertGreater(num_del_nodes, 0)
        del_nodes.delete() # all the descendants of the chosen nodes are deleted automatically
        qset_after_delete = ProductTag.objects.filter(id__in=expect_del_descs)
        self.assertFalse(qset_after_delete.exists())
        qset_after_delete = ProductTagClosure.objects.filter(condition)
        self.assertFalse(qset_after_delete.exists())
        self.assertEqual((num_all_nodes - num_del_nodes), ProductTagClosure.objects.count())
## end of class TagDeletionTestCase


class TagQueryTestCase(TransactionTestCase):
    instances = {'ProductTag':None , 'ProductTagClosure':None}

    def setUp(self):
        num_tags = len(_fixtures['ProductTag'])
        models_info = [(ProductTag, num_tags),]
        _common_instances_setup(out=self.instances, models_info=models_info)
        tag_map = _modelobj_list_to_map(self.instances['ProductTag'])
        self.instances['ProductTagClosure'] = _product_tag_closure_setup(
                tag_map=tag_map, data=_fixtures['ProductTagClosure'])

    def tearDown(self):
        self.instances['ProductTag'].clear()
        self.instances['ProductTagClosure'].clear()

    def test_input_error(self):
        init_qset = ProductTag.objects.all()
        with self.assertRaises(AssertionError):
            init_qset.get_ascs_descs_id(IDs=[], fetch_asc=False, fetch_desc=False)
        expect_fetched_ids = [30, 31] # all roots of the trees
        invalid_depth_list = ['-1.0', '0.3', 0.3, '0.5', '0.99', 0.99, '1.01', 1.01, 'xyz', 1.0, 2.0, 99.0, '99.0']
        for invalid_depth in invalid_depth_list:
            actual_fetched_ids = init_qset.get_ascs_descs_id(IDs=['root'], fetch_asc=False,
                    fetch_desc=True, depth=invalid_depth)
            self.assertSetEqual(set(expect_fetched_ids), set(actual_fetched_ids))
        expect_fetched_ids = [30, 31, 32,33,35, 34,38,39]
        valid_depth_list = ['1', 1]
        for valid_depth in valid_depth_list:
            actual_fetched_ids = init_qset.get_ascs_descs_id(IDs=['root'], fetch_asc=False,
                    fetch_desc=True, depth=valid_depth)
            self.assertSetEqual(set(expect_fetched_ids), set(actual_fetched_ids))


    def test_fetch_root_ascs(self):
        tag_ids = [40, 'root', 37]
        expect_fetched_ids = [30, 31] # all roots of the trees
        init_qset = ProductTag.objects.all()
        actual_fetched_ids = init_qset.get_ascs_descs_id(IDs=tag_ids, fetch_asc=True,
                fetch_desc=False, self_exclude=False)
        self.assertSetEqual(set(expect_fetched_ids), set(actual_fetched_ids))
        actual_fetched_ids = init_qset.get_ascs_descs_id(IDs=tag_ids, fetch_asc=True,
                fetch_desc=False, depth=9999, self_exclude=False)
        self.assertSetEqual(set(expect_fetched_ids), set(actual_fetched_ids))
        actual_fetched_ids = init_qset.get_ascs_descs_id(IDs=tag_ids, fetch_asc=True,
                fetch_desc=False, self_exclude=True)
        self.assertSetEqual(set(expect_fetched_ids), set(actual_fetched_ids))
        # fetch tag id which is root of a tree
        tag_ids = [31]
        expect_fetched_ids = [31] # all roots of the trees
        actual_fetched_ids = init_qset.get_ascs_descs_id(IDs=tag_ids, fetch_asc=True,
                fetch_desc=False, depth=None, self_exclude=False)
        self.assertSetEqual(set(expect_fetched_ids), set(actual_fetched_ids))
        for idx in range(1, 5):
            actual_fetched_ids = init_qset.get_ascs_descs_id(IDs=tag_ids, fetch_asc=True,
                    fetch_desc=False, depth=idx, self_exclude=False)
            self.assertSetEqual(set(expect_fetched_ids), set(actual_fetched_ids))

    def test_fetch_root_descs(self):
        expect_fetched_id_map = {
                30: [(32,33,35), (36,37,47,48)],
                31: [(34,38,39), (43,44,45,46,40,49), (41,42,50,51)],
            }
        tag_ids = [40, 'root', 37]
        init_qset = ProductTag.objects.all()
        for idx in range(0, 3):
            expect_fetched_ids = list(expect_fetched_id_map.keys())
            for jdx in range(idx):
                added_ids = [n for v in expect_fetched_id_map.values() for n in v[jdx]]
                expect_fetched_ids.extend(added_ids)
            actual_fetched_ids = init_qset.get_ascs_descs_id(IDs=tag_ids, fetch_asc=False,
                fetch_desc=True, depth=idx, self_exclude=False)
            self.assertSetEqual(set(expect_fetched_ids), set(actual_fetched_ids))
        # the `depth` augment indicates the number of levels loaded from the given tag and
        tag_id = 31
        for idx in range(0, len(expect_fetched_id_map[tag_id]) + 1):
            expect_fetched_ids = [tag_id]
            for jdx in range(idx):
                added_ids = [n for n in expect_fetched_id_map[tag_id][jdx]]
                expect_fetched_ids.extend(added_ids)
            actual_fetched_ids = init_qset.get_ascs_descs_id(IDs=[tag_id], fetch_asc=False,
                fetch_desc=True, depth=idx, self_exclude=False)
            self.assertSetEqual(set(expect_fetched_ids), set(actual_fetched_ids))
            actual_fetched_ids = init_qset.get_ascs_descs_id(IDs=[tag_id], fetch_asc=False,
                fetch_desc=True, depth=idx, self_exclude=True)
            self.assertSetEqual(set(expect_fetched_ids) - set([tag_id]), set(actual_fetched_ids))

    def test_fetch_non_root_ascs(self):
        tag_id = 40
        expect_fetched_ids = [tag_id, 38, 31]
        init_qset = ProductTag.objects.all()
        for idx in range(0, len(expect_fetched_ids)):
            actual_fetched_ids = init_qset.get_ascs_descs_id(IDs=[tag_id], fetch_asc=True,
                fetch_desc=False, depth=idx, self_exclude=False)
            self.assertSetEqual(set(expect_fetched_ids[:idx+1]), set(actual_fetched_ids))
        expect_fetched_ids.remove(tag_id)
        for idx in range(0, len(expect_fetched_ids)):
            actual_fetched_ids = init_qset.get_ascs_descs_id(IDs=[tag_id], fetch_asc=True,
                fetch_desc=False, depth=idx, self_exclude=True)
            self.assertSetEqual(set(expect_fetched_ids[:idx]), set(actual_fetched_ids))


    def test_fetch_non_root_descs(self):
        tag_id = 38
        expect_fetched_ids = [(40,49), (41,42,50,51)]
        init_qset = ProductTag.objects.all()
        for idx in range(len(expect_fetched_ids) + 1):
            flat_expect_fetched_ids = [tag_id]
            for jdx in range(idx):
                added_ids = [n for n in expect_fetched_ids[jdx]]
                flat_expect_fetched_ids.extend(added_ids)
            actual_fetched_ids = init_qset.get_ascs_descs_id(IDs=[tag_id], fetch_asc=False,
                fetch_desc=True, depth=idx, self_exclude=False)
            self.assertSetEqual(set(flat_expect_fetched_ids), set(actual_fetched_ids))
            flat_expect_fetched_ids.remove(tag_id)
            actual_fetched_ids = init_qset.get_ascs_descs_id(IDs=[tag_id], fetch_asc=False,
                fetch_desc=True, depth=idx, self_exclude=True)
            self.assertSetEqual(set(flat_expect_fetched_ids), set(actual_fetched_ids))

    def test_fetch_non_root_ascs_and_descs(self):
        tag_id = 38
        expect_fetched_ids = [(38,), (31,38,40,49,), (31,38,40,49, 41,42,50,51)]
        init_qset = ProductTag.objects.all()
        for idx in range(len(expect_fetched_ids)):
            actual_fetched_ids = init_qset.get_ascs_descs_id(IDs=[tag_id], fetch_asc=True,
                fetch_desc=True, depth=idx, self_exclude=False)
            self.assertSetEqual(set(expect_fetched_ids[idx]), set(actual_fetched_ids))

