from importlib import import_module
import logging

from django.conf import settings as django_settings
from django.core.cache import caches as DjangoBuiltinCaches
from django.contrib.sessions.middleware import SessionMiddleware

_logger = logging.getLogger(__name__)

# The middlewares in this module works only for web pages hosted in Django
# server, since this project focuses on backend API servers, no other backend
# app in this project uses the middlewares here.


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
        if account.is_authenticated:
            cache_user_sess = DjangoBuiltinCaches["user_session"]
            key = "restrict_account_id_{pk}".format(pk=account.pk)
            value = cache_user_sess.get(key)
            if value is not None and value != request.session.session_key:
                engine = import_module(django_settings.SESSION_ENGINE)
                old_session = engine.SessionStore(session_key=value)
                # the condition below is necessary because expired sessions may be cleaned up
                # periodically by other async task (e.g. in my case, celery-beat) before current login event.
                if old_session:
                    msg = "concurrent login detected, deleting old session"
                    log_args = [
                        "msg",
                        msg,
                        "cache_sess_key",
                        key,
                        "cache_sess_value",
                        value,
                        "account_id",
                        account.pk,
                    ]
                    _logger.warning(None, *log_args)
                    old_session.delete()
            cache_user_sess.set(key, request.session.session_key)

        response = self.get_response(request)
        return response


## end of class OneSessionPerAccountMiddleware


class ExtendedSessionMiddleware(SessionMiddleware):
    enable_process_request = False
    enable_process_response = False

    def process_request(self, request):
        if self.enable_process_request is not True:
            return
        super().process_request(request)

    def process_response(self, request, response):
        if self.enable_process_response is not True:
            return response
        return super().process_response(request, response)


class SessionSetupMiddleware(ExtendedSessionMiddleware):
    enable_process_response = True


class SessionVerifyMiddleware(ExtendedSessionMiddleware):
    enable_process_request = True
