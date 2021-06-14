import logging

from django.core.exceptions     import ObjectDoesNotExist, MultipleObjectsReturned, PermissionDenied
from django.contrib.auth.models import User as AuthUser
from rest_framework.settings    import api_settings as drf_settings
from rest_framework.response    import Response as RestResponse
from rest_framework             import status as RestStatus

from common.views.proxy.mixins import BaseGetProfileIDMixin
from ..apps   import UserManagementConfig as UserMgtCfg
from ..models import AuthUserResetRequest, EmailAddress, GenericUserProfile
from .constants import LOGIN_WEB_URL

_logger = logging.getLogger(__name__)

class GetProfileIDMixin(BaseGetProfileIDMixin):
    def get_profile(self, account, **kwargs):
        if not hasattr(self, '_cache_profile'):
            self._cache_profile = account.genericuserauthrelation.profile
        return self._cache_profile

    def get_profile_id(self, request, **kwargs):
        # TODO, should be not-implemented error , let other apps subclass this mixin
        account = request.user
        if account and isinstance(account, AuthUser):
            profile = self.get_profile(account=account)
            profile_id = profile.pk
        else:
            # which means unauthenticated accesses happened to model instances,
            # application developers should analyze log data and determine whether this
            # part of the system has been compromised.
            profile_id = self.UNKNOWN_ID
        return str(profile_id)


def check_auth_req_token(fn_succeed, fn_failure):
    def inner(self, request, *args, **kwargs):
        activate_token = kwargs.get('token', None)
        auth_req = AuthUserResetRequest.is_token_valid(activate_token)
        if auth_req:
            kwargs['auth_req'] = auth_req
            resp_kwargs = fn_succeed(self, request, *args, **kwargs)
        else:
            resp_kwargs = fn_failure(self, request, *args, **kwargs)
        return RestResponse(**resp_kwargs)
    return inner


class AuthTokenCheckMixin:
    def token_valid(self, request, *args, **kwargs):
        return {'data': {}, 'status':None, 'template_name': None}

    def token_expired(self, request, *args, **kwargs):
        context = {drf_settings.NON_FIELD_ERRORS_KEY : ['invalid auth req token']}
        status = RestStatus.HTTP_401_UNAUTHORIZED
        return {'data':context, 'status':status}



## --------------------------------------------------------------------------------------
# process single user at a time
# create new request in AuthUserResetRequest for either changing username or password
# Send mail with account-reset URL page to the user

def get_profile_account_by_email(addr:str, request):
    # TODO, handle concurrent identical request sent at the same time from the same client,
    # perhaps using CSRF token, or hash request body, to indentify that the first request is
    # processing while the second one comes in for both of concurrent and identical requests.
    try:
        email = EmailAddress.objects.get(addr=addr)
        useremail = email.useremail
        prof_cls = useremail.user_type.model_class()
        if not prof_cls is  GenericUserProfile:
            raise MultipleObjectsReturned("invalid class type for individual user profile")
        profile = prof_cls.objects.get(pk=useremail.user_id)
        if not profile.active:
            raise PermissionDenied("not allowed to query account of a deactivated user")
        account = profile.auth.login # may raise ObjectDoesNotExist exception
    except (ObjectDoesNotExist, MultipleObjectsReturned, PermissionDenied) as e:
        fully_qualified_cls_name = '%s.%s' % (type(e).__module__, type(e).__qualname__)
        err_args = ["email", addr, "excpt_type", fully_qualified_cls_name, "excpt_msg", e,]
        _logger.warning(None, *err_args, request=request)
        useremail = None
        profile = None
        account = None
    return useremail, profile, account


