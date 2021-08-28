import operator
import urllib.parse
import json
import logging
from functools import reduce
from datetime import datetime, timezone

from django.core.exceptions import FieldError as DjangoFieldError
from django.db.models import Q, Prefetch
from django.db.models.constants import LOOKUP_SEP
from rest_framework.filters     import BaseFilterBackend, OrderingFilter, SearchFilter
from rest_framework.exceptions  import ValidationError as RestValidationError, ErrorDetail as RestErrorDetail

_logger = logging.getLogger(__name__)


class SimpleRangeFilter(BaseFilterBackend):
    """
    light-weight version of search filter to look for range of any value
    (e.g. integer, float, datetime) as an alternative of `django_filter` library.
    Limitation:
        * complicated nested condition is NOT supported in this class
        * all boolean operators bewteen two different terms are either AND or OR
          e.g.
          ((age > 7) AND (age < 18)) AND (score < 65) AND (work_done >= 18)
          ((age > 7) AND (age < 18)) OR  (score < 65) OR  (work_done >= 18)

          you can also customize the condition , determine the order of each (nested)
          subcondition :
          ((age > 7) AND (age < 18)) AND (score < 65) OR  (work_done >= 18)

        * boolean operator between different surfix within the same term should be all AND or OR
          e.g.
          ((age > 7) AND (age < 18))
    """
    search_param = ['range_term',]
    _inclusive_keywords = ['gt', 'gte', 'lt', 'lte']

    def get_search_fields(self, view):
        # search_field_map is required for mapping from query parameter of http request to
        # field name of custom model backend
        search_field_map = getattr(view, 'search_field_map', None)
        assert  search_field_map, "search_field_map must be provided to use this filter"
        return  search_field_map

    def get_search_terms(self, request):
        params = []
        log_args = []
        for prefix in self.search_param:
            for surfix in self._inclusive_keywords:
                param_name = '%s%s%s' % (prefix, LOOKUP_SEP, surfix)
                param = request.query_params.get(param_name, None)
                try:
                    if param:
                        param = self.normalize(param)
                        params.append({'prefix':prefix, 'surfix':surfix, 'value':param})
                except (ValueError, TypeError) as e: # report then discard
                    log_args.extend(['error_param', (prefix, surfix, param, e)])
        if any(log_args):
            _logger.warning(None, *log_args, request=request)
        return params

    def construct_search(self, terms, field_map):
        out = {}
        map_keys = field_map.keys()
        for term_set in terms:
            prefix = term_set['prefix']
            if not prefix in map_keys:
                continue # discard the term_set due to mapping failure
            if out.get(prefix, None) is None:
                _operator   = field_map[prefix]['operator']
                out[prefix] = {'operator': _operator,'list':[]}
            _mapped_prefix = field_map[prefix]['field_name']
            lookup_fieldname = '%s%s%s' % (_mapped_prefix, LOOKUP_SEP, term_set['surfix'])
            condition = {lookup_fieldname: term_set['value']}
            out[prefix]['list'].append(condition)
        return out

    def filter_queryset(self, request, queryset, view):
        search_field_map = self.get_search_fields(view)
        search_terms = self.get_search_terms(request)
        log_args = ['search_field_map', search_field_map, 'search_terms', search_terms]
        if not search_field_map or not search_terms:
            _logger.debug(None, *log_args, request=request)
            return queryset
        orm_lookups = self.construct_search(terms=search_terms , field_map=search_field_map)
        base = queryset
        conditions = []
        for lookup in orm_lookups.values():
            queries = [Q(**cond) for cond in lookup['list']]
            conditions.append(reduce(lookup['operator'] , queries))
        conditions = self.combine_terms_conditions(conditions)
        log_args.extend(['orm_lookups', orm_lookups, 'conditions', conditions])
        _logger.debug(None, *log_args, request=request)
        queryset = queryset.filter(conditions)
        return queryset

    def normalize(self, param):
        raise NotImplementedError

    def combine_terms_conditions(self, conditions):
        """
        subclasses can customize final condtion by overriding this function
        e.g. all the operators could also be OR, or specify the order of sub-condition
        with custom set of boolean operators
        """
        return reduce(operator.and_, conditions)

#### end of SimpleRangeFilter


class DateTimeRangeFilter(SimpleRangeFilter):
    search_param = ['date']
    def normalize(self, param):
        return datetime.fromisoformat(param)


class ClosureTableFilter(BaseFilterBackend):
    def filter_queryset(self, request, queryset, view):
        # filter out the instance whose depth = 0 only in read view
        closure_model_cls = getattr(view, 'closure_model_cls', None)
        if closure_model_cls is None:
            return queryset
        closure_qset = closure_model_cls.objects.filter(depth__gt=0)
        field_names  = request.query_params.get('fields', '').split(',')
        prefetch_objs = []
        if 'ancestors' in field_names :
            prefetch_objs.append(Prefetch('ancestors',   queryset=closure_qset))
        if 'descendants' in field_names :
            prefetch_objs.append(Prefetch('descendants', queryset=closure_qset))
        queryset = queryset.prefetch_related( *prefetch_objs )
        ####err_args = ["low_level_prefetch_query", queryset.query] # TODO, find better way to log raw SQL
        ####_logger.debug(None, *err_args, request=request)
        return queryset

class  AggregateFieldOrderingFilter(OrderingFilter):
    _aggregate_fields  = {}
    @classmethod
    def mirror(cls):
        # double the key set, use the same corresponding value for ordering rule in Django ORM.
        # e.g. `field_name` defines ascending order , while `-field_name` defines descending order
        ag = cls._aggregate_fields
        negate = {}
        for k,v in ag.items():
            newkey = k[1:] if k.startswith('-') else '-%s' % k
            negate[newkey] = v
        ag.update(negate)

    def filter_queryset(self, request, queryset, view):
        ordering = self.get_ordering(request, queryset, view)
        # currently the support of multiple aggregate in one queryset is limited
        if ordering:
            aggregate_included = set(ordering) & set(self._aggregate_fields.keys())
            aggregate_fields  = {k:v[1] for k,v in self._aggregate_fields.items() if k in aggregate_included}
            if aggregate_fields:
                order_substitute  = {k:v[0] for k,v in self._aggregate_fields.items() if k in aggregate_included}
                ordering = map(lambda v:order_substitute[v] if v in order_substitute.keys() else v , ordering)
                ordering = list(ordering)
                queryset = queryset.annotate( **aggregate_fields )
            #import pdb
            #pdb.set_trace()
            return queryset.order_by(*ordering)
        return queryset


class AbstractAdvancedSearchMixin:
    enable_advanced_search_param = "advanced_search"
    req_advsearch_cond_param = "adv_search_cond"

    UNLIMITED_OPERANDS = -1

    operator_map = {
        'boolean': {
            'and': (operator.and_, UNLIMITED_OPERANDS),
            'or' : (operator.or_ , UNLIMITED_OPERANDS),
            'not': (operator.not_, 1)
        },
        #'bitwise': {
        #    '&' :(operator.and_ , 2),
        #    '|' :(operator.or_, 2),
        #    '~' :(operator.inv , 1),
        #},
        'comparison':{
            '==':(operator.eq , 2), # for string / number comparison
            '!=':(operator.ne , 2), # for string / number comparison
            'contains':(operator.contains , 2), # for string comparison / list item lookup
            '<' :(operator.lt , 2),
            '>' :(operator.gt , 2),
            '<=':(operator.le , 2),
            '>=':(operator.ge , 2),
        },
    }

    def get_exception(self, **kwargs):
        raise NotImplementedError()

    def is_enabled_adv_search(self, request):
        """
        check whether client attempts to enable advanced search function,
        return the check result.
        """
        raise NotImplementedError()

    def get_adv_condition(self, request):
        """
        This is where you retrieve the source of advanced search condition, mostly
        it is a serialized JSON data in body section of client request,
        or it could be in the URL query parameter of the client request.
        """
        raise NotImplementedError()

    def get_advanced_search_condition(self, request):
        enable = self.is_enabled_adv_search(request=request)
        if enable:
            cond = self.get_adv_condition(request=request)
            if not isinstance(cond, (dict, list,)):
                try:  # de-serialize the seach condition placed in request body
                    cond = json.loads(cond)
                except json.JSONDecodeError as je:
                    err_msg = 'JSON decode error, msg:%s, pos:%s' % (je.msg, je.pos)
                    raise self.get_exception(err_msg=err_msg)
            return cond

    def _parse_condition(self, condition):
        """
        Argument `condition` is a tree structure that may contain comparison operators
        (>=, <=, >, <, ==, etc.) and boolean operators (and, or, &, |, etc.).
        If boolean operator is found in current node, it must be non-leaf node
        then the current node visits all the child nodes by recursively invoking this
        function ; if comparison operator is found in current node, then it must be
        leaf node.

        Each node contains at least 2 fields :
            * `operator` : for calculating the condition, subclasses can extend the
                valid operators to meet application requirement
            * `operands` : a list of operands acceptable for the `operator` field,
                non-leaf node refers to `operands` field as all its child nodes.

        Optional field in a node :
            * `metadata` : provided as extra information to calculate condition for application

        The example of condition tree may look like :

            {"operator": "or",
                "operands":[
                    {
                        "operator":"==",
                        "operands":["max_volt", 7.2],
                        "metadata": {"dtype": 4}
                    },
                    {
                        "operator":"contains",
                        "operands":["attributes__value", "w"],
                        "metadata": {"dtype": 1}
                    }
                ]
            }
        """
        _operator = condition['operator']
        assert any(condition['operands']), "empty operands are NOT allowed for the operator `%s`" % _operator
        chosen_op_type  = None
        chosen_operator = None
        for op_type, op_opt in self.operator_map.items():
            op_tuple = op_opt.get(_operator, None)
            if not op_tuple:
                continue
            assert op_tuple[1] == len(condition['operands']) or op_tuple[1] == self.UNLIMITED_OPERANDS, \
                "Fail to parse condition string, due to incorrect number of operands"
            chosen_op_type = op_type
            chosen_operator = op_tuple
            break
        if chosen_op_type == 'boolean':
            parsed = [self._parse_condition(operand) for operand in condition['operands']]
            parsed = self._create_nonleaf_node(_operator=chosen_operator, operands=parsed,
                    metadata=condition.get('metadata', None))
        elif chosen_op_type == 'comparison':
            parsed = self._create_leaf_node(_operator=chosen_operator, operands=condition['operands'],
                    metadata=condition.get('metadata', None))
        else:
            raise TypeError("parsing error, unknow type `%s`" % _operator)
        return parsed

    def _create_leaf_node(self, _operator, operands, metadata=None):
        """
        Create and return a new leaf node.
        Subclasses are free to determine the order of the operands
        Example of a leaf node :
            {
                "operator":"==",
                "operands":["max_volt", 7.2],
                "metadata": {"dtype": 4}
            }
        """
        raise NotImplementedError

    def _create_nonleaf_node(self, _operator, operands, metadata=None):
        """
        Create and return a new non-leaf node.
        Example of a non-leaf node :
            {"operator": "or",
                "operands":[
                    { ... nested condition , as child node ... },
                    { ... nested condition , as child node ... }
                ]
            }
        """
        raise NotImplementedError
## end of class AbstractAdvancedSearchMixin


class AdvancedSearchFilter(SearchFilter, AbstractAdvancedSearchMixin):
    """
    extend original DRF search function (grab keywords only from URL query parameters)
    to perform more advanced search condition placed within inbound request.
    """
    _django_q_lookup_map = {
        operator.eq: 'exact',
        operator.ne: 'exact',
        operator.contains: 'contains',
        operator.lt: 'lt' ,
        operator.gt: 'gt' ,
        operator.le: 'lte',
        operator.ge: 'gte',
    }

    def get_exception(self, **kwargs):
        err_msg = kwargs.pop('err_msg', '')
        err_detail = {self.enable_advanced_search_param: [RestErrorDetail(err_msg)] }
        excpt = RestValidationError(detail=err_detail)
        return excpt

    def is_enabled_adv_search(self, request):
        enable = request.query_params.get(self.enable_advanced_search_param, '')
        unprintable_chars = [chr(idx) for idx in range(0x10)]
        for u_char in unprintable_chars:
            enable = enable.replace(u_char, '')  # strip unprintable characters
        return enable

    def get_adv_condition(self, request):
        body = request.body
        # note that HTTP protocol doesn't strictly deny GET request with body
        # content, however JavaScript does, which is not convenient in web
        # frontend apps, the only way for web frontend to acheive this is
        # to URL-encode the (JSON) serialized advanced condition, then set
        # it as URL query parameter
        if not body:
            body = request.query_params.get(self.req_advsearch_cond_param, '')
            # decode URI parameter
            body = urllib.parse.unquote(body)
        return body

    def _create_leaf_node(self, _operator, operands, metadata=None):
        """
        In Django, Q object is applied to this advanced search filter because
        Q object is designed for complex search at ORM layer.
        """
        err_msg = "only 2 operands can be passed into Django Q object, but received %s"
        assert len(operands) == 2, err_msg % (operands)
        err_msg = "unknown operator `%s` at leaf node"
        assert _operator[0] in self._django_q_lookup_map.keys(), err_msg % _operator[0]
        lookup_cmd = self._django_q_lookup_map[_operator[0]]
        key = [operands[0] , lookup_cmd,]
        value = operands[1]
        key = LOOKUP_SEP.join(key)
        out = Q(**{key:value})
        if _operator[0] is operator.ne:
            out = ~out
        return out

    def _create_nonleaf_node(self, _operator, operands, metadata=None):
        """
        Make use of boolean operator supported in Q objects, no need to
        create new non-leaf node
        """
        return reduce(_operator[0], operands)

    def filter_queryset(self, request, queryset, view):
        err_msg = None
        queryset = super().filter_queryset(request=request, queryset=queryset, view=view)
        advanced_cond = self.get_advanced_search_condition(request=request)
        if advanced_cond:
            try:
                parsed_cond = self._parse_condition(condition=advanced_cond)
                queryset = queryset.filter(parsed_cond)
                queryset = queryset.distinct() # reduce duplicates
            except DjangoFieldError as e:
                # do not expose valid field names on DjangoFieldError
                _stop_from = "Choices are"
                idx = e.args[0].find(_stop_from)
                err_msg = e.args[0][:idx]
            except Exception as e:
                err_msg = "%s , %s" % (type(e).__name__ , e)
            else:
                pass # succeed to perform advanced search
            if err_msg:
                log_args = ['advanced_cond', advanced_cond, 'err_msg', err_msg]
                _logger.info(None, *log_args, request=request)
                raise self.get_exception(err_msg=err_msg)
        return queryset


