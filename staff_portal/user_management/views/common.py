import logging

from django.middleware     import csrf
from django.core.exceptions     import ObjectDoesNotExist, MultipleObjectsReturned, PermissionDenied
from rest_framework.settings    import api_settings as drf_settings
from rest_framework.response    import Response as RestResponse
from rest_framework             import status as RestStatus

from ..apps   import UserManagementConfig as UserMgtCfg
from ..models import AuthUserResetRequest, EmailAddress, GenericUserProfile
from .constants    import LOGIN_URL

_logger = logging.getLogger(__name__)

def check_auth_req_token(fn_succeed, fn_failure):
    def inner(self, request, *args, **kwargs):
        activate_token = self.kwargs.get('token', None)
        auth_req = AuthUserResetRequest.is_token_valid(activate_token)
        if auth_req:
            kwargs['auth_req'] = auth_req
            response_data, status, template_name = fn_succeed(self, request, *args, **kwargs)
        else:
            response_data, status, template_name = fn_failure(self, request, *args, **kwargs)
        return RestResponse(data=response_data, status=status, template_name=template_name)
    return inner


class LoginAccountCommonEntryMixin:
    def _get_token_valid(self, request, *args, **kwargs):
        activate_token = self.kwargs.get('token', None)
        template_name_list = UserMgtCfg.template_name[self.__class__.__name__]
        template_name = template_name_list[0]
        api_url = UserMgtCfg.api_url[self.__class__.__name__].split('/')
        api_url.pop()
        api_url = '/'.join(api_url)
        formparams = {'non_field_errors':drf_settings.NON_FIELD_ERRORS_KEY,
            'submit_url': "/{}/{}".format(UserMgtCfg.app_url, api_url),
            'activate_token': activate_token,
            'csrf_token': {'name': 'csrfmiddlewaretoken', 'value': csrf.get_token(request)},
            'success_url_redirect' : LOGIN_URL,
        }
        context = {'formparams': formparams, }
        status = None
        return context, status, template_name

    def _get_token_expired(self, request, *args, **kwargs):
        template_name_list = UserMgtCfg.template_name[self.__class__.__name__]
        template_name = template_name_list[1]
        context = {}
        status = RestStatus.HTTP_401_UNAUTHORIZED
        return context, status, template_name

    get = check_auth_req_token(fn_succeed=_get_token_valid, fn_failure=_get_token_expired)

    def _post_token_expired(self, request, *args, **kwargs):
        response_data = {drf_settings.NON_FIELD_ERRORS_KEY : ['invalid activate token']}
        status = RestStatus.HTTP_401_UNAUTHORIZED
        return response_data, status, None


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


