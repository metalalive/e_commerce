from importlib import import_module
from datetime  import date, time, timedelta
import unicodedata
import functools
import logging

from django.conf   import  settings as django_settings
from django.core   import  mail
from django.template   import Template, Context
from django.utils.html import strip_tags

from .celery import app as celery_app
from .elasticsearch import es_client, get_dsl_template

_logger = logging.getLogger(__name__)


def log_wrapper(loglevel=logging.DEBUG):
    """
    internal log wrapper for async tasks, logging whenever any task reports error
    """
    def _wrapper(func):
        @functools.wraps(func) # copy metadata from custom func, which will be used for task caller
        def _inner(*arg, **kwargs):
            out = None
            log_args = ['action', func.__name__]
            try:
                out = func(*arg, **kwargs)
                log_args.extend(['status', 'completed', 'output', out])
                _logger.log(loglevel, None, *log_args)
            except Exception as e:
                excpt_cls = "%s.%s" % (type(e).__module__ , type(e).__qualname__)
                excpt_msg = ' '.join(list(map(lambda x: str(x), e.args)))
                log_args.extend(['status', 'failed', 'excpt_cls', excpt_cls, 'excpt_msg', excpt_msg])
                _logger.error(None, *log_args)
                raise
            return out
        return _inner
    return _wrapper


def remove_control_characters(str_in):
    return "".join(c for c in str_in if unicodedata.category(c)[0] != 'C')


@celery_app.task(bind=True)
@log_wrapper()
def sendmail(self, to_addrs, from_addr, msg_template_path, msg_data, subject_template,
                subject_data=None, attachment_paths=None,):
    assert isinstance(to_addrs,list) , "to_addrs %s must be list type" % to_addrs
    tsk_instance = self
    msg_html = None
    subject  = None
    with open(subject_template,'r') as f: # load template files, render with given data
        subject = f.readline()
    with open(msg_template_path,'r') as f:
        msg_html = f.read()
    assert subject , "Error occured on reading file %s" % subject_template
    assert msg_html, "Error occured on reading file %s" % msg_template_path
    subject_data = subject_data or {}

    subject = remove_control_characters(subject)
    subject = subject.format(**subject_data)
    template = Template(msg_html)
    context  = Context(msg_data)
    msg_html = template.render(context)
    msg_plaintext = strip_tags(msg_html)

    mailobj = mail.message.EmailMultiAlternatives(subject=subject, body=msg_plaintext,
            from_email=from_addr, to=to_addrs,)
    mailobj.attach_alternative(msg_html, 'text/html')
    if attachment_paths:
        for path, mimetype in attachment_paths:
            mailobj.attach_file(path=path, mimetype=mimetype)

    log_args = ['to_addrs', to_addrs, 'from_addr', from_addr, 'subject_template', subject_template,
            'msg_template_path', msg_template_path, 'renderred_subject', subject,
            'task_req_id', tsk_instance.request.id, 'task_req_expire', tsk_instance.request.expires,
            ]
    _logger.debug(None, *log_args)
    result = mailobj.send()
    # TODO, implement error handler for network error, e.g. backup and send at later time ?
    return result


@celery_app.task
def default_error_handler(task_id):
    result = AsyncResult(task_id)
    exc = result.get(propagate=False)
    errmsg = "task {0} exception : {1!r}\n{2!r}"
    args = [task_id, exc, result.traceback]
    print(errmsg.format(*args))


@celery_app.task
@log_wrapper(loglevel=logging.INFO)
def clean_expired_web_session():
    engine = import_module(django_settings.SESSION_ENGINE)
    engine.SessionStore.clear_expired()


@celery_app.task
@log_wrapper(loglevel=logging.INFO)
def clean_old_log_data(days=1, weeks=52, scroll_size=1000, requests_per_second=-1): # 365 days by default
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
# end of clean_old_log_data


