import random
import copy
import json
from functools import partial
from unittest.mock import Mock

from django.test import TransactionTestCase
from rest_framework.exceptions import ValidationError as DRFValidationError
from rest_framework.settings import DEFAULTS as drf_default_settings

from common.util.python import ExtendedDict, sort_nested_object
from product.tests.common import _fixtures, http_request_body_template
from .common import TreeNodeMixin, HttpRequestDataGenTag, TagVerificationMixin

class TagCommonMixin(HttpRequestDataGenTag, TagVerificationMixin):
    usrprof_id = 123

    def _init_new_trees(self, num_trees=3, min_num_nodes=2, max_num_nodes=15, min_num_siblings=1,
            max_num_siblings=4, write_value_fn=None, value_compare_fn=None):
        write_value_fn = write_value_fn or self._write_value_fn
        value_compare_fn = value_compare_fn or self._value_compare_fn
        origin_trees = TreeNodeMixin.rand_gen_trees(
                num_trees=num_trees, min_num_nodes=min_num_nodes,
                max_num_nodes=max_num_nodes, min_num_siblings=min_num_siblings,
                max_num_siblings=max_num_siblings, write_value_fn=write_value_fn)
        req_data = self.trees_to_req_data(trees=origin_trees)
        serializer = self.serializer_class(many=True, data=req_data, usrprof_id=self.usrprof_id)
        serializer.is_valid(raise_exception=True)
        actual_instances = serializer.save()
        obj_ids = tuple(map(lambda obj: obj.pk, actual_instances))
        entity_data, closure_data = self.load_closure_data(node_ids=obj_ids) # serializer.data
        saved_trees = TreeNodeMixin.gen_from_closure_data(entity_data=entity_data, closure_data=closure_data)
        matched, not_matched = TreeNodeMixin.compare_trees(trees_a=origin_trees, trees_b=saved_trees,
                value_compare_fn=value_compare_fn)
        #if any(not_matched):
        #    import pdb
        #    pdb.set_trace()
        self.assertListEqual(not_matched, [])
        self.assertEqual(len(matched), len(origin_trees))
        return saved_trees
## end of class TagCommonMixin


class TagCreationTestCase(TransactionTestCase, TagCommonMixin):
    def setUp(self):
        pass

    def tearDown(self):
        pass

    def test_create_new_trees(self):
        num_rounds = 20
        for _ in range(num_rounds):
            self._init_new_trees()

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
            origin_tree_nodes[idx+1].parent = origin_tree_nodes[idx]
        origin_trees = [origin_tree_nodes[0]]
        req_data = self.trees_to_req_data(trees=origin_trees)
        # make a loop
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
            if asc_data is loop_data_end:
                break
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


class TagUpdateTestCase(TransactionTestCase, TagCommonMixin):
    def setUp(self):
        self.num_trees = 3
        self.existing_trees = self._init_new_trees(num_trees=self.num_trees, min_num_nodes=30,
                max_num_nodes=40, min_num_siblings=2, max_num_siblings=4,)

    def tearDown(self):
        pass

    def _update_and_validate_tree(self, moving_nodes):
        req_data = list(map(lambda node: {'id':node.value['id'], 'name':node.value['name'], \
                    'exist_parent': node.parent.value['id'] if node.parent else None, \
                    'new_parent':None}, moving_nodes))
        random.shuffle(req_data) # `id` field should be unique value in each data item
        tag_ids = list(map(lambda node:node.value['id'] ,moving_nodes))
        tag_objs = self.serializer_class.Meta.model.objects.filter(id__in=tag_ids)
        serializer = self.serializer_class(many=True, data=req_data, instance=tag_objs,
                usrprof_id=self.usrprof_id)
        serializer.is_valid(raise_exception=True)
        actual_instances = serializer.save()
        obj_ids = tuple(map(lambda d: d['id'], self.existing_trees.entity_data))
        entity_data, closure_data = self.load_closure_data(node_ids=obj_ids)
        trees_after_save = TreeNodeMixin.gen_from_closure_data(entity_data=entity_data, closure_data=closure_data)
        def _value_compare_fn(val_a, val_b):
            return val_a['name'] == val_b['name'] and val_a['id'] == val_b['id']
        matched, not_matched = TreeNodeMixin.compare_trees(trees_a=self.existing_trees,
                trees_b=trees_after_save,  value_compare_fn=_value_compare_fn)
        #if any(not_matched):
        #    import pdb
        #    pdb.set_trace()
        self.assertListEqual(not_matched, [])
        self.assertEqual(len(matched), len(self.existing_trees))


    def test_subtrees_move_out(self):
        moving_nodes = []
        for idx in range(self.num_trees):
            parent_from = self.existing_trees[idx]
            parent_to   = self.existing_trees[(idx+1)%self.num_trees]
            moving_node = parent_from.children[0]
            moving_node.parent = parent_to
            moving_nodes.append(moving_node)
            # extract subtree from current moving node
            parent_from = moving_node
            parent_to   = self.existing_trees[idx]
            moving_node = parent_from.children[0]
            moving_node.parent = parent_to
            moving_nodes.append(moving_node)
        self._update_and_validate_tree(moving_nodes)
    ## end of test_nested_subtrees_out()


    def test_subtrees_move_in(self):
        moving_nodes = []
        for idx in range(self.num_trees):
            curr_node = self.existing_trees[idx]
            while any(curr_node.children):
                curr_node = curr_node.children[0]
            moving_node = curr_node.parent
            parent_to   = self.existing_trees[(idx+1)%self.num_trees]
            moving_node.parent = parent_to
            moving_nodes.append(moving_node)
            # extract subtree from current moving node
            curr_node = self.existing_trees[(idx+1)%self.num_trees]
            while any(curr_node.children):
                curr_node = curr_node.children[0]
            parent_to   = moving_node.children[0]
            moving_node = curr_node.parent
            moving_node.parent = parent_to
            moving_nodes.append(moving_node)
        self._update_and_validate_tree(moving_nodes)


    def test_subtrees_move_internal(self):
        moving_nodes = []
        for root in self.existing_trees:
            parent_to   = root.children[0]
            moving_node = root.children[-1]
            moving_node.parent = parent_to
            moving_nodes.append(moving_node)
            parent_to   = moving_node.children[-1]
            moving_node = moving_node.children[0]
            moving_node.parent = parent_to
            moving_nodes.append(moving_node)
        self._update_and_validate_tree(moving_nodes)


    def test_subtrees_move_mix(self):
        self.assertGreaterEqual(self.num_trees, 3) # this test case requires at least 3 existing trees
        moving_nodes = []
        roots = self.existing_trees
        #-------------------------------
        parent_to   = roots[0].children[-1]
        moving_node = roots[2].children[0]
        moving_node.parent = parent_to
        moving_nodes.append(moving_node)
        #-------------------------------
        moving_node_bak = moving_node
        curr_node = moving_node_bak
        while any(curr_node.children):
            sorted_children = sorted(curr_node.children, key=lambda node:node.num_nodes)
            curr_node = sorted_children[-1]
        parent_to = curr_node
        curr_node = roots[1]
        while any(curr_node.children):
            curr_node = curr_node.children[0]
        moving_node = curr_node.parent
        moving_node.parent = parent_to
        moving_nodes.append(moving_node)
        #-------------------------------
        parent_to = roots[2]
        curr_node = moving_node_bak
        while any(curr_node.children):
            curr_node = curr_node.children[0]
        moving_node = curr_node.parent
        moving_node.parent = parent_to
        moving_nodes.append(moving_node)
        #-------------------------------
        parent_to = moving_node_bak
        curr_node = moving_node_bak
        while any(curr_node.children):
            sorted_children = sorted(curr_node.children, key=lambda node:node.depth)
            curr_node = sorted_children[-1]
        moving_node = curr_node.parent
        if moving_node is parent_to:
            #import pdb
            #pdb.set_trace()
            pass
        else:
            moving_node.parent = parent_to
            moving_nodes.append(moving_node)
        #-------------------------------
        self._update_and_validate_tree(moving_nodes)
    ## end of test_nested_subtrees_mix()


    def test_tree_chains(self):
        self.assertGreaterEqual(self.num_trees, 2) # this test case requires at least 2 existing trees
        moving_nodes = []
        moving_node  = None
        roots = self.existing_trees
        curr_node = roots[0]
        while any(curr_node.children):
            curr_node = curr_node.children[-1]
        parent_to = curr_node
        while moving_node is not roots[1]:
            # gradually extract subtrees of roots[1] and append it to roots[0]
            curr_node = roots[1]
            while any(curr_node.children):
                curr_node = curr_node.children[0]
            moving_node = curr_node.parent
            moving_node.parent = parent_to
            moving_nodes.append(moving_node)
            parent_to = moving_node.children[0]
        #-------------------------------
        self.existing_trees.remove(roots[1])
        self._update_and_validate_tree(moving_nodes)


    def test_all_merge_one(self):
        self.assertGreaterEqual(self.num_trees, 3) # this test case requires at least 3 existing trees
        moving_nodes = []
        roots = self.existing_trees
        #-------------------------------
        for root in roots[1:] :
            moving_node  = None
            while moving_node is not root:
                curr_node = roots[0]
                while any(curr_node.children):
                    rand_idx = random.randrange(0, len(curr_node.children))
                    curr_node = curr_node.children[rand_idx]
                parent_to = curr_node
                # gradually extract subtrees of roots[1] and append it to roots[0]
                curr_node = root
                while any(curr_node.children):
                    sorted_children = sorted(curr_node.children, key=lambda node:node.depth)
                    curr_node = sorted_children[-1]
                moving_node = curr_node.parent
                moving_node.parent = parent_to
                moving_nodes.append(moving_node)
                parent_to = moving_node.children[0]
        #-------------------------------
        self.existing_trees.remove(roots[2])
        self.existing_trees.remove(roots[1])
        self._update_and_validate_tree(moving_nodes)


    def test_subtrees_to_new_root(self):
        self.assertGreaterEqual(self.num_trees, 3) # this test case requires at least 3 existing trees
        extracted_subtrees = []
        moving_nodes = []
        roots = self.existing_trees
        #-------------------------------
        for root in roots:
            curr_node = root
            while any(curr_node.children):
                sorted_children = sorted(curr_node.children, key=lambda node:node.depth)
                curr_node = sorted_children[-1]
            extracted_subtrees.append(curr_node.parent)
        #-------------------------------
        for subtree in extracted_subtrees[1:] :
            curr_node = extracted_subtrees[0]
            while any(curr_node.children):
                rand_idx = random.randrange(0, len(curr_node.children))
                curr_node = curr_node.children[rand_idx]
            parent_to = curr_node
            moving_node = subtree
            moving_node.parent = parent_to
            moving_nodes.append(moving_node)
        extracted_subtrees[0].parent = None
        moving_nodes.append(extracted_subtrees[0])
        self.existing_trees.append(extracted_subtrees[0])
        self._update_and_validate_tree(moving_nodes)


    def test_loop_detection_in_one_tree(self):
        error_caught = None
        moving_nodes = []
        root = self.existing_trees[0]
        curr_node = root
        while any(curr_node.children):
            sorted_children = sorted(curr_node.children, key=lambda node:node.depth)
            curr_node = sorted_children[-1]
        root.parent = curr_node
        moving_nodes.append(root)
        with self.assertRaises(DRFValidationError):
            try:
                self._update_and_validate_tree(moving_nodes)
            except DRFValidationError as e:
                error_caught = e
                raise
        self.assertNotEqual(error_caught, None)
        non_field_err_key = drf_default_settings['NON_FIELD_ERRORS_KEY']
        err_info = error_caught.detail[non_field_err_key]
        pattern_pos = err_info[0].find(self.err_msg_loop_detected)
        self.assertGreater(pattern_pos, 0)
        # --------------------------------
        root.parent = None
        origin_grand_parent = curr_node.parent.parent
        curr_node.parent.parent = curr_node
        moving_nodes.clear()
        moving_nodes.append(curr_node.parent)
        with self.assertRaises(DRFValidationError):
            try:
                self._update_and_validate_tree(moving_nodes)
            except DRFValidationError as e:
                error_caught = e
                raise
        self.assertNotEqual(error_caught, None)
        non_field_err_key = drf_default_settings['NON_FIELD_ERRORS_KEY']
        err_info = error_caught.detail[non_field_err_key]
        pattern_pos = err_info[0].find(self.err_msg_loop_detected)
        self.assertGreater(pattern_pos, 0)


    def test_loop_detection_across_2_trees(self):
        self.assertGreaterEqual(self.num_trees, 2) # this test case requires at least 2 existing trees
        moving_nodes = []
        moving_node  = None
        chosen_subtrees = []
        roots = self.existing_trees[:2]
        for root in roots:
            curr_node = root
            while any(curr_node.children):
                sorted_children = sorted(curr_node.children, key=lambda node:node.depth)
                curr_node = sorted_children[-1]
            chosen_subtrees.append(curr_node.parent)
            moving_nodes.append(curr_node.parent)
        expect_loop_nodes = [ chosen_subtrees[1], chosen_subtrees[0],
                chosen_subtrees[1].children[0], chosen_subtrees[0].children[0]
            ]
        chosen_subtrees[0].parent, chosen_subtrees[1].parent = chosen_subtrees[1].children[0], chosen_subtrees[0].children[0]
        with self.assertRaises(DRFValidationError):
            try:
                self._update_and_validate_tree(moving_nodes)
            except DRFValidationError as e:
                error_caught = e
                raise
        self.assertNotEqual(error_caught, None)
        non_field_err_key = drf_default_settings['NON_FIELD_ERRORS_KEY']
        err_info = error_caught.detail[non_field_err_key]
        pattern_pos = err_info[0].find(self.err_msg_loop_detected)
        self.assertGreater(pattern_pos, 0)
        for loop_node in expect_loop_nodes:
            form_label_pattern = 'form #%s'
            pattern_pos = err_info[0].find(form_label_pattern % loop_node.value['id'])
            self.assertGreaterEqual(pattern_pos, 0)


    def test_loop_detection_across_3_trees(self):
        self.assertGreaterEqual(self.num_trees, 3) # this test case requires at least 3 existing trees
        moving_nodes = []
        moving_node  = None
        chosen_subtrees = []
        roots = self.existing_trees[:3]
        for root in roots:
            curr_node = root
            while any(curr_node.children):
                sorted_children = sorted(curr_node.children, key=lambda node:node.depth)
                curr_node = sorted_children[-1]
            chosen_subtrees.append(curr_node.parent)
            moving_nodes.append(curr_node.parent)
        expect_loop_nodes = [ chosen_subtrees[2], chosen_subtrees[1], chosen_subtrees[0],
                chosen_subtrees[2].children[0], chosen_subtrees[1].children[0],
                chosen_subtrees[0].children[0]
            ]
        chosen_subtrees[0].parent, chosen_subtrees[1].parent, chosen_subtrees[2].parent = \
                chosen_subtrees[1].children[0], chosen_subtrees[2].children[0], chosen_subtrees[0].children[0]
        with self.assertRaises(DRFValidationError):
            try:
                self._update_and_validate_tree(moving_nodes)
            except DRFValidationError as e:
                error_caught = e
                raise
        self.assertNotEqual(error_caught, None)
        non_field_err_key = drf_default_settings['NON_FIELD_ERRORS_KEY']
        err_info = error_caught.detail[non_field_err_key]
        pattern_pos = err_info[0].find(self.err_msg_loop_detected)
        self.assertGreater(pattern_pos, 0)
        for loop_node in expect_loop_nodes:
            form_label_pattern = 'form #%s'
            pattern_pos = err_info[0].find(form_label_pattern % loop_node.value['id'])
            self.assertGreaterEqual(pattern_pos, 0)
## end of class TagUpdateTestCase


class TagRepresentationTestCase(TransactionTestCase, TagCommonMixin):
    def setUp(self):
        self.num_trees = 3
        self.existing_trees = self._init_new_trees(num_trees=self.num_trees, min_num_nodes=20,
                max_num_nodes=40, min_num_siblings=2, max_num_siblings=3,)

    def tearDown(self):
        pass

    def test_all_descs_of_roots(self):
        nodes = self.existing_trees
        tag_ids = list(map(lambda node:node.value['id'], nodes))
        tag_objs = self.serializer_class.Meta.model.objects.filter(id__in=tag_ids)
        serializer = self.serializer_class(many=True, instance=tag_objs,
                usrprof_id=self.usrprof_id)
        actual_data = serializer.data
        trees_iter = iter(nodes)
        for data in actual_data:
            tree = next(trees_iter)
            self._assert_simple_fields(check_fields=['id','name'],  exp_sale_item=tree.value,
                    ac_sale_item=data)
            self.assertEqual(len(data['ancestors']), 0)
            expect_children = list(map(lambda d:d.value['id'], tree.children))
            actual_children = filter(lambda d: d['depth'] == 1, data['descendants'])
            actual_children = list(map(lambda d:d['descendant'], actual_children))
            self.assertSetEqual(set(expect_children), set(actual_children))
            self.assertEqual(len(actual_children), data['num_children'])
            expect_num_descs = nodes.closure_data.filter(ancestor=data['id'], depth__gt=0).count()
            actual_num_descs = len(data['descendants'])
            self.assertGreaterEqual(actual_num_descs, data['num_children'])
            self.assertEqual(expect_num_descs, actual_num_descs)
            # TODO, verify number of tagged saleable items


    def _assert_adjacent_nodes(self, chosen_subtrees, mocked_request):
        tag_ids = list(map(lambda node:node.value['id'], chosen_subtrees))
        tag_objs = self.serializer_class.Meta.model.objects.filter(id__in=tag_ids).order_by('id')
        serializer = self.serializer_class(many=True, instance=tag_objs,
                context={'request': mocked_request}, usrprof_id=self.usrprof_id)
        actual_data = serializer.data
        chosen_subtrees = sorted(chosen_subtrees, key=lambda t:t.value['id'])
        trees_iter = iter(chosen_subtrees)
        for data in actual_data:
            tree = next(trees_iter)
            self._assert_simple_fields(check_fields=['id','name'],  exp_sale_item=tree.value,
                    ac_sale_item=data)
            self.assertEqual(len(data['ancestors']), 1)
            self.assertEqual(data['ancestors'][0]['depth'], 1)
            expect_children = list(map(lambda d:d.value['id'], tree.children))
            actual_children = list(map(lambda d:d['descendant'], data['descendants']))
            self.assertSetEqual(set(expect_children), set(actual_children))
            self.assertEqual(len(actual_children), data['num_children'])

    def test_adjacent_nodes_only(self):
        mocked_request = Mock()
        mocked_request.query_params = ExtendedDict()
        mocked_request.query_params._mutable = True
        mocked_request.query_params.update({'parent_only': 'yes', 'children_only':'yes'})
        roots = self.existing_trees
        chosen_subtrees = []
        # --------------------------------
        for root in roots:
            sorted_children = sorted(root.children, key=lambda node:node.depth)
            chosen_subtrees.append(sorted_children[-1])
        self._assert_adjacent_nodes(chosen_subtrees, mocked_request)
        # --------------------------------
        chosen_subtrees.clear()
        for root in roots:
            curr_node = root
            while any(curr_node.children):
                sorted_children = sorted(curr_node.children, key=lambda node:node.depth)
                curr_node = sorted_children[-1]
            chosen_subtrees.append(curr_node.parent)
        self._assert_adjacent_nodes(chosen_subtrees, mocked_request)


    def test_all_ascs_of_subtrees(self):
        roots = self.existing_trees
        chosen_subtrees = []
        expect_ancestors = {}
        for root in roots:
            curr_node = root
            ascs = []
            while any(curr_node.children):
                ascs.append(curr_node)
                sorted_children = sorted(curr_node.children, key=lambda node:node.depth)
                curr_node = sorted_children[-1]
            chosen_node = curr_node.parent
            ascs.pop()
            expect_ancestors[chosen_node.value['id']] = ascs
            chosen_subtrees.append(chosen_node)
        chosen_subtrees = sorted(chosen_subtrees, key=lambda t:t.value['id'])
        tag_ids = list(map(lambda node:node.value['id'], chosen_subtrees))
        tag_objs = self.serializer_class.Meta.model.objects.filter(id__in=tag_ids).order_by('id')
        serializer = self.serializer_class(many=True, instance=tag_objs, usrprof_id=self.usrprof_id)
        actual_data = serializer.data
        trees_iter = iter(chosen_subtrees)
        for data in actual_data:
            tree = next(trees_iter)
            self._assert_simple_fields(check_fields=['id','name'],  exp_sale_item=tree.value,
                    ac_sale_item=data)
            self.assertGreater(len(data['ancestors']), 1)
            expect_ancestor = list(map(lambda d:d.value['id'], expect_ancestors[tree.value['id']] ))
            actual_ancestor = list(map(lambda d:d['ancestor'], data['ancestors']))
            self.assertSetEqual(set(expect_ancestor), set(actual_ancestor))


    def test_partial_fields_roots(self):
        roots = self.existing_trees[:2]
        mocked_request = Mock()
        mocked_request.query_params = ExtendedDict()
        mocked_request.query_params._mutable = True
        mocked_request.query_params.update({'fields': 'id,name,num_children',})
        tag_ids = list(map(lambda node:node.value['id'], roots))
        tag_objs = self.serializer_class.Meta.model.objects.filter(id__in=tag_ids).order_by('id')
        serializer = self.serializer_class(many=True, instance=tag_objs, usrprof_id=self.usrprof_id,
                context={'request': mocked_request},)
        actual_data = serializer.data
        def _gen_expect_data(node):
            out = copy.deepcopy(node.value)
            out['num_children'] = len(node.children)
            return out
        expect_data = list(map(_gen_expect_data, roots))
        expect_data = sort_nested_object(expect_data)
        actual_data = sort_nested_object(actual_data)
        self.assertEqual(json.dumps(expect_data), json.dumps(actual_data))


