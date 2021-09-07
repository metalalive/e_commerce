import random
import copy
import json
from functools import partial

from django.test import TransactionTestCase
from rest_framework.exceptions import ValidationError as DRFValidationError
from rest_framework.settings import DEFAULTS as drf_default_settings

from common.util.python import sort_nested_object
from product.tests.common import _fixtures, http_request_body_template
from .common import TreeNodeMixin, HttpRequestDataGenTag, TagVerificationMixin

class TagCommonMixin(HttpRequestDataGenTag, TagVerificationMixin):
    err_msg_loop_detected = 'will form a loop, which is NOT allowed in closure table'

    def setUp(self):
        pass

    def tearDown(self):
        pass

    def _gen_tag_name(self):
        num_valid_tags = len(_fixtures['ProductTag'])
        idx = random.randrange(0, num_valid_tags)
        return  _fixtures['ProductTag'][idx]['name']


class TagCreationTestCase(TagCommonMixin, TransactionTestCase):
    usrprof_id = 123

    def setUp(self):
        super().setUp()

    def tearDown(self):
        super().tearDown()

    def test_create_new_trees(self):
        num_rounds = 20
        for _ in range(num_rounds):
            self._init_new_trees()

    def _init_new_trees(self, num_trees=3, min_num_nodes=2, max_num_nodes=15):
        origin_trees = TreeNodeMixin.rand_gen_trees(num_trees=num_trees, min_num_nodes=min_num_nodes,
                max_num_nodes=max_num_nodes, write_value_fn=self._write_value_fn)
        req_data = self.trees_to_req_data(trees=origin_trees)
        serializer = self.serializer_class(many=True, data=req_data, usrprof_id=self.usrprof_id)
        serializer.is_valid(raise_exception=True)
        actual_instances = serializer.save()
        obj_ids = tuple(map(lambda obj: obj.pk, actual_instances))
        entity_data, closure_data = self.load_closure_data(node_ids=obj_ids) # serializer.data
        saved_trees = TreeNodeMixin.gen_from_closure_data(entity_data=entity_data, closure_data=closure_data)
        matched, not_matched = TreeNodeMixin.compare_trees(trees_a=origin_trees, trees_b=saved_trees,
                value_compare_fn=self._value_compare_fn)
        #if any(not_matched):
        #    import pdb
        #    pdb.set_trace()
        self.assertListEqual(not_matched, [])
        self.assertEqual(len(matched), len(origin_trees))
        return saved_trees

    def _write_value_fn(self, node):
        out = {'name': self._gen_tag_name()}
        node.value = out

    def _value_compare_fn(self, val_a, val_b):
        return val_a['name'] == val_b['name']

    def _append_new_trees_to_existing_nodes(self, existing_trees, appending_trees):
        appending_trees_iter = iter(appending_trees)
        for existing_tree in existing_trees:
            try:
                appending_tree = next(appending_trees_iter)
                exist_parent = existing_tree
                while any(exist_parent.children):
                    exist_parent = exist_parent.children[0]
                appending_tree.value['exist_parent'] = exist_parent.value['id']
                appending_tree.parent = exist_parent
                exist_parent.children.append(appending_tree)
            except StopIteration as e:
                break
        out = existing_trees.copy() # shallow copy should be enough
        out.extend([t for t in appending_trees_iter])
        return out


    def test_append_new_trees_to_existing_nodes(self):
        num_rounds = 8
        num_trees = 3
        existing_trees = self._init_new_trees(num_trees=num_trees, max_num_nodes=7)
        for _ in range(num_rounds):
            appending_trees = TreeNodeMixin.rand_gen_trees(num_trees=random.randrange(2, num_trees + 2),
                    min_num_nodes=7, max_num_nodes=10, write_value_fn=self._write_value_fn)
            trees_before_save = self._append_new_trees_to_existing_nodes(
                    existing_trees=existing_trees, appending_trees=appending_trees)
            req_data = self.trees_to_req_data(trees=appending_trees)
            serializer = self.serializer_class(many=True, data=req_data, usrprof_id=self.usrprof_id)
            serializer.is_valid(raise_exception=True)
            actual_instances = serializer.save()
            obj_ids = tuple(map(lambda obj: obj.pk, actual_instances))
            obj_ids = obj_ids + tuple(map(lambda d: d['id'], existing_trees.entity_data))
            entity_data, closure_data = self.load_closure_data(node_ids=obj_ids) # serializer.data
            trees_after_save = TreeNodeMixin.gen_from_closure_data(entity_data=entity_data, closure_data=closure_data)
            matched, not_matched = TreeNodeMixin.compare_trees(trees_a=trees_before_save, trees_b=trees_after_save,
                    value_compare_fn=self._value_compare_fn)
            ##if any(not_matched):
            ##    import pdb
            ##    pdb.set_trace()
            self.assertListEqual(not_matched, [])
            ##if len(matched) != len(trees_before_save):
            ##    import pdb
            ##    pdb.set_trace()
            self.assertEqual(len(matched), len(trees_before_save))
            existing_trees = trees_after_save
    ## end of test_append_new_trees_to_existing_nodes()


    def test_loop_detection_simple_tree(self):
        num_nodes = 4
        origin_tree_nodes = [TreeNodeMixin(value={'name': 'tag %s' % (idx),}) \
                for idx in range(num_nodes)]
        for idx in range(num_nodes - 1):
            origin_tree_nodes[idx].children.append( origin_tree_nodes[idx+1] )
            origin_tree_nodes[idx+1].parent = origin_tree_nodes[idx]
        origin_trees = [origin_tree_nodes[0]]
        req_data = self.trees_to_req_data(trees=origin_trees)
        # make a loop
        origin_tree_nodes[-1].children.append( origin_tree_nodes[0] )
        origin_tree_nodes[0].parent = origin_tree_nodes[-1]
        req_data[0]['new_parent'] = num_nodes - 1
        error_caught = None
        serializer = self.serializer_class(many=True, data=req_data, usrprof_id=self.usrprof_id)
        with self.assertRaises(DRFValidationError):
            try:
                serializer.is_valid(raise_exception=True)
            except DRFValidationError as e:
                error_caught = e
                raise
        self.assertNotEqual(error_caught, None)
        non_field_err_key = drf_default_settings['NON_FIELD_ERRORS_KEY']
        err_info = error_caught.detail[non_field_err_key]
        loop_detect_msg_pos = err_info[0].find(self.err_msg_loop_detected)
        self.assertGreater(loop_detect_msg_pos, 0)
    ## end of test_loop_detection_simple_tree()


    def test_loop_detection_rand_gen_trees(self):
        num_trees = 3
        appending_trees = TreeNodeMixin.rand_gen_trees(num_trees=num_trees,  min_num_nodes=15,
                max_num_nodes=25, max_num_siblings=2, write_value_fn=self._write_value_fn)
        req_data = self.trees_to_req_data(trees=appending_trees)
        non_root_data = map(lambda idx: {'idx': idx, 'data':req_data[idx]}, range(len(req_data)))
        non_root_data = list(filter(lambda d: d['data']['new_parent'] is not None, non_root_data))
        idx = random.randrange(0, len(non_root_data))
        loop_data_start = non_root_data[idx]
        curr_data = loop_data_start['data']
        ascs_data = []
        while curr_data['new_parent']:
            idx = curr_data['new_parent']
            asc_data = req_data[idx]
            ascs_data.append({'idx': idx, 'data':asc_data})
            curr_data = asc_data
        if len(ascs_data) > 1:
            shuffled = ascs_data.copy()
            random.shuffle(shuffled)
            loop_data_end = shuffled[0]
        else:
            loop_data_end = ascs_data[0]
        origin_new_parent = loop_data_end['data']['new_parent']
        loop_data_end['data']['new_parent'] = loop_data_start['idx']
        error_caught = None
        serializer = self.serializer_class(many=True, data=req_data, usrprof_id=self.usrprof_id)
        with self.assertRaises(DRFValidationError):
            try:
                serializer.is_valid(raise_exception=True)
            except DRFValidationError as e:
                error_caught = e
                raise
        self.assertNotEqual(error_caught, None)
        non_field_err_key = drf_default_settings['NON_FIELD_ERRORS_KEY']
        err_info = error_caught.detail[non_field_err_key]
        pattern_pos = err_info[0].find(self.err_msg_loop_detected)
        self.assertGreater(pattern_pos, 0)
        form_label_pattern = 'form #%s'
        pattern_pos = err_info[0].find(form_label_pattern % loop_data_start['idx'])
        self.assertGreaterEqual(pattern_pos, 0)
        for asc_data in ascs_data:
            pattern_pos = err_info[0].find(form_label_pattern % asc_data['idx'])
            self.assertGreaterEqual(pattern_pos, 0)
    ## end of test_loop_detection_rand_gen_trees()


    def test_loop_detection_self_ref_node(self):
        num_nodes = 3
        req_data_template = http_request_body_template['ProductTag']
        req_data = [req_data_template.copy() for _ in range(num_nodes)]
        for idx in range(num_nodes):
            req_data[idx]['new_parent'] = idx
            req_data[idx]['name'] = 'tag #%s' % idx
        req_data[0]['new_parent'] = None
        error_caught = None
        with self.assertRaises(ValueError):
            try:
                serializer = self.serializer_class(many=True, data=req_data, usrprof_id=self.usrprof_id)
            except ValueError as e:
                error_caught = e
                raise
        self.assertNotEqual(error_caught, None)
        actual_err_msg = ''.join(error_caught.args)
        expect_err_msg = 'self-directed edge at (1,1) is NOT allowed'
        self.assertEqual(actual_err_msg, expect_err_msg)

## end of class TagCreationTestCase


