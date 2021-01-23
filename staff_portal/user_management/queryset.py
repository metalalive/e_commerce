import json
import logging

from rest_framework import status as RestStatus

from common.util.elasticsearch import ElasticSearchQuerySet

_logger = logging.getLogger(__name__)


class UserActionSet(ElasticSearchQuerySet):
    """
    implement iterable model-like object with elasticsearch backend
    """
    _essential_keys = ['action', 'profile_id', 'affected_items']
    dsl_template_path = 'user_management/data/dsl_action_history_template.json'
    index = 'log-*'
    doc_type = 'app_server'

    def _represent_delete(self, kv_pairs, doc):
        affected_items = kv_pairs['affected_items']
        affected_items = affected_items.replace('\'', '"')
        doc['affected_objs'] = json.loads(affected_items)
        return True

    def _represent_recover(self, kv_pairs, doc):
        http_status = int(kv_pairs['http_status'])
        doc['http_status'] = http_status
        if http_status == RestStatus.HTTP_200_OK :
            return self._represent_delete(kv_pairs, doc)
        else: # 410
            doc['affected_objs'] = []
            return True

    def _represent_login(self, kv_pairs, doc):
        doc['result'] = kv_pairs['result']
        return True

    def _represent_dummy(self, kv_pairs, doc):
        return True

    _valid_actions = {
        'create'   : _represent_delete,
        'update'   : _represent_delete,
        'delete'   : _represent_delete,
        'recover'  : _represent_recover,
        'login'    : _represent_login,
        'logout'   : _represent_dummy,
        'recover_username'  : _represent_dummy,
        'reset_password'    : _represent_delete,
        'update_username'   : _represent_delete,
        'update_password'   : _represent_delete,
        'deactivate_account': _represent_delete,
        'reactivate_account': _represent_delete,
    }

    def __init__(self, request, paginator):
        self._username = request.user.username
        self._profile = request.user.genericuserauthrelation.profile
        super().__init__(request=request, paginator=paginator)

    def generate_subclause(self, clause_type, lookup_type, fieldname, fieldvalue):
        if fieldname == 'timestamp':
            fieldname = '@timestamp'
        if clause_type == 'range':
            fieldvalue = fieldvalue.isoformat() + 'Z'
            out = {fieldname: {lookup_type : fieldvalue}}
        else:
            out = {fieldname: fieldvalue}
        return out

    def edit_dsl_template(self, read_dsl):
        # must-have default setup
        read_dsl['from'] = self._start_pos
        read_dsl['size'] = self._filtered_page_size
        identity_field  = read_dsl['query']['bool']['must'][2]['bool']['should']
        identity_field[0]['nested']['query']['bool']['must'][1]['match']['msg_kv_pairs.value'] = self._username
        identity_field[1]['nested']['query']['bool']['must'][1]['term']['msg_kv_pairs.value'] = self._profile.pk
        # extra condition set by user
        top_level_cond = read_dsl['query']['bool']['must']
        extra_conds = []
        self.parse_filter_args(container=extra_conds)
        top_level_cond.extend(extra_conds)
        log_args = ['extra_conds', extra_conds]
        _logger.debug(None, *log_args)

    def load(self):
        if hasattr(self, '_result_cache'):
            return self._result_cache
        result = super().load()
        self._representation(result=result)
        self._result_cache = result

        log_args = ['fetched_data', result, 'start_pos', self._start_pos, 'end_pos', self._end_pos,
                'profile_id', self._profile.pk, 'filtered_page_size', self._filtered_page_size]
        _logger.debug(None, *log_args)
        return result


    def _representation(self, result):
        hit_data = result['hits']
        hit_records = hit_data.get('hits', None)
        if hit_records is None:
            return
        err_args = []
        log_args = []
        for doc in hit_records:
            doc_id = doc.pop('_id')
            src = doc.pop('_source')
            err_msg = None
            kv_pairs = {pair['key']:pair['value'] for pair in src['msg_kv_pairs']}
            # check whether the record has all essential key-value pairs
            lack = set(self._essential_keys) - set(kv_pairs.keys())
            _action = kv_pairs.get('action')
            if not _action in ('delete','create','update','recover'):
                lack -= set(['affected_items'])
            elif _action == 'recover':
                http_status = kv_pairs.get('http_status', RestStatus.HTTP_400_BAD_REQUEST)
                http_status = int(http_status)
                if http_status >= RestStatus.HTTP_400_BAD_REQUEST:
                    lack -= set(['affected_items'])
            if _action in ('login', 'logout'):
                # login-failure record does NOT contain `profile_id` field
                lack -= set(['profile_id'])
            if any(lack):
                err_msg = 'doc_id = %s, essential fields not found : %s' % (doc_id, ','.join(lack))
            elif not _action in self._valid_actions.keys() :
                err_msg = 'doc_id = %s, invalid action : %s' % (doc_id, _action)
            if err_msg:
                err_args.extend(['err_msg', err_msg])
                continue
            succeed = self._valid_actions[_action](self, kv_pairs=kv_pairs, doc=doc)
            if succeed:
                doc['timestamp'] = src['@timestamp']
                doc['ipaddr'] = src['request']['ip'] # not host field
                doc['uri']    = src['request']['uri']['path']
                doc['action'] = _action
        #### end of loop hit_records
        if any(err_args):
            _logger.error(None, *err_args)


#### end of UserActionSet


