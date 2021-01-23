import json
import logging

from django.db.models import Q
from django.db.models.constants import LOOKUP_SEP
from elasticsearch import Elasticsearch

_logger = logging.getLogger(__name__)


def get_dsl_template(path):
    read_dsl = None
    with open(path, 'r') as f:
        read_dsl = json.load(f)
    assert read_dsl, "failed to load DSL query from file"
    return read_dsl


def _get_es_client(secrets_path):
    secrets = get_dsl_template(path=secrets_path)
    username = secrets['elasticsearch']['username']
    password = secrets['elasticsearch']['password']
    nodes = secrets['elasticsearch']['nodes']
    nodes = ['%s:%s' % (node['host'], node['port']) for node in nodes]
    return Elasticsearch(nodes, http_auth=(username, password), use_ssl=False, maxsize=4, timeout=12 )

es_client = _get_es_client(secrets_path='./common/data/secrets.json')


class ElasticSearchQuerySet:
    dsl_template_path = None
    index = None
    doc_type = None
    _skip_edit_dsl_template = False

    def __init__(self, request, paginator):
        self._es = es_client
        self._query_params = request.query_params
        self._start_pos = -1
        self._end_pos = -1
        self._filtered_page_size = paginator.get_page_size(request)
        self._page_query_param = paginator.page_query_param
        # note that how these two containers are used for condition estimate is determined by
        # all subclasses implemented on application side, it highly depends on the DSL template
        # applied in each subclass.
        self._condition_kwargs = {}
        self._condition_args = []

    def __getitem__(self, index):
        if not isinstance(index, (int, slice)):
            raise TypeError(
                'UserActionModel indices must be integers or slices, not %s.'
                % type(index).__name__
            )
        log_args = ['index', index]
        _logger.debug(None, *log_args)
        result = self.load()
        assert self._start_pos == index.start, 'value mismatch, self._start_pos = %s, index.start = %s' \
            % (self._start_pos, index.start)
        assert self._end_pos >= index.stop, 'value mismatch, self._end_pos = %s, index.stop = %s' \
            % (self._end_pos, index.stop)
        out = result['hits']['hits']
        # since I only fetch just enough data from es, the start and stop position of index slice must be adjusted
        out = out[0 : index.stop - index.start]
        return out

    def __len__(self):
        result = self.load()
        return len(result['hits']['hits']) if result['hits'].get('hits', None)  else 0

    def count(self):
        """
        will be accessed by Django Paginator, this function mimics django's QuerySet.count()
        """
        result = self.load()
        return result['hits']['total']

    def ordered(self): # will be accessed by Django Paginator
        return True

    def load(self):
        if hasattr(self, '_result_cache'):
            return self._result_cache
        self._get_load_range()
        read_dsl = get_dsl_template(path=self.dsl_template_path)
        self.edit_dsl_template(read_dsl=read_dsl)
        result = self._search(index=self.index, doc_type=self.doc_type, read_dsl=read_dsl)
        self._result_cache = result
        return result

    def edit_dsl_template(self, read_dsl):
        """
        entry function for subclasses customizing the DSL query from template
        """
        if self._skip_edit_dsl_template is False:
            raise NotImplementedError

    def _search(self, index, doc_type, read_dsl, filter_path=None):
        """ wrapper to perform low-level query operation from elasticsearch """
        defualt_filter_path = ['_shards','timed_out','hits.total', 'hits.hits._id', 'hits.hits._source']
        filter_path = filter_path or defualt_filter_path
        if not isinstance(read_dsl, str):
            read_dsl = json.dumps(read_dsl)
        result = self._es.search(index=index, doc_type=doc_type, body=read_dsl, filter_path=filter_path)
        return result

    def _get_load_range(self):
        page_number = self._query_params.get(self._page_query_param, '')
        page_number = int(page_number) if page_number.isdigit() else 1
        start_pos = (page_number - 1) *  self._filtered_page_size
        end_pos   = start_pos + self._filtered_page_size
        self._start_pos = start_pos
        self._end_pos = end_pos
        return start_pos, end_pos

    def filter(self, *args, **kwargs):
        self._condition_kwargs.update(kwargs)
        self._condition_args.extend(args)
        log_args = ['args', args, 'kwargs', kwargs]
        _logger.debug(None, *log_args) # TODO, deepcopy self then output
        return self

    def generate_subclause(self, *args, **kwargs):
        raise NotImplementedError

    def _parse_leaf_q(self, cond):
        lookup_map = {
            'gt':'range', 'lt':'range', 'gte':'range', 'lte':'range',
            'icontains': 'match', 'contains': 'match', 'iexact': 'term', 'exact': 'term',
        }
        lookup  = cond[0].split(LOOKUP_SEP)
        lookup_type = lookup[-1]
        out = {}
        clause_type = lookup_map.get(lookup_type, None)
        if clause_type:
            fieldname = LOOKUP_SEP.join( lookup[:-1] )
            fieldvalue = cond[1]
            out[clause_type] = self.generate_subclause(clause_type=clause_type, lookup_type=lookup_type,
                    fieldname=fieldname, fieldvalue=fieldvalue)
        return out

    def _parse_nonleaf_q(self, cond):
        out = []
        for child in cond.children:
            if isinstance(child, Q):
                item = self._parse_nonleaf_q(cond=child)
            else:
                item = self._parse_leaf_q(cond=child)
            if item:
                out.append(item)
        if len(out) > 1:
            items = out
            if cond.connector == 'AND':
                out = {'bool': {'must': items}}
            elif cond.connector == 'OR':
                out = {'bool': {'should': items}}
        elif len(out) == 1:
            out = out[0]
        return out

    def parse_filter_args(self, container:list):
        #### self._condition_kwargs
        for cond in self._condition_args:
            if not isinstance(cond, Q):
                continue # discard
            extra_cond = self._parse_nonleaf_q(cond=cond)
            if any(extra_cond):
                container.append(extra_cond)



