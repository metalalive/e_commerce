import unicodedata
import logging

from django.core   import  mail
from django.template   import Template, Context
from django.utils.html import strip_tags

from .celery import app as celery_app
from common.logging.util import log_fn_wrapper

_logger = logging.getLogger(__name__)


def remove_control_characters(str_in):
    return "".join(c for c in str_in if unicodedata.category(c)[0] != 'C')


@celery_app.task(bind=True, queue='mailing', routing_key='mail.defualt')
@log_fn_wrapper(logger=_logger, loglevel=logging.INFO)
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


