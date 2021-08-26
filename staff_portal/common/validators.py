from collections.abc import Iterable
import operator
import logging

from django.core.exceptions import ValidationError
from django.utils.deconstruct import deconstructible
from django.db.models import Model as DjangoModel

from common.util.python       import string_unprintable_check
from common.util.python.graph import is_graph_cyclic

_logger = logging.getLogger(__name__)


@deconstructible
class ClosureSingleTreeLoopValidator:
    """
    A given tree T2 will form a loop if :
    * the entire T2 becomes a child of a given node T1_n.
    * T1_n is one of T2's descendants
    Pre-requisite :
        * Both T2 and T1_n come from the same database closure table.
    """
    def __init__(self, **kwargs):
        self.T2_root_id    = kwargs['T2_root_id']
        self.closure_model = kwargs['closure_model']
        self.ancestor_column_name   = kwargs["ancestor_column_name"]
        self.descendant_column_name = kwargs["descendant_column_name"]
        self.err_field_name = kwargs.pop('err_field_name', None)

    def __call__(self, value):
        T1_node_id = value
        filter_dict = {}
        filter_dict[self.ancestor_column_name] = self.T2_root_id
        filter_dict[self.descendant_column_name] = T1_node_id
        T1_T2_path_cnt = self.closure_model.objects.filter(**filter_dict).count()
        if T1_T2_path_cnt > 0:
            log_msg = ['filter_dict', filter_dict, 'T1_node_id', T1_node_id,
                    'T2_root_id', self.T2_root_id, 'T1_T2_path_cnt', T1_T2_path_cnt]
            _logger.info(None, *log_msg)
            err_msg = ["Loop detected, the selected parent of the node (ID ", str(self.T2_root_id), \
                    ") must NOT be its descendant (ID ", str(T1_node_id) ,")"]
            err_msg = "".join(err_msg)
            if self.err_field_name:
                err_msg = {self.err_field_name : err_msg}
            raise ValidationError(err_msg)


@deconstructible
class TreeNodesLoopValidator:
    """
    Given a set of tree nodes, each pair P represents an edge, with 2 nodes (N0, N1),
    . These edges form a loop if :
        * certain edge P1 is found amongs all edges , which makes loop starting from
          node  P1_N0 --> P1_N1 -->
                P2_N0 --> P2_N1 -->
                P3_N0 --> P3_N1 -->
                ......
                Px_N0 --> Px_N1 --> P1_N0
    This validator doesn't need to work with ORM, DFS (Depth-First-Search) is required
    """
    NOT_UPDATE_YET = -1
    ROOT_OF_TREE = -2
    # error message template
    default_err_msg = "The hierarchy tree contains a cycle in the list of nodes : {loop_node_list} ."

    def __init__(self, **kwargs):
        self._err_msg_cb = kwargs.pop('err_msg_cb', None)
        tree_edge = kwargs['tree_edge']
        self._graph = self._build_graph(tree_edge)
        self._process_nodes_inbound()

    def _build_graph(self, tree_edge):
        """ build graph from the given edges """
        log_msg = ['tree_edge', tree_edge,]
        nodes = {}
        for n0, n1 in tree_edge:
            if n0 == n1:
                log_msg.extend(['n0', n0, 'n1', n1, 'msg', 'self-directed edge NOT allowed'])
                _logger.warning(None, *log_msg)
                err_msg = ["self-directed edge at (", str(n0), ",", str(n1), ") is NOT allowed"]
                raise ValueError("".join(err_msg))
            if not nodes.get(n1):
                nodes[n1] = {'outbound':[], 'inbound': self.NOT_UPDATE_YET}
            if isinstance(n0, str): # if it's digit, it must be negative integer (ROOT_OF_TREE or NOT_UPDATE_YET)
                nodes[n1]['inbound'] = {'path_len':1, "ID": n0}
                if not nodes.get(n0):
                    nodes[n0] = {'outbound':[], 'inbound': self.NOT_UPDATE_YET}
                nodes[n0]['outbound'].append(n1)
            else:
                nodes[n1]['inbound'] = self.ROOT_OF_TREE
        log_msg.extend(['nodes', nodes])
        _logger.debug(None, *log_msg)
        return nodes

    def _process_nodes_inbound(self):
        for key, node in self._graph.items():
            if isinstance(node['inbound'], dict):
                node['inbound'] = [node['inbound']['ID']]
            else: # self.ROOT_OF_TREE , self.NOT_UPDATE_YET
                node['inbound'] = []


    def __call__(self, caller=None):
        is_directed = False # treated as undirected graph
        result, loop_node_list = is_graph_cyclic(self._graph, is_directed)
        if result:
            if self._err_msg_cb :
                err_msg = self._err_msg_cb(loop_node_list)
            else:
                err_msg = self.default_err_msg.format(loop_node_list=str(loop_node_list))
            log_msg = ['graph', self._graph, 'is_directed', is_directed, 'err_msg', err_msg]
            _logger.info(None, *log_msg)
            raise ValidationError(err_msg)


@deconstructible
class ClosureCrossTreesLoopValidator(TreeNodesLoopValidator):
    """
    Given a set of distinct trees T1, T2, T3 .... Tx will form a loop if :
    * Tree T' is found among the given trees, which makes a loop starting from
    T'
    Pre-requisite :
        * all given trees must come from the same database closure table.
    """

    def __init__(self, **kwargs):
        self.closure_model = kwargs['closure_model']
        self.depth_column_name   = kwargs["depth_column_name"]
        self.ancestor_column_name   = kwargs["ancestor_column_name"]
        self.descendant_column_name = kwargs["descendant_column_name"]
        super().__init__(**kwargs)


    def _build_graph(self, tree_edge):
        """
        build the simplified directed graph, by extending source node of each given edge E0 in
        edge set `tree_edge`, find if there's destination node of another edge E1 which :
        * is in the given edge set `tree_edge`
        * can visit source node of edge E0, through limited amount of edges.
        This validator runs with specified closure-table model.
        """
        nodes = super()._build_graph(tree_edge)
        log_msg = []
        for key, node in nodes.items():
            if (node['inbound'] == self.ROOT_OF_TREE) or  (isinstance(node['inbound'],dict)
                    and node['inbound']['path_len'] == 1):
                continue # already done, no need to update the parent of current node key
            # list all ancestors of current node in ascending order
            filter_dict = { self.descendant_column_name: key,
                            "{depth}__gt".format(depth=self.depth_column_name) : 0}
            query = self.closure_model.objects.values(self.depth_column_name, self.ancestor_column_name
                    ).filter(**filter_dict).order_by(self.depth_column_name)
            if len(query) == 0:
                node['inbound'] = self.ROOT_OF_TREE
                continue
            log_msg.extend(['updating_node_key', key])
            for q in query: # loop through each ancestor of current node
                #### print(".... q : "+ str(q))
                parent_key = str(q[self.ancestor_column_name])
                # this function only looks for the ancestor which is present in the graph
                if (nodes.get(parent_key, None) is None):
                    continue
                path_len = q[self.depth_column_name]
                if node['inbound'] == self.NOT_UPDATE_YET:
                    node['inbound'] = {'path_len':path_len, "ID": parent_key}
                    nodes[parent_key]['outbound'].append(key)
                    break
                else: # TODO: is it possible to allow 2 parent nodes connecting to the same node ?
                    err_msg = "found second inbound node from %s to %s" % (parent_key, key)
                    log_msg.extend(['err_msg', err_msg])
                    _logger.error(None, *log_msg)
                    raise ValueError(err_msg)
                # elif node['inbound']['path_len'] > path_len:
                #     node['inbound']['path_len'] = path_len
                #     node['inbound']["ID"] =  parent_key
                #     nodes[parent_key]['outbound'].append(key)
        log_msg.extend(['nodes', nodes])
        _logger.debug(None, *log_msg)
        return nodes


class NumberBoundaryValidator:
    requires_context = False
    _comparison_fn = {
        (False, False): operator.lt,
        (False, True) : operator.le,
        (True, False) : operator.gt,
        (True, True)  : operator.ge,
    }
    _error_msg_pattern = "given value:%s, limit:%s, operator:%s"

    def __init__(self, limit, larger_than:bool, include:bool):
        self.limit = limit
        self.larger_than = larger_than
        self.include = include

    def __call__(self, value):
        key = (self.larger_than, self.include)
        chosen_fn = self._comparison_fn[key]
        if not chosen_fn(value, self.limit):
            err_msg = self._error_msg_pattern % (value, self.limit, chosen_fn.__name__)
            raise ValidationError(err_msg)


class UnprintableCharValidator:
    _error_msg_pattern = "unprintable characters found in given value list: %s"

    def __init__(self, extra_unprintable_set):
        self.extra_unprintable_set = extra_unprintable_set

    def __call__(self, value):
        unprintables = []
        if isinstance(value, str):
            value = [value]
        for v in value:
            unprintable = string_unprintable_check(v, self.extra_unprintable_set)
            if unprintable:
                unprintables.append(v)
        if any(unprintables):
            err_msg = self._error_msg_pattern % ', '.join(unprintables)
            raise ValidationError(err_msg)


@deconstructible
class EditFormObjIdValidator:
    requires_context = True
    """
    check whether ID of each form matches ID of each instance in REST framework serializer,
    This validator can only be used in bulk update form scenario
    """
    def __init__(self, **kwargs):
        pass

    def __call__(self, value, caller):
        if (not isinstance(value, Iterable)) or (caller.instance is None) or (not isinstance(caller.instance, Iterable)):
            err_msg = "The argument `value` should be a list of form data, and `caller.isntance` \
                       should be a list of object instances associated with each form."
            raise TypeError(err_msg)
        assert hasattr(caller, 'instance_ids'), 'lack of property `instance_ids` on caller %s' % type(caller)
        form_ids = caller.extract_form_ids(formdata=value, include_null=False)
        obj_ids  = caller.instance_ids # should be framework independent
        diff = set(form_ids).symmetric_difference(obj_ids)
        if any(diff):
            log_msg = ['diff', diff, 'value', value, 'obj_ids', obj_ids, 'form_ids', form_ids,]
            _logger.info(None, *log_msg)
            err_msg = "The IDs in client form doesn't match IDs in object instances, the list shows the differnece: %s" % diff
            raise ValidationError(err_msg)


@deconstructible
class SelectIDsExistValidator:
    def __init__(self, **kwargs):
        self.model_cls = kwargs.get('model_cls', None)
        self.queryset = kwargs.get('queryset', None)
        self.err_field_name = kwargs.get('err_field_name', None)
        if not self.model_cls and not self.queryset:
            err_msg = "Either of the arguments `model_cls` and `queryset` has to be provided."
            log_msg = ['err_msg', err_msg]
            _logger.warning(None, *log_msg)
            raise ValueError(err_msg)

    def __call__(self, value):
        # the argument -- value here has to be a list of IDs
        # note that application callers have to handle data type of ID
        # or primary key in advance before running this validator
        if not isinstance(value ,list):
            value = [value]
        qset = self.queryset
        if not qset:
            try:
                qset = self.model_cls.objects.filter(pk__in=value)
            except ValueError:
                err_msg = "The list of ID values contains incorrect data type: %s" % (value)
                log_msg = ['err_msg', err_msg]
                _logger.info(None, *log_msg)
                if self.err_field_name:
                    err_msg = {self.err_field_name:err_msg}
                raise ValidationError(err_msg)
        diff = set(value) - set(qset.values_list('pk', flat=True))
        #### if len(value) != qset.count():
        if len(diff) > 0:
            err_msg = "There must be non-existing ID in the list : %s" % (diff)
            log_msg = ['err_msg', err_msg]
            _logger.info(None, *log_msg)
            if self.err_field_name:
                err_msg = {self.err_field_name:err_msg}
            raise ValidationError(err_msg)


@deconstructible
class UniqueListItemsValidator:
    err_msg_pattern = '{"message":"duplicate item found in the list","field":"%s","value":%s}'

    def __init__(self, fields:list, error_cls=ValidationError):
        self._unique_fields_among_children = fields
        self._error_cls = error_cls

    def __call__(self, value):
        for fname in self._unique_fields_among_children :
            fvalue = [q[fname].pk if isinstance(q[fname], DjangoModel) else q[fname] for q in value]
            if len(fvalue) != len(set(fvalue)):
                err_msg = self.err_msg_pattern  % (fname, fvalue)
                log_msg = ['err_msg', err_msg]
                _logger.info(None, *log_msg)
                raise  self._error_cls(err_msg)


