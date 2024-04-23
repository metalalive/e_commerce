from django.conf import settings as django_settings
from ..jwt import JWT


def _determine_expiry(user):
    """
    determine session expiry time in seconds dynamically for different status of users
    e.g. superuser, staff, customers
    """
    # TODO: find better way to doing this
    if user and user.is_authenticated and getattr(user, "pk"):
        if user.is_superuser:
            expiry_secs = django_settings.SESSION_COOKIE_AGE
        elif user.is_staff:
            expiry_secs = django_settings.SESSION_COOKIE_AGE << 2
        else:
            expiry_secs = django_settings.SESSION_COOKIE_AGE << 4
    else:
        expiry_secs = -1
    return expiry_secs


def jwt_httpreq_verify(request):
    result = None
    audience = getattr(request, "cors_host_label", "api")
    encoded_acs_tok = request.COOKIES.get(django_settings.JWT_NAME_ACCESS_TOKEN, None)
    encoded_rfr_tok = request.COOKIES.get(django_settings.JWT_NAME_REFRESH_TOKEN, None)
    _jwt = JWT()
    if encoded_acs_tok is not None:  # verify access token first
        result = _jwt.verify(
            unverified=encoded_acs_tok, audience=[audience], keystore=request._keystore
        )
    elif encoded_rfr_tok is not None:  # verify refresh token if access token is invalid
        result = _jwt.verify(
            unverified=encoded_rfr_tok, audience=[audience], keystore=request._keystore
        )
        if result:
            # issue new access token, also renew refresh token without changing
            # its payload, both tokens will be signed with different secret key
            # later on processing response
            acs = JWT()
            acs.payload["acc_id"] = _jwt.payload["acc_id"]
            _jwt.header["kid"] = ""  # force to rotate refresh token
    return result


#### def gen_jwt_token_set(acs, rfr, user=None, **kwargs):
####     """
####     generate a pair of JWT-based tokens , one for accessing resource,
####     the other one for refreshing the access token
####     """
####     @property
####     def valid(self):
####         """ could be True, False, or None (not verified yet) """
####         acs_valid = self.acs and getattr(self.acs, 'valid', False)  is True
####         rfr_valid = self.rfr and getattr(self.rfr, 'valid', False)  is True
####         return acs_valid or rfr_valid
####
####     def get_entries(self):
####         max_age_rfr = _determine_expiry(user=self.user) # get expiry time based on user status
####         max_age_acs = django_settings.JWT_ACCESS_TOKEN_VALID_PERIOD
####         if max_age_rfr <= max_age_acs :
####             log_args = ['max_age_rfr', max_age_rfr,'max_age_acs', max_age_acs,'user', self.user]
####             _logger.error(None, *log_args) # internal error that should be fixed at development stage
####         num_refreshes = math.ceil(max_age_rfr / max_age_acs)
####         max_age_rfr = num_refreshes * max_age_acs
####         out = (
####                 {'jwtobj': self.acs, 'max_age': max_age_acs, 'cookie_domain':None,
####                 'cookie_name': django_settings.JWT_NAME_ACCESS_TOKEN},
####                 {'jwtobj': self.rfr, 'max_age': max_age_rfr, 'cookie_domain':None,
####                 'cookie_name': django_settings.JWT_NAME_REFRESH_TOKEN},
####         )
####         return out
####
####     attrs = {}
####     essential = {'acs': acs, 'rfr': rfr, 'user': user, 'valid': valid, 'destroy': False,
####             'entries': property(get_entries, None), 'valid_token_names':('acs','rfr',) }
####     attrs.update(kwargs)
####     attrs.update(essential)
####     cls = type('_RequestTokenSet', (), attrs)
####     return cls()
#### ## end of gen_jwt_token_set
