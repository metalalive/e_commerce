import random
import copy
import json
import operator
from functools import partial, reduce

from django.db.models import Q

from common.util.python import ExtendedList
from product.models.base import ProductTagClosure
from product.serializers.base import TagSerializer

from product.tests.common import _fixtures, http_request_body_template, HttpRequestDataGen, BaseVerificationMixin, listitem_rand_assigner

class TreeNodeMixin:
    def __init__(self, value=None):
        self.value = value
        self._parent = None
        self.children = []

    @property
    def depth(self):
        num_list = [t.depth for t in self.children]
        max_depth = max(num_list) if any(num_list) else 0
        return 1 + max_depth

    @property
    def num_nodes(self):
        num_list = [t.num_nodes for t in self.children]
        num_descs = reduce(operator.add , num_list) if any(num_list) else 0
        return 1 + num_descs

    @property
    def parent(self):
        return self._parent

    @parent.setter
    def parent(self, new_parent):
        old_parent = self._parent
        if old_parent:
            old_parent.children.remove(self)
        if new_parent:
            new_parent.children.append(self)
        self._parent = new_parent

    @classmethod
    def rand_gen_trees(cls, num_trees, min_num_nodes=2, max_num_nodes=15, min_num_siblings=1,
            max_num_siblings=4, write_value_fn=None):
        # this method will generate number of trees, each tree has random number of nodes,
        # each non-leaf node has at least one child (might be random number of children)
        trees = []
        for _ in range(num_trees):
            tree = [cls() for _ in range(random.randrange(min_num_nodes, (max_num_nodes + 1))) ]
            if write_value_fn and callable(write_value_fn):
                for node in tree:
                    write_value_fn(node)
            parent_iter = iter(tree)
            child_iter  = iter(tree)
            next(child_iter)
            try:
                for curr_parent in parent_iter:
                    num_siblings = random.randrange(min_num_siblings, (max_num_siblings + 1))
                    #curr_parent.children = []
                    for _ in range(num_siblings):
                        curr_child = next(child_iter)
                        curr_child.parent = curr_parent
            except StopIteration:
                pass
            finally:
                trees.append(tree[0])
        return trees


    @classmethod
    def gen_from_closure_data(cls, entity_data, closure_data):
        tmp_nodes = {}
        nodes_data = closure_data.filter(depth=0) # tightly coupled with Django ORM
        for node_data in nodes_data:
            assert node_data['ancestor'] == node_data['descendant'], 'depth is zero, ancestor and descendant \
                    have to be the same, node data: %s' % node_data
            node = tmp_nodes.get(node_data['ancestor'])
            assert node is None, 'node conflict, depth:0, node data: %s' % node_data
            entity_dataitem = entity_data.get(id=node_data['ancestor'])
            tmp_nodes[node_data['ancestor']] = cls(value=entity_dataitem)

        nodes_data = closure_data.filter(depth=1)
        for node_data in nodes_data:
            assert node_data['ancestor'] != node_data['descendant'], 'depth is non-zero, ancestor and \
                    descendant have to be different, node data: %s' % node_data
            parent_node = tmp_nodes[node_data['ancestor']]
            child_node  = tmp_nodes[node_data['descendant']]
            assert (parent_node is not None) and (child_node is not None), \
                    'both of parent and child must not be null, node data: %s' % node_data
            assert (child_node.parent is None) and (child_node not in parent_node.children), \
                    'path duplicate ? depth:1, node data: %s' % node_data
            child_node.parent = parent_node

        nodes_data = closure_data.filter(depth__gte=2)
        for node_data in nodes_data:
            assert node_data['ancestor'] != node_data['descendant'], 'depth is non-zero, ancestor and \
                    descendant have to be different, node data: %s' % node_data
            asc_node = tmp_nodes[node_data['ancestor']]
            desc_node = tmp_nodes[node_data['descendant']]
            assert (asc_node is not None) and (desc_node is not None), \
                    'both of ancestor and decendant must not be null, node data: %s' % node_data
            curr_node_pos = desc_node
            for _ in range(node_data['depth']):
                curr_node_pos = curr_node_pos.parent
            #if curr_node_pos != asc_node:
            #    import pdb
            #    pdb.set_trace()
            assert curr_node_pos == asc_node, 'corrupted closure node data: %s' % node_data

        trees = ExtendedList()
        trees.extend(list(filter(lambda t: t.parent is None, tmp_nodes.values())))
        trees.entity_data = entity_data
        trees.closure_data = closure_data
        return trees
    ## end of gen_from_closure_data


    @classmethod
    def _compare_single_tree(cls, node_a, node_b, value_compare_fn):
        diff = []
        is_the_same = value_compare_fn(val_a=node_a.value, val_b=node_b.value)
        if is_the_same is False:
            item = {'message':'value does not matched', 'value_a':node_a.value, 'value_b':node_b.value,}
            diff.append(item)
        num_child_a = len(node_a.children)
        num_child_b = len(node_b.children)
        if num_child_a == num_child_b:
            if num_child_a > 0:
                _, not_matched = cls.compare_trees(trees_a=node_a.children, trees_b=node_b.children,
                        value_compare_fn=value_compare_fn)
                diff.extend(not_matched)
            else:
                pass # leaf node, end of recursive call
        else:
            item = {'message':'num of children does not matched', 'value_a':num_child_a, 'value_b': num_child_b,}
            diff.append(item)
        return diff

    @classmethod
    def compare_trees(cls, trees_a, trees_b, value_compare_fn):
        assert callable(value_compare_fn), 'value_compare_fn: %s has to be callable' % value_compare_fn
        matched = []
        not_matched = []
        for tree_a in trees_a:
            matched_tree = None
            diffs = []
            for tree_b in trees_b:
                diff = cls._compare_single_tree(node_a=tree_a, node_b=tree_b,
                        value_compare_fn=value_compare_fn)
                if any(diff):
                    diffs.append(diff)
                else:
                    matched_tree = tree_b
                    break
            if matched_tree is not None:
                matched.append((tree_a, matched_tree))
            else:
                item = {'message':'tree_a does not matched', 'tree_a':tree_a.value, 'diffs': diffs}
                not_matched.append(item)
        #if any(not_matched):
        #    import pdb
        #    pdb.set_trace()
        return matched, not_matched
## end of class TreeNodeMixin


def _auto_increment_gen_fn(num=0):
    while True:
        num = num + 1
        yield num

_auto_inc_gen =  _auto_increment_gen_fn()


class HttpRequestDataGenTag(HttpRequestDataGen):
    def refresh_req_data(self, trees=None, shuffle=False, num_trees=1, min_num_nodes=1,
            max_num_nodes=1, min_num_siblings=1, max_num_siblings=1, write_value_fn=None):
        if trees is None:
            write_value_fn = write_value_fn or self._write_value_fn
            trees = TreeNodeMixin.rand_gen_trees(
                num_trees=num_trees, min_num_nodes=min_num_nodes,
                max_num_nodes=max_num_nodes, min_num_siblings=min_num_siblings,
                max_num_siblings=max_num_siblings, write_value_fn=write_value_fn)
        req_data = self.trees_to_req_data(trees=trees, shuffle=shuffle)
        return trees, req_data

    def _gen_tag_name(self):
        num_valid_tags = len(_fixtures['ProductTag'])
        idx = random.randrange(0, num_valid_tags)
        return  _fixtures['ProductTag'][idx]['name']

    def _write_value_fn(self, node):
        out = {'name': self._gen_tag_name()}
        node.value = out

    def trees_to_req_data(self, trees, shuffle=False):
        out = []
        for root in trees:
            req_data = self._tree_to_req_data(curr_node=root, parent_data=None)
            out.extend(req_data)
        if shuffle:
            random.shuffle(out)
        for d in out:
            if d['new_parent'] is not None:
                idx = out.index(d['new_parent'])
                d['new_parent'] = idx
        for d in out:
            d.pop('_unik_key', None)
        return out

    def _tree_to_req_data(self, curr_node, parent_data):
        req_data_template = http_request_body_template['ProductTag']
        req_data = req_data_template.copy()
        req_data.update(curr_node.value)
        #req_data['exist_parent'] = 
        req_data['new_parent'] = parent_data
        req_data['_unik_key'] = next(_auto_inc_gen)
        out = [req_data]
        for child in curr_node.children:
            child_req_data = self._tree_to_req_data(curr_node=child, parent_data=req_data)
            out.extend(child_req_data)
        return out


class TagVerificationMixin(BaseVerificationMixin):
    serializer_class = TagSerializer
    err_msg_loop_detected = 'will form a loop, which is NOT allowed in closure table'

    def load_closure_data(self, node_ids):
        # load closure data from django ORM, not from DRF serializer because
        # serializer gets rid of unecessary data like closure id
        closure_node_cls = ProductTagClosure
        entity_cls = self.serializer_class.Meta.model
        condition = Q(descendant__id__in=node_ids)
        closure_qset = closure_node_cls.objects.filter(condition).values(
                'id', 'ancestor', 'descendant', 'depth')
        entity_qset = entity_cls.objects.filter(id__in=node_ids).values('id', 'name')
        return entity_qset, closure_qset

    def _value_compare_fn(self, val_a, val_b):
        return val_a['name'] == val_b['name']


