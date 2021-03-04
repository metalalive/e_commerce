from django.conf import settings as django_settings
from rest_framework.settings import APISettings

USER_SETTINGS = getattr(django_settings, 'REST_PROXY', None)

DEFAULTS = {
    'HOST': None,

    'AUTH': {'username':None, 'password':None, 'token':None},

    'TIMEOUT': None,

    'HEADER': {
        'accept': 'application/json, application/x-ndjson',
        'accept-language': 'en-US,en;q=0.8',
        'content-type': 'text/plain',
        'content-length': '0',
    },

    'VERIFY_SSL': True,
} # end of DEFAULTS

api_proxy_settings = APISettings(user_settings=USER_SETTINGS, defaults=DEFAULTS)

