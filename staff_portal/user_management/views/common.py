import logging

from django.core.exceptions     import ObjectDoesNotExist, MultipleObjectsReturned, PermissionDenied
from rest_framework.settings    import api_settings as drf_settings
from rest_framework.response    import Response as RestResponse
from rest_framework             import status as RestStatus

from ..apps   import UserManagementConfig as UserMgtCfg
from ..models.base import EmailAddress, GenericUserProfile
from ..models.auth import UnauthResetAccountRequest
from .constants import LOGIN_WEB_URL

_logger = logging.getLogger(__name__)


def check_auth_req_token(fn_succeed, fn_failure):
    def inner(self, request, *args, **kwargs):
        activate_token = kwargs.get('token', None)
        auth_req = UnauthResetAccountRequest.is_token_valid(activate_token)
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
# create new request in UnauthResetAccountRequest for either changing username or password
# Send mail with account-reset URL page to the user

def get_profile_by_email(addr:str, request):
    try:
        email = EmailAddress.objects.get(addr=addr)
        prof_cls = email.user_type.model_class()
        if prof_cls is not GenericUserProfile:
            raise MultipleObjectsReturned("invalid class type for individual user profile")
        profile = prof_cls.objects.get(pk=email.user_id)
        if not profile.account.is_active:
            raise PermissionDenied("not allowed to query account of a deactivated user")
    except (ObjectDoesNotExist, MultipleObjectsReturned, PermissionDenied) as e:
        fully_qualified_cls_name = '%s.%s' % (type(e).__module__, type(e).__qualname__)
        err_args = ["email", addr, "excpt_type", fully_qualified_cls_name, "excpt_msg", e.args[0]]
        _logger.warning(None, *err_args, request=request)
        email = None
        profile = None
    return email, profile


