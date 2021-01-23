from datetime import timedelta

from django.core.exceptions import ImproperlyConfigured
from django.contrib.sessions.backends.file import SessionStore as FileSessionStore


class SessionStore(FileSessionStore):
    """
    override the same function of its parent class, to return correct expiry time
    of a seesion after set_expiry() was called
    """
    # if set_expiry() and _expiry_date() are used together, it will cause get_expiry_age() return
    # constant integer rather than reflect real time period available for accessing that session,
    # don't use Django's default implementation

    def set_expiry(self, value):
        """ force int to be convert to timedelta format """
        if value and isinstance(value, int):
            value = timedelta(seconds=value)
        super().set_expiry(value=value)

    # def _expiry_date(self, session_data):
    #     custom_expiry_secs = session_data.get('_session_expiry')
    #     if custom_expiry_secs and not isinstance(custom_expiry_secs, int):
    #         errmsg = "_session_expiry in file session has to be integer, not object type."
    #         raise ImproperlyConfigured(errmsg)
    #     exp_secs = custom_expiry_secs or self.get_session_cookie_age()
    #     out =  self._last_modification() + timedelta(seconds=exp_secs)
    #     # print("refined file session, _expiry_date , exp_secs: "+ str(exp_secs))
    #     # print("refined file session, _expiry_date , out: "+ str(out))
    #     # print("refined file session, _expiry_date , _last_modification(): "+ str(self._last_modification()))
    #     return out


