import os as builtin_os
from importlib import import_module
from datetime  import datetime, date, time, timedelta
import logging

from django.conf   import  settings as django_settings

from .elasticsearch import es_client, get_dsl_template
from .celery import app as celery_app
from common.logging.util import log_fn_wrapper


_logger = logging.getLogger(__name__)

@celery_app.task
@log_fn_wrapper(logger=_logger, loglevel=logging.INFO)
def clean_old_log_localhost(max_days_keep=100):
    num_removed = 0
    curr_pos = django_settings.BASE_DIR
    while curr_pos.name != 'staff_portal':
        curr_pos = curr_pos.parent
    log_path = curr_pos.joinpath('./tmp/log/staffsite')
    if log_path.exists():
        for curr_node in log_path.iterdir():
            stat = curr_node.stat()
            t0 = datetime.utcnow() - timedelta(days=max_days_keep)
            t1 = datetime.utcfromtimestamp(stat.st_mtime)
            if t0 > t1:
                if curr_node.is_file:
                    builtin_os.remove(curr_node)
                    num_removed += 1
    return num_removed

@celery_app.task
@log_fn_wrapper(logger=_logger, loglevel=logging.INFO)
def clean_old_log_elasticsearch(days=1, weeks=52, scroll_size=1000, requests_per_second=-1): # 365 days by default
    """
    clean up all log data created before current time minus time_delta
    """
    # scroll_size shouldn't be over 10k, the cleanup will be very slow when scroll_size is over 2k
    def _set_ts_userlog(dslroot, value):
        dslroot['query']['bool']['must'][0]['range']['@timestamp']['lte'] = value

    def _set_ts_xpackmonitor(dslroot, value):
        dslroot['query']['range']['timestamp']['lte'] = value

    _fixture = {
        'log-*' : {
            'dsl_template_path': 'common/data/dsl_clean_useraction_log.json',
            'set_ts': _set_ts_userlog,
        },
        '.monitoring-*' : {
            'dsl_template_path': 'common/data/dsl_clean_xpack_monitoring_log.json',
            'set_ts': _set_ts_xpackmonitor,
        },
    }
    responses = []
    td = timedelta(days=days, weeks=weeks)
    d0 = date.today()
    d1 = d0 - td
    t0 = time(microsecond=1)
    time_args = [d1.isoformat(), 'T', t0.isoformat(), 'Z']
    delete_before = ''.join(time_args)
    request_timeout = 35

    for idx, v in _fixture.items():
        read_dsl = get_dsl_template(path=v['dsl_template_path'])
        v['set_ts'](dslroot=read_dsl, value=delete_before)
        total_deleted = 0
        response = {}
        # explicitly divide all data to smaller size (size == scroll_size) in each bulk request
        # so ES can delete them quickly, it is wierd ES poorly handles scroll requests when size is
        # much greater than scroll_size and requests_per_second is a positive integer.
        while True:
        ### for jdx in range(10):
            response = es_client.delete_by_query(index=idx, body=read_dsl, size=scroll_size, scroll_size=scroll_size,
                    requests_per_second=requests_per_second, conflicts='proceed', request_timeout=request_timeout, timeout='31s')
            if any(response['failures']):
                log_args = ['td', td, 'delete_before', delete_before, 'response', response,
                        'total_deleted_docs', total_deleted]
                raise Exception(log_args)
            if response['deleted'] > 0:
                total_deleted += response['deleted']
            else:
                break
        response['total_deleted'] = total_deleted
        responses.append(response)
    return responses
# end of clean_old_log_elasticsearch


