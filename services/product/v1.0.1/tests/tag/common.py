import random

from django.db.models import Q

from product.models.base import ProductTagClosure
from product.serializers.base import TagSerializer

from ecommerce_common.tests.common import TreeNodeMixin
from tests.common import (
    _fixtures,
    http_request_body_template,
    HttpRequestDataGen,
    BaseVerificationMixin,
)


def _auto_increment_gen_fn(num=0):
    while True:
        num = num + 1
        yield num


_auto_inc_gen = _auto_increment_gen_fn()


class HttpRequestDataGenTag(HttpRequestDataGen):
    def refresh_req_data(
        self,
        trees=None,
        shuffle=False,
        num_trees=1,
        min_num_nodes=1,
        max_num_nodes=1,
        min_num_siblings=1,
        max_num_siblings=1,
        write_value_fn=None,
    ):
        if trees is None:
            write_value_fn = write_value_fn or self._write_value_fn
            trees = TreeNodeMixin.rand_gen_trees(
                num_trees=num_trees,
                min_num_nodes=min_num_nodes,
                max_num_nodes=max_num_nodes,
                min_num_siblings=min_num_siblings,
                max_num_siblings=max_num_siblings,
                write_value_fn=write_value_fn,
            )
        req_data = self.trees_to_req_data(trees=trees, shuffle=shuffle)
        return trees, req_data

    def _gen_tag_name(self):
        num_valid_tags = len(_fixtures["ProductTag"])
        idx = random.randrange(0, num_valid_tags)
        return _fixtures["ProductTag"][idx]["name"]

    def _write_value_fn(self, node):
        out = {"name": self._gen_tag_name()}
        node.value = out

    def trees_to_req_data(self, trees, shuffle=False):
        out = []
        for root in trees:
            req_data = self._tree_to_req_data(curr_node=root, parent_data=None)
            out.extend(req_data)
        if shuffle:
            random.shuffle(out)
        for d in out:
            if d["new_parent"] is not None:
                idx = out.index(d["new_parent"])
                d["new_parent"] = idx
        for d in out:
            d.pop("_unik_key", None)
        return out

    def _tree_to_req_data(self, curr_node, parent_data):
        req_data_template = http_request_body_template["ProductTag"]
        req_data = req_data_template.copy()
        req_data.update(curr_node.value)
        # req_data['exist_parent'] =
        req_data["new_parent"] = parent_data
        req_data["_unik_key"] = next(_auto_inc_gen)
        out = [req_data]
        for child in curr_node.children:
            child_req_data = self._tree_to_req_data(
                curr_node=child, parent_data=req_data
            )
            out.extend(child_req_data)
        return out


class TagVerificationMixin(BaseVerificationMixin):
    serializer_class = TagSerializer
    err_msg_loop_detected = "will form a loop, which is NOT allowed in closure table"

    def load_closure_data(self, node_ids):
        # load closure data from django ORM, not from DRF serializer because
        # serializer gets rid of unecessary data like closure id
        closure_node_cls = ProductTagClosure
        entity_cls = self.serializer_class.Meta.model
        condition = Q(descendant__id__in=node_ids)
        closure_qset = closure_node_cls.objects.filter(condition).values(
            "id", "ancestor", "descendant", "depth"
        )
        entity_qset = entity_cls.objects.filter(id__in=node_ids).values("id", "name")
        return entity_qset, closure_qset

    def _value_compare_fn(self, val_a, val_b):
        return val_a["name"] == val_b["name"]
