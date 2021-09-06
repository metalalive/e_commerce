import random
import copy
import json
from functools import partial

from django.test import TransactionTestCase
from rest_framework.exceptions import ValidationError as DRFValidationError
from rest_framework.settings import DEFAULTS as drf_default_settings

from common.util.python import sort_nested_object
from product.tests.common import _fixtures
from .common import TreeNodeMixin, HttpRequestDataGenTag, TagVerificationMixin

class TagCommonMixin(HttpRequestDataGenTag, TagVerificationMixin):
    def setUp(self):
        pass

    def tearDown(self):
        pass

    def _gen_tag_name(self):
        num_valid_tags = len(_fixtures['ProductTag'])
        idx = random.randrange(0, num_valid_tags)
        return  _fixtures['ProductTag'][idx]['name']


class TagCreationTestCase(TagCommonMixin, TransactionTestCase):
    def setUp(self):
        super().setUp()

    def tearDown(self):
        super().tearDown()

    def test_create_new_trees(self):
        for _ in range(20):
            self._init_new_trees()

    def _init_new_trees(self, num_trees=3, min_num_nodes=2, max_num_nodes=15):
        usrprof_id = 123
        def write_value_fn(node):
            out = {'name': self._gen_tag_name()}
            node.value = out
        def value_compare_fn(val_a, val_b):
            return val_a['name'] == val_b['name']
        origin_trees = TreeNodeMixin.rand_gen_trees(num_trees=num_trees, min_num_nodes=min_num_nodes,
                max_num_nodes=max_num_nodes, write_value_fn=write_value_fn)
        req_data = self.trees_to_req_data(trees=origin_trees)
        serializer = self.serializer_class(many=True, data=req_data, usrprof_id=usrprof_id)
        serializer.is_valid(raise_exception=True)
        actual_instances = serializer.save()
        entity_data, closure_data = self.load_closure_data(actual_instances) # serializer.data
        saved_trees = TreeNodeMixin.gen_from_closure_data(entity_data=entity_data, closure_data=closure_data)
        matched, not_matched = TreeNodeMixin.compare_trees(trees_a=origin_trees, trees_b=saved_trees,
                value_compare_fn=value_compare_fn)
        #if any(not_matched):
        #    import pdb
        #    pdb.set_trace()
        self.assertListEqual(not_matched, [])
        self.assertEqual(len(matched), len(origin_trees))
        return saved_trees


    def test_append_new_trees_to_existing_nodes(self):
        usrprof_id = 123
        def write_value_fn(node):
            out = {'name': self._gen_tag_name()}
            node.value = out
        def value_compare_fn(val_a, val_b):
            return val_a['name'] == val_b['name']
        saved_trees = self._init_new_trees(num_trees=4, max_num_nodes=7)
        appending_trees = TreeNodeMixin.rand_gen_trees(num_trees=3, min_num_nodes=7,
                max_num_nodes=10, write_value_fn=write_value_fn)
        req_data = self.trees_to_req_data(trees=appending_trees)


    def test_loop_detection(self):
        pass

    def test_invalid_input(self):
        pass


