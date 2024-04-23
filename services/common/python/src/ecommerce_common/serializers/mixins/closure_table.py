import logging
from collections import OrderedDict

from ecommerce_common.util.graph import path_exists
from ecommerce_common.validators import (
    SelectIDsExistValidator,
    TreeNodesLoopValidator,
    ClosureCrossTreesLoopValidator,
)
from softdelete.models import SoftDeleteObjectMixin

EMPTY_VALUES = (None, "", [], (), {})
_logger = logging.getLogger(__name__)


class ClosureTableMixin:
    """generic class for maintaining closure-table data structure"""

    EMPTY_VALUES = EMPTY_VALUES
    CLOSURE_MODEL_CLS = None
    PK_FIELD_NAME = None
    DEPTH_FIELD_NAME = None
    ANCESTOR_FIELD_NAME = None
    DESCENDANT_FIELD_NAME = None

    @property
    def is_create(self):
        """detect whether to perform create operation"""
        raise NotImplementedError("`ClosureTableMixin.is_create` must be implemented.")

    def _get_field_data(self, form, key, default=None, remove_after_read=False):
        """
        get field data of a form by passing an associated key,
        devleopers must override this function since form structure varies between applications
        """
        raise NotImplementedError(
            "`ClosureTableMixin._get_field_data(form, key, default)` must be implemented."
        )

    def _set_field_data(self, form, key, val):
        raise NotImplementedError(
            "`ClosureTableMixin._set_field_data(form, key, val)` must be implemented."
        )

    def get_node_ID(self, node):
        raise NotImplementedError(
            "`ClosureTableMixin.get_node_ID(node)` must be implemented."
        )

    def _loopdetect_errmsg(self, loop_node_list):
        err_msg = "{loop_forms} will form a loop, which is NOT allowed in closure table"
        loop_node_list = ["".join(["form #", str(n)]) for n in loop_node_list]
        loop_node_list = ", ".join(loop_node_list)
        return err_msg.format(loop_forms=loop_node_list)

    # TODO: explain what's new_parent in comment
    def prepare_cycle_detection_validators(self, forms):
        """
        This mixin class maintains trees in a closure table, which should be acyclic
        . This function provides validator class, arranges root of each edit tree
        and its new parent to a set of edges e.g. (new_parent, root_id), feed
        the edges to the cycle-detection validator.
        """
        assert (
            self.CLOSURE_MODEL_CLS
            and self.DEPTH_FIELD_NAME
            and self.ANCESTOR_FIELD_NAME
            and self.DESCENDANT_FIELD_NAME
        ), "caller must provide all of the  parameters : CLOSURE_MODEL_CLS, DEPTH_FIELD_NAME, ANCESTOR_FIELD_NAME,DESCENDANT_FIELD_NAME"
        tree_edge = []
        init_kwargs = {"tree_edge": tree_edge, "err_msg_cb": self._loopdetect_errmsg}
        for idx in range(len(forms)):
            form = forms[idx]
            exist_parent = self._get_field_data(form, "exist_parent", default=None)
            exist_parent = str(exist_parent) if exist_parent is not None else ""
            if self.is_create:
                new_parent = self._get_field_data(form, "new_parent", default=None)
                new_parent = str(new_parent) if new_parent is not None else ""
                if (exist_parent in self.EMPTY_VALUES) and (
                    not new_parent in self.EMPTY_VALUES
                ):
                    parent_id = new_parent
                else:
                    # when a new node's parent is another existent node (exist_parent), it's unlikely to be
                    # the rootcause when a loop is detected, so the graph can be simplified by setting
                    # current node as root node
                    parent_id = TreeNodesLoopValidator.ROOT_OF_TREE
                tree_edge.append(tuple([parent_id, str(idx)]))
            else:  # edit
                if exist_parent in self.EMPTY_VALUES:
                    exist_parent = TreeNodesLoopValidator.ROOT_OF_TREE
                parent_id = exist_parent
                edge_dst = self._get_field_data(form, "id")
                assert edge_dst is not None, "edge_dst must NOT be null"
                edge_dst = str(edge_dst)
                tree_edge.append(tuple([parent_id, edge_dst]))
        if self.is_create:
            validator_cls = TreeNodesLoopValidator
        else:  # edit
            init_kwargs["closure_model"] = self.CLOSURE_MODEL_CLS
            init_kwargs["depth_column_name"] = self.DEPTH_FIELD_NAME
            init_kwargs["ancestor_column_name"] = self.ANCESTOR_FIELD_NAME
            init_kwargs["descendant_column_name"] = self.DESCENDANT_FIELD_NAME
            validator_cls = ClosureCrossTreesLoopValidator
        vobj = validator_cls(**init_kwargs)
        self._closure_nodes_dependency_graph = vobj.graph
        # if not self.is_create:
        #    import pdb
        #    pdb.set_trace()
        return vobj

    def get_sorted_insertion_forms(self, forms):
        """reorder the forms in case there are dependencies among the newly added forms"""
        if hasattr(self, "_sorted_insertion_forms"):
            return self._sorted_insertion_forms
        self._saved_nodes = []  # TODO: delete at the end of creation operations
        insert_after = {}
        insert_after_log = []
        seq_to_sorted_log = []
        unsorted_forms = []
        sorted_forms = []
        for idx in range(len(forms)):
            form = forms[idx]
            exist_parent = self._get_field_data(form, "exist_parent", default="")
            new_parent = self._get_field_data(form, "new_parent", default="")
            self._set_field_data(form=form, key="_sort_idx", val=idx)
            if exist_parent in self.EMPTY_VALUES:
                if new_parent in self.EMPTY_VALUES:
                    sorted_forms.append(form)
                else:  # record the position the current form should be in sorted list
                    key = idx
                    unsorted_forms.append(form)
                    insert_after[key] = forms[int(new_parent)]
                    insert_after_log.append((key, int(new_parent)))
            else:
                sorted_forms.append(form)
        while (
            len(sorted_forms) < len(forms) and not unsorted_forms in self.EMPTY_VALUES
        ):
            for form in unsorted_forms:
                key = self._get_field_data(form=form, key="_sort_idx")
                try:
                    new_parent = sorted_forms.index(insert_after[key])
                    seq_to_sorted_log.append((key, new_parent))
                    self._set_field_data(form, "new_parent", new_parent)
                    sorted_forms.append(form)
                    unsorted_forms.remove(form)
                except ValueError as e:
                    # skip if parent form required hasn't been added yet to the sorted list.
                    seq_to_sorted_log.append((key, -1))
        for form in sorted_forms:
            self._get_field_data(form=form, key="_sort_idx", remove_after_read=True)
        # import pdb
        # pdb.set_trace()
        self._sorted_insertion_forms = sorted_forms
        log_msg = [
            "insert_after",
            insert_after_log,
            "seq_to_sorted_log",
            seq_to_sorted_log,
        ]
        _logger.debug(None, *log_msg)
        return sorted_forms

    def _get_insertion_parent_id(self, exist_parent="", new_parent=""):
        parent_id = ""
        if exist_parent in self.EMPTY_VALUES:
            if new_parent in self.EMPTY_VALUES:
                parent_id = ""  # -1
            else:
                parent = self._saved_nodes[int(new_parent)]
                parent_id = self.get_node_ID(parent)
        else:
            parent_id = exist_parent
        return parent_id

    #### end of _get_insertion_parent_id

    def get_insertion_ancestors(self, leaf_node, exist_parent="", new_parent=""):
        if leaf_node is None or self.get_node_ID(leaf_node) is None:
            raise ValueError("leaf_node must be saved before calling this function")
        self._saved_nodes.append(leaf_node)
        out = [
            {
                self.ANCESTOR_FIELD_NAME: leaf_node,
                self.DESCENDANT_FIELD_NAME: leaf_node,
                self.DEPTH_FIELD_NAME: 0,
            }
        ]
        parent_id = self._get_insertion_parent_id(
            exist_parent=exist_parent, new_parent=new_parent
        )
        log_msg = [
            "exist_parent",
            exist_parent,
            "new_parent",
            new_parent,
            "parent_id",
            parent_id,
        ]
        _logger.debug(None, *log_msg)
        if not parent_id in self.EMPTY_VALUES:
            node_cls = type(
                leaf_node
            )  # the node_cls must have related field `ancestors`
            ancestors = node_cls.objects.get(
                pk=parent_id
            ).ancestors.all()  # TODO refactor
            path_objs = [
                {
                    self.DEPTH_FIELD_NAME: (a.depth + 1),
                    self.ANCESTOR_FIELD_NAME: a.ancestor,
                    self.DESCENDANT_FIELD_NAME: leaf_node,
                }
                for a in ancestors
            ]
            out += path_objs
        return out

    def _init_edit_tree(self, forms, instances):
        """
        This function initializes data structure for a set of trees edited and sumbitted in the form data
        on client request. If the form data contains a tree whose parent ID will be changed,
        the change of the tree will be recorded to the output edit tree structure.
        """
        edit_trees = {}
        old_parents = {}
        new_parents = {}
        old_parents_log = []
        new_parents_log = []
        for form in forms:
            _id = self._get_field_data(form=form, key="id")
            new_parents[str(_id)] = form
            _exist_parent = self._get_field_data(form=form, key="exist_parent")
            new_parents_log.append((_id, _exist_parent))
        for node in instances:
            qset = node.ancestors.filter(**{self.DEPTH_FIELD_NAME: 1})
            parent_node = qset[0].ancestor if len(qset) == 1 else None
            old_parents[str(node.pk)] = {"node": node, "parent": parent_node}
            old_parents_log.append((node.pk, parent_node.pk if parent_node else -1))
        for k, v in old_parents.items():
            new_parent_ID = self._get_field_data(
                form=new_parents[k], key="exist_parent", default=""
            )
            old_parent_ID = str(v["parent"].pk) if v["parent"] else ""
            if new_parent_ID != old_parent_ID:
                node_cls = type(v["node"])
                new_parent = (
                    node_cls.objects.get(pk=new_parent_ID)
                    if not new_parent_ID in self.EMPTY_VALUES
                    else None
                )
                edit_trees[k] = {
                    "old": {
                        "obj": v["parent"],
                    },
                    "new": {
                        "obj": new_parent,
                    },
                    "obj": v["node"],
                    "form": new_parents[k],
                }
                #### self._set_field_data( form=new_parents[k], key='_edit_tree', val=edit_trees[k] )
        log_msg = [
            "old_parents_log",
            old_parents_log,
            "new_parents_log",
            new_parents_log,
        ]
        _logger.debug(None, *log_msg)
        del old_parents
        del new_parents
        return edit_trees

    def _reorder_edit_tree(self, edit_trees_in):
        """
        The given list of edit tree has to be reordered if there are dependencies found
        among the trees, (e.g. tree T_A, T_B in the edit tree list, new parent of T_A is one
        of descendants of T_B ... etc) reorder ensures correctness of update sequence.
        """
        update_after = {}
        unsorted_trees = {}
        sorted_trees = {}
        dependency_log = []
        # check if new parent of each tree is a descendant of another tree in the same edit list,
        # if so, then it is essential to build  dependancy graph
        id_list = list(edit_trees_in.keys())
        asc_field_name = "{a}__in".format(a=self.ANCESTOR_FIELD_NAME)
        for k, v in edit_trees_in.items():  # estimate update dependency
            if v["new"]["obj"]:
                id_list.remove(k)
                node_srcs = tuple(
                    filter(
                        lambda src: path_exists(
                            graph=self._closure_nodes_dependency_graph,
                            node_src=src,
                            node_dst=k,
                        ),
                        id_list,
                    )
                )
                # if set(id_list) != set(node_srcs):
                #    import pdb
                #    pdb.set_trace()
                conditions = {asc_field_name: node_srcs}
                qset = (
                    v["new"]["obj"]
                    .ancestors.filter(**conditions)
                    .order_by(self.DEPTH_FIELD_NAME)
                    .values_list(self.ANCESTOR_FIELD_NAME, flat=True)
                )
                qset = tuple(map(str, qset))
                id_list.append(k)
            else:
                qset = []
            if len(qset) == 0:
                v["dependency"] = None
                sorted_trees[k] = v
            else:
                v["dependency"] = qset[0]
                unsorted_trees[k] = v
                update_after[k] = qset
            dependency_log.append((k, v["dependency"]))
        log_msg = ["dependency_log", dependency_log]
        _logger.debug(None, *log_msg)

        remove = []
        while len(sorted_trees) < len(edit_trees_in):  # start reordering
            log_msg = ["update_after_snapshot", update_after]
            _logger.debug(None, *log_msg)
            for k, v in unsorted_trees.items():
                sorted_id_list = list(sorted_trees.keys())
                update_after[k] = list(set(update_after[k]) - set(sorted_id_list))
                if update_after[k] in self.EMPTY_VALUES:
                    sorted_trees[k] = v
                    remove.append(k)
            # if not any(remove):
            #    import pdb
            #    pdb.set_trace()
            assert any(
                remove
            ), "abort from infinite loop due to failure on sorting edit trees"
            for k in remove:
                del unsorted_trees[k]
                del update_after[k]
            remove.clear()
        return sorted_trees

    ### end of  _reorder_edit_tree

    def _construct_edit_ancestors(self, edit_trees_in):
        """
        This function estimates difference of all ancestors before/after editing each tree in the list.
        Note the given edit-tree list should be sorted by dependency before running this function.
        """
        depth_desc_order = "-{d}".format(d=self.DEPTH_FIELD_NAME)
        ancestor_compare_log = []
        for k, v in edit_trees_in.items():
            # retrieve old ancestors of each edit tree from model layer
            old_parent = v["old"]["obj"]
            new_parent = v["new"]["obj"]
            qset_old_ascs = old_parent.ancestors.all() if old_parent else []
            qset_new_ascs = (
                new_parent.ancestors.all().order_by(depth_desc_order)
                if new_parent
                else []
            )
            v["old"]["ancestors"] = [
                {
                    "obj": a.ancestor,
                    "depth": (a.depth + 1),
                }
                for a in qset_old_ascs
            ]
            if v["dependency"] is None:
                v["new"]["ancestors"] = [
                    {
                        "obj": a.ancestor,
                        "depth": (a.depth + 1),
                    }
                    for a in qset_new_ascs
                ]
            else:
                if new_parent is None:
                    log_msg = [
                        "new_parent",
                        new_parent,
                        "v_dependency",
                        v["dependency"],
                        "node_pk",
                        k,
                    ]
                    _logger.error(None, *log_msg)
                    raise ValueError(
                        "A node with dependency must have another node instance as its new parent"
                    )
                dependency_node = edit_trees_in[v["dependency"]]
                v["new"]["ancestors"] = [
                    {"obj": a["obj"], "depth": 0}
                    for a in dependency_node["new"]["ancestors"]
                ]
                d_idx = 0
                #### for idx, a in enumerate(qset_new_ascs):
                for a in qset_new_ascs:
                    if str(a.ancestor.pk) == v["dependency"]:
                        break
                    d_idx = 1 + d_idx
                qset_new_ascs = list(qset_new_ascs[d_idx:])
                v["new"]["ancestors"] += [
                    {"obj": a.ancestor, "depth": 0} for a in qset_new_ascs
                ]
                ancestor_len = len(v["new"]["ancestors"])
                for idx in range(ancestor_len):
                    v["new"]["ancestors"][idx]["depth"] = ancestor_len - idx

            ancestor_compare_log_item = {
                "node_pk": k,
                "old": list(map(lambda a: a["obj"].pk, v["old"]["ancestors"])),
                "new": list(map(lambda a: a["obj"].pk, v["new"]["ancestors"])),
                "depth": list(map(lambda a: a["depth"], v["new"]["ancestors"])),
            }
            ancestor_compare_log += ["ancestor_compare_log", ancestor_compare_log_item]

        log_msg = ancestor_compare_log
        _logger.debug(None, *log_msg)

    #### end of _construct_edit_ancestors

    def _construct_edit_descendants(self, edit_trees_in):
        """
        This function estimates difference of all descendants before/after editing each tree in
        the edit list. Note the given edit list must be sorted by dependency.
        """
        old_descs = {}
        edit_tree_roots = edit_trees_in.keys()
        # retrieve old descendants of each edit tree from model layer
        for k, v in edit_trees_in.items():
            qset_old_descs = v["obj"].descendants.all()
            v["old"]["descendants"] = [
                {"obj": d.descendant, "depth": d.depth} for d in qset_old_descs
            ]
            # In each node, find the subtree which will be moving somewhere in edit list
            old_descs[k] = [str(d["obj"].pk) for d in v["old"]["descendants"]]
            v["old"]["moving_subtree_root"] = [
                k2 for k2 in edit_tree_roots if k != k2 and k2 in old_descs[k]
            ]
            # find subtrees that will be (1) added to current edit tree from another edit tree
            # (2) still under the same tree, but in different position.
            v["new"]["moving_subtree_root"] = []
            if v["dependency"]:
                edit_trees_in[v["dependency"]]["new"]["moving_subtree_root"].append(k)
            # For each edit tree, find the subtrees that will be moved out (to another edit tree)
            v["new"]["move_out_subtree_root"] = []
            # find subtrees that move internally , check the ancestor path from node `k` to
            # node `k2`, there might be different ancestor(s) in the middle of the path
            v["new"]["move_internal_subtree_root"] = []
            find_ancestor_fn = lambda x: str(x["obj"].pk) == str(k)
            for k2 in v["old"]["moving_subtree_root"]:
                new_ancs = [
                    str(a["obj"].pk) for a in edit_trees_in[k2]["new"]["ancestors"]
                ]
                if not k in new_ancs:
                    v["new"]["move_out_subtree_root"].append(k2)
                k_in_old_asc = list(
                    filter(find_ancestor_fn, edit_trees_in[k2]["old"]["ancestors"])
                )
                k_in_new_asc = list(
                    filter(find_ancestor_fn, edit_trees_in[k2]["new"]["ancestors"])
                )
                if any(k_in_old_asc) and any(k_in_new_asc):
                    if len(k_in_old_asc) == 1 and len(k_in_new_asc) == 1:
                        k_depth_in_old_asc = k_in_old_asc[0]["depth"]
                        k_depth_in_new_asc = k_in_new_asc[0]["depth"]
                        ## if k_depth_in_old_asc != k_depth_in_new_asc:
                        ##     v['new']['move_internal_subtree_root'].append(k2)
                        old_ascs_k_to_k2 = filter(
                            lambda x: x["depth"] <= k_depth_in_old_asc,
                            edit_trees_in[k2]["old"]["ancestors"],
                        )
                        new_ascs_k_to_k2 = filter(
                            lambda x: x["depth"] <= k_depth_in_new_asc,
                            edit_trees_in[k2]["new"]["ancestors"],
                        )
                        extract_fn = lambda x: (x["obj"].pk, x["depth"])
                        old_ascs_k_to_k2 = set(map(extract_fn, old_ascs_k_to_k2))
                        new_ascs_k_to_k2 = set(map(extract_fn, new_ascs_k_to_k2))
                        diff = old_ascs_k_to_k2 ^ new_ascs_k_to_k2
                        if any(diff):
                            v["new"]["move_internal_subtree_root"].append(k2)
                    else:  # TODO, log error
                        raise ValueError
        ## end of edit_trees_in.items() iteration

        # In each edit tree, get rid of the subtree(s) which meet the condition above.
        remove = []
        for k, v in edit_trees_in.items():
            exc_ids = (
                set(v["new"]["moving_subtree_root"])
                | set(v["new"]["move_out_subtree_root"])
                | set(v["new"]["move_internal_subtree_root"])
            )
            for w in exc_ids:  # collect ID of all descendants in the subtree
                excp_old_descs = [
                    str(d["obj"].pk) for d in edit_trees_in[w]["old"]["descendants"]
                ]
                if k in excp_old_descs:  # means a path is found from `w` to `k`
                    has_path = path_exists(
                        graph=self._closure_nodes_dependency_graph,
                        node_src=w,
                        node_dst=k,
                    )
                    if has_path is False:
                        excp_old_descs = list(
                            filter(lambda n: n not in old_descs[k], excp_old_descs)
                        )
                    # import pdb
                    # pdb.set_trace()
                remove += excp_old_descs
            v["new"]["descendants"] = list(
                filter(
                    lambda d: str(d["obj"].pk) not in remove, v["old"]["descendants"]
                )
            )
            ## [d for d in v['old']['descendants'] if not str(d['obj'].pk) in remove]
            remove.clear()
        # import pdb
        # pdb.set_trace()

        descendant_compare_log = []
        for k, v in edit_trees_in.items():
            descendant_compare_log_item = {
                "node_pk": k,
                "old": list(map(lambda d: d["obj"].pk, v["old"]["descendants"])),
                "new": list(map(lambda d: d["obj"].pk, v["new"]["descendants"])),
            }
            descendant_compare_log += [
                "descendant_compare_log",
                descendant_compare_log_item,
            ]

        log_msg = descendant_compare_log
        _logger.debug(None, *log_msg)

        for k, v in edit_trees_in.items():
            # cleanup after finish
            v["old"]["moving_subtree_root"].clear()
            v["new"]["moving_subtree_root"].clear()
            v["new"]["move_out_subtree_root"].clear()
            v["new"]["move_internal_subtree_root"].clear()
            v["old"]["descendants"].clear()
            del v["old"]["moving_subtree_root"]
            del v["new"]["moving_subtree_root"]
            del v["new"]["move_out_subtree_root"]
            del v["new"]["move_internal_subtree_root"]
            del v["old"]["descendants"]

    #### end of _construct_edit_descendants

    def _estimate_update_conflict(self, edit_trees_in):
        """
        find all objects in update sequence that will cause violation on unique
        constraint (if applied) on closure table model
        """
        entire_data_list = OrderedDict()
        entire_obj_list = OrderedDict()
        for k, v in edit_trees_in.items():
            closure_tree = self._get_field_data(form=v["form"], key="closure_tree")
            update_path_data = closure_tree["update"]["data"]
            update_path_objs = closure_tree["update"]["obj"]
            assert len(update_path_data) == len(update_path_objs)
            for d in update_path_data:
                asc_id = d[self.ANCESTOR_FIELD_NAME].pk
                desc_id = d[self.DESCENDANT_FIELD_NAME].pk
                entire_data_list[(asc_id, desc_id)] = d
            for obj in update_path_objs:
                asc_id = getattr(obj, self.ANCESTOR_FIELD_NAME).pk
                desc_id = getattr(obj, self.DESCENDANT_FIELD_NAME).pk
                entire_obj_list[(asc_id, desc_id)] = obj

        path_keys_data = entire_data_list.keys()
        path_keys_obj = entire_obj_list.keys()
        union = set(path_keys_data) & set(path_keys_obj)
        union_removed = []
        for u in union:
            new_id = entire_data_list[u][self.PK_FIELD_NAME]
            old_id = getattr(entire_obj_list[u], self.PK_FIELD_NAME)
            if old_id == new_id:
                union_removed.append(u)

        union = union - set(union_removed)
        obj_map = map(lambda u: entire_obj_list[u], union)
        self._conflict_update_paths = list(obj_map)
        log_msg = [
            "path_keys_data",
            path_keys_data,
            "path_keys_obj",
            path_keys_obj,
            "union",
            union,
            "_conflict_update_paths",
            list(
                map(
                    lambda p: getattr(p, self.PK_FIELD_NAME),
                    self._conflict_update_paths,
                )
            ),
        ]
        _logger.debug(None, *log_msg)

    #### end of _estimate_update_conflict

    def _construct_edit_paths(self, edit_trees_in):
        if not hasattr(self, "_delete_path_objs"):
            self._delete_path_objs = []
        delete_path_objs = self._delete_path_objs
        pk_field_name = "{p}__in".format(p=self.PK_FIELD_NAME)
        asc_field_name = "{a}__in".format(a=self.ANCESTOR_FIELD_NAME)
        depth_desc_order = "-{d}".format(d=self.DEPTH_FIELD_NAME)
        create_path_log = []
        update_path_log = []
        delete_path_log = []

        for k, v in edit_trees_in.items():
            old_path_len = len(v["old"]["ancestors"])
            new_path_len = len(v["new"]["ancestors"])
            num_new_descendants = len(v["new"]["descendants"])
            create_path_data = []
            update_path_objs = []
            update_path_data = []
            old_paths = []  # collecting paths that are already stored
            anc_id_union = {
                asc_field_name: [a["obj"].pk for a in v["old"]["ancestors"]]
            }
            exclude_path_dup = self._get_all_assigned_paths()
            exclude_path_del = list(map(lambda obj: obj.pk, delete_path_objs))
            exclude_paths = {pk_field_name: exclude_path_dup + exclude_path_del}
            for d in v["new"]["descendants"]:
                # to reduce conflicts when updating existing paths (nodes), it is good to load old path of an
                # editing node which starts from its root to its old parent
                qset = (
                    d["obj"].ancestors.filter(**anc_id_union).exclude(**exclude_paths)
                )
                qset = qset.order_by(depth_desc_order)
                old_paths += list(qset)
            old_ascs_log = list(map(lambda a: a["obj"].pk, v["old"]["ancestors"]))
            new_descs_log = list(map(lambda d: d["obj"].pk, v["new"]["descendants"]))
            idx = jdx = kdx = 0
            for jdx in range(new_path_len):
                for kdx in range(num_new_descendants):
                    selected_obj_list = update_path_data
                    idx = jdx * num_new_descendants + kdx
                    if idx >= len(
                        old_paths
                    ):  # if running out of available elements in old_paths
                        if (
                            len(delete_path_objs) > 0
                        ):  # check if I can reuse the model instances that will be deleted
                            obj = delete_path_objs.pop(0)
                        else:  # create new model instance object(s)
                            obj = None
                            selected_obj_list = create_path_data
                    else:
                        obj = old_paths[idx]
                    path_obj = {}
                    if obj:
                        path_dup = self._get_editing_path(path_id=obj.pk)
                        if path_dup is not None:
                            log_msg = [
                                "msg",
                                "duplicate path found",
                                "subtree_root_id",
                                k,
                                "path_id",
                                obj.pk,
                                "old_ascs",
                                old_ascs_log,
                                "new_descs",
                                new_descs_log,
                            ]
                            _logger.error(None, *log_msg)
                        path_obj[self.PK_FIELD_NAME] = obj.pk
                        update_path_objs.append(obj)
                    path_obj[self.DESCENDANT_FIELD_NAME] = v["new"]["descendants"][kdx][
                        "obj"
                    ]
                    path_obj[self.ANCESTOR_FIELD_NAME] = v["new"]["ancestors"][jdx][
                        "obj"
                    ]
                    path_obj[self.DEPTH_FIELD_NAME] = (
                        v["new"]["ancestors"][jdx]["depth"]
                        + v["new"]["descendants"][kdx]["depth"]
                    )
                    selected_obj_list.append(path_obj)
            #### end of iteration  range(new_path_len)
            if (
                old_path_len > new_path_len
            ):  # move the objects unused in this iteration to delete list, they would be used later
                idx = (1 + jdx) * (1 + kdx) if new_path_len > 0 else 0
                delete_path_objs += old_paths[idx:]

            self._chk_duplicate_paths(update_path_data, log_collector=update_path_log)

            create_path_log_part = []
            for m in create_path_data:
                _asc = m[self.ANCESTOR_FIELD_NAME]
                _desc = m[self.DESCENDANT_FIELD_NAME]
                _dep = m[self.DEPTH_FIELD_NAME]
                create_path_log_part.append(
                    {"from": _asc.pk, "to": _desc.pk, "depth": _dep}
                )
            create_path_log += create_path_log_part
            log_msg = [
                "msg",
                "edit tree check completed",
                "subtree_root_id",
                k,
                "old_ascs",
                old_ascs_log,
                "new_descs",
                new_descs_log,
            ]
            _logger.info(None, *log_msg)

            closure_tree = {
                "update": {"obj": update_path_objs, "data": update_path_data},
                "create": create_path_data,
            }
            self._set_field_data(form=v["form"], key="closure_tree", val=closure_tree)
        #### end of iteration edit_trees_in.items()
        self._estimate_update_conflict(edit_trees_in=edit_trees_in)
        for idx in range(len(delete_path_objs)):
            obj = delete_path_objs[idx]
            unused = {
                self.PK_FIELD_NAME: obj.pk,
                self.DESCENDANT_FIELD_NAME: obj.descendant,
                "_obj": obj,
                self.ANCESTOR_FIELD_NAME: obj.ancestor,
                self.DEPTH_FIELD_NAME: obj.depth,
            }
            delete_path_objs[idx] = unused
        self._chk_duplicate_paths(
            delete_path_objs, tag="delete", clear=True, log_collector=delete_path_log
        )

        log_msg = [
            "update_path_log",
            update_path_log,
            "create_path_log",
            create_path_log,
            "delete_path_log",
            delete_path_log,
        ]
        _logger.debug(None, *log_msg)

    #### end of _construct_edit_paths

    def get_sorted_update_forms(self, forms, instances):
        if not hasattr(self, "_sorted_update_forms"):
            # check whether parent is modified.
            edit_trees = self._init_edit_tree(forms=forms, instances=instances)
            if any(edit_trees):
                # reorder if there's cascading updates e.g. new parent of one node is in subtree of another node in the update list
                edit_trees = self._reorder_edit_tree(edit_trees_in=edit_trees)
                # construct old/new ancestors for each edit tree
                self._construct_edit_ancestors(edit_trees_in=edit_trees)
                # construct old/new descendants for each edit tree
                self._construct_edit_descendants(edit_trees_in=edit_trees)
                # construct paths for old/new ancestors/descendants
                self._construct_edit_paths(edit_trees_in=edit_trees)

            # re-construct sorted form as output
            sorted_forms = [v.pop("form") for v in edit_trees.values()]
            all_ids = [str(self._get_field_data(form=form, key="id")) for form in forms]
            excluded = list(set(all_ids) - set(edit_trees.keys()))
            excluded = [
                form
                for form in forms
                if str(self._get_field_data(form=form, key="id")) in excluded
            ]
            self._sorted_update_forms = excluded + sorted_forms

        return self._sorted_update_forms

    #### end of get_sorted_update_forms

    def _chk_duplicate_paths(
        self, obj_list, tag="update", clear=False, log_collector=None
    ):
        if not hasattr(self, "_trace_duplicate_paths"):
            self._trace_duplicate_paths = {}
        _log_part = []
        for m in obj_list:
            _id = m[self.PK_FIELD_NAME]
            _asc = m[self.ANCESTOR_FIELD_NAME]
            _desc = m[self.DESCENDANT_FIELD_NAME]
            _dep = m[self.DEPTH_FIELD_NAME]
            path_item = {"path_id": _id, "from": _asc.pk, "to": _desc.pk, "depth": _dep}
            if log_collector is not None:
                _log_part.append(path_item)
            m2 = self._get_editing_path(
                path_id=_id
            )  # m2 = self._trace_duplicate_paths.get(_id, None)
            if m2 is not None:
                _ori_id = m2[self.PK_FIELD_NAME]
                _ori_asc = m2[self.ANCESTOR_FIELD_NAME].pk
                _ori_desc = m2[self.DESCENDANT_FIELD_NAME].pk
                _ori_dep = m2[self.DEPTH_FIELD_NAME]
                log_msg = [
                    "path_id_dup",
                    _id,
                    "from_dup",
                    _asc.pk,
                    "to_dup",
                    _desc.pk,
                    "depth_dup",
                    _dep,
                    "path_id_ori",
                    _ori_id,
                    "from_ori",
                    _ori_asc,
                    "to_ori",
                    _ori_desc,
                    "depth_ori",
                    _ori_dep,
                    "msg",
                    "duplicate path found",
                    "tag",
                    tag,
                    "clear",
                    clear,
                    "log_collector",
                    log_collector,
                ]
                _logger.error(None, *log_msg)
                err_msg = "duplicate path found : ID={path_id}, asc={from}, desc={to}, depth={depth}"
                err_msg = err_msg.format(**path_item)
                raise ValueError(err_msg)
            else:
                self._trace_duplicate_paths[_id] = m
        #### end of iteration obj_list
        if log_collector is not None:
            for item in _log_part:
                log_collector.append(item)
        if clear:
            self._trace_duplicate_paths.clear()
            delattr(self, "_trace_duplicate_paths")

    #### end of _chk_duplicate_paths

    def _get_editing_path(self, path_id):
        """get existing path from internal editing list"""
        out = None
        if hasattr(self, "_trace_duplicate_paths"):
            out = self._trace_duplicate_paths.get(path_id, None)
        return out

    def _get_all_assigned_paths(self):
        if hasattr(self, "_trace_duplicate_paths"):
            keys = list(self._trace_duplicate_paths.keys())
        else:
            keys = []
        return keys

    def clean_dup_update_paths(self):
        """
        clean up duplicate paths (nodes) in bulk update operation by :
            * deleting unused nodes
            * temporarily setting null to path objects that will be updated with
              new value at later time, in order to meet unique constraint requirement
        """
        delete_nodes_log = []
        if hasattr(self, "_delete_path_objs"):
            # clean up useless clsoure nodes prior to creating or updating other nodes
            # , in order not to violate unique constraint applied at model level
            for d in self._delete_path_objs:
                delete_nodes_log.append(str(d["_obj"].pk))
                kwargs = {}
                if isinstance(d["_obj"], SoftDeleteObjectMixin):
                    kwargs["hard"] = True
                d["_obj"].delete(**kwargs)
            self._delete_path_objs.clear()
            delattr(self, "_delete_path_objs")

        if hasattr(self, "_conflict_update_paths"):
            for u in self._conflict_update_paths:
                setattr(u, self.ANCESTOR_FIELD_NAME, None)
                setattr(u, self.DESCENDANT_FIELD_NAME, None)
                u.save(accept_null_node=True)
            self._conflict_update_paths.clear()
            delattr(self, "_conflict_update_paths")
        log_msg = ["deleted_unused_nodes", ",".join(delete_nodes_log)]
        _logger.info(None, *log_msg)


#### end of class ClosureTableMixin


class BaseClosureNodeMixin:
    """
    * this mixin should work with common.serializers.ExtendedModelSerializer
      and be placed prior to ExtendedModelSerializer in MRO (module resolution
      order in python class inheritance)
    * any serializer that inherits this mixins should have list serializer which
      also inherits ClosureTableMixin (as shown above)
    """

    class Meta:
        validate_only_field_names = ["exist_parent", "new_parent"]

    def __init__(self, *args, **kwargs):
        super().__init__(*args, **kwargs)
        req = self.context.get("request", None)
        if req:
            self._parent_only = req.query_params.get("parent_only", False)
            self._children_only = req.query_params.get("children_only", False)
        else:
            self._parent_only = False
            self._children_only = False
        if self._parent_only:
            self._init_fields_adj_node(
                new_fields=["ancestors", "ancestor", "depth", "id"]
            )
        if self._children_only:
            self._init_fields_adj_node(
                new_fields=["descendants", "descendant", "depth", "id"]
            )

    def _init_fields_adj_node(self, new_fields):
        req = self.context.get("request", None)
        allowed_fields = req.query_params.get("fields", "").split(",")
        if any(allowed_fields):
            allowed_fields.extend(new_fields)
            allowed_fields = list(set(allowed_fields))
            backup_mutable = req.query_params._mutable
            req.query_params._mutable = True
            req.query_params["fields"] = ",".join(allowed_fields)
            req.query_params._mutable = backup_mutable

    def to_representation(self, instance, _logger=None):
        out = super().to_representation(instance=instance)
        _filter_fn = lambda a: a.get("depth", None) == 1
        if self._parent_only:
            _reduced = list(filter(_filter_fn, out.get("ancestors", [])))
            if out.get("ancestors", None):
                out["ancestors"] = _reduced
            if _logger:
                log_msg = [
                    "grp_id",
                    out["id"],
                    "grp_ancestors",
                    out["ancestors"],
                    "parent_only",
                    self._parent_only,
                ]
                _logger.debug(None, *log_msg)
        if self._children_only:
            _reduced = list(filter(_filter_fn, out.get("descendants", [])))
            if out.get("descendants", None):
                out["descendants"] = _reduced
        return out

    def validate(self, value, _logger=None, exception_cls=Exception):
        """serializer-level validation"""
        exist_parent = self._validate_only_fields.get("exist_parent", "")
        new_parent = self._validate_only_fields.get("new_parent", "")
        if not exist_parent in EMPTY_VALUES:
            if isinstance(exist_parent, str) and exist_parent.isdigit():
                exist_parent = int(exist_parent)
            v = SelectIDsExistValidator(
                model_cls=self.Meta.model, err_field_name="exist_parent"
            )
            v(exist_parent)
        elif not new_parent in EMPTY_VALUES:
            if not isinstance(new_parent, int) and not new_parent.isdigit():
                raise exception_cls({"new_parent": "it must be integer"})
        self._validate_only_fields["exist_parent"] = (
            "" if exist_parent in EMPTY_VALUES else str(exist_parent)
        )
        value.update(
            self._validate_only_fields
        )  # for later sorting operation at parent
        if _logger:
            log_msg = ["_validate_only_fields", self._validate_only_fields]
            _logger.debug(None, *log_msg)
        return value

    def create(self, validated_data):
        exist_parent = validated_data.pop("exist_parent", "")
        new_parent = validated_data.pop("new_parent", "")
        with self.atomicity():
            instance = super().create(validated_data=validated_data)
            # maintain group hierarchy
            closure_tree = self.parent.get_insertion_ancestors(
                leaf_node=instance, exist_parent=exist_parent, new_parent=new_parent
            )
            self.fields["ancestors"].create(validated_data=closure_tree)
        return instance

    def update(self, instance, validated_data):
        exist_parent = validated_data.pop("exist_parent", "")
        new_parent = validated_data.pop("new_parent", "")
        closure_tree = validated_data.pop("closure_tree", None)
        with self.atomicity():
            instance = super().update(instance=instance, validated_data=validated_data)
            # For bulk update on group hierarchy, it's good to explicitly & separately specify
            # which nodes (of closure table) should be created / updated / deleted
            if closure_tree:
                self.fields["ancestors"].update(
                    instance=closure_tree["update"]["obj"],
                    validated_data=closure_tree["update"]["data"],
                )
                self.fields["ancestors"].create(validated_data=closure_tree["create"])
        return instance
