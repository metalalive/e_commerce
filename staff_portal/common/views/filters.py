import operator
import logging
from functools import reduce
from datetime import datetime, timezone

from django.db.models import Q
from django.db.models.constants import LOOKUP_SEP
from rest_framework.filters     import BaseFilterBackend

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


