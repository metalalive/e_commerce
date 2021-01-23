from importlib import import_module
import logging

from django.http        import  HttpResponse
from django.conf        import  settings as django_settings
from django.core.cache  import  caches as DjangoBuiltinCaches
from django.db.utils    import  OperationalError
from django.utils.deprecation import MiddlewareMixin

from common.models.db import db_conn_retry_wrapper, get_db_error_response

_logger = logging.getLogger(__name__)

class OneSessionPerAccountMiddleware:
    """
    * check weather concurrent login happens to an individual user account,
      and avoid that by removing duplicate (previously-created) sessions.
    * to apply this middleware, it has to sit behind the another built-in middleware called
      `django.contrib.auth.middleware.AuthenticationMiddleware` in django.conf.settings.MIDDLEWARE,
      because this middleware accesses request.user at the beginning of call function, the attribute
      `user` in `request` object is added ONLY after `AuthenticationMiddleware` completes its
      execution.
    """
    def __init__(self, get_response):
        self.get_response = get_response


    def __call__(self, request):
        account = request.user
        try:
            if account.is_authenticated:
                cache_user_sess = DjangoBuiltinCaches['user_session']
                key = "restrict_account_id_{pk}".format(pk=account.pk)
                value = cache_user_sess.get(key)
                if value is not None and value != request.session.session_key:
                    engine = import_module(django_settings.SESSION_ENGINE)
                    old_session = engine.SessionStore(session_key=value)
                    # the condition below is necessary because expired sessions may be cleaned up
                    # periodically by other async task (e.g. in my case, celery-beat) before current login event.
                    if old_session:
                        msg = "concurrent login detected, deleting old session"
                        log_args = ['msg', msg, 'cache_sess_key', key, 'cache_sess_value', value,
                                'account_id', account.pk]
                        _logger.warning(None, *log_args)
                        old_session.delete()
                cache_user_sess.set(key, request.session.session_key)

            response = self.get_response(request)

        except OperationalError as e:
            status = get_db_error_response(e=e, headers={})
            # do NOT use DRF response since the request is being short-circuited by directly returning
            # custom response at here and it won't invoke subsequent (including view) middlewares.
            # Instead I use HttpResponse simply containing error response status without extra message
            # in the response body.
            response = HttpResponse(status=status)
            err_msg = ' '.join(list(map(lambda x: str(x), e.args)))
            log_msg = ['status', status, 'msg', err_msg]
            _logger.warning(None, *log_msg)

        return response


