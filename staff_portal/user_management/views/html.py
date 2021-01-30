from datetime import datetime, timezone
import logging

from django.conf   import  settings as django_settings
from django.middleware.csrf     import rotate_token
from django.views.generic.base  import View, ContextMixin, TemplateResponseMixin
from django.contrib.auth.mixins import LoginRequiredMixin
from django.contrib.contenttypes.models  import ContentType
from rest_framework.settings    import api_settings as drf_settings
from rest_framework             import status as RestStatus
from rest_framework.response    import Response as RestResponse
from rest_framework.renderers   import TemplateHTMLRenderer, JSONRenderer
from rest_framework.views       import APIView
from rest_framework.generics    import GenericAPIView


from common.views  import  BaseAuthHTMLView, BaseLoginView
from common.views.mixins     import  LimitQuerySetMixin, UserEditViewLogMixin, BulkUpdateModelMixin
from common.auth.middleware  import  csrf_protect_m
from common.util.python.async_tasks  import  sendmail as async_send_mail, default_error_handler as async_default_error_handler

from ..serializers import AuthRoleSerializer, QuotaUsageTypeSerializer, GenericUserGroupSerializer, GenericUserProfileSerializer
from ..serializers import LoginAccountSerializer, AuthUserResetRequestSerializer
from ..apps        import UserManagementConfig as UserMgtCfg
from .constants    import LOGIN_URL, _PRESERVED_ROLE_IDS, MAX_NUM_FORM
from .common       import check_auth_req_token, LoginAccountCommonEntryMixin, get_profile_account_by_email, GetProfileIDMixin

_logger = logging.getLogger(__name__)

class AuthHTMLView(BaseAuthHTMLView):
    login_url = LOGIN_URL


class AuthRoleHTMLparamMixin:
    formparams = {
            'max_num_form': MAX_NUM_FORM,  'app_name': UserMgtCfg.app_url,
            'non_field_errors':drf_settings.NON_FIELD_ERRORS_KEY,
            'submit_uri': UserMgtCfg.api_url['AuthRoleAPIView'][0],
            'success_redirect_uri': UserMgtCfg.api_url['DashBoardView'],
            'permission_api_uri': UserMgtCfg.api_url['AuthPermissionView'],
            }

class AuthRoleAddHTMLView(AuthHTMLView, AuthRoleHTMLparamMixin):
    def get(self, request, *args, **kwargs):
        template_name = UserMgtCfg.template_name[self.__class__.__name__]
        context = {'formparams': self.formparams}
        return RestResponse(data=context, template_name=template_name)


class AuthRoleUpdateHTMLView(LimitQuerySetMixin, AuthHTMLView, AuthRoleHTMLparamMixin):
    serializer_class = AuthRoleSerializer
    PRESERVED_ROLE_IDS = _PRESERVED_ROLE_IDS
    queryset = serializer_class.Meta.model.objects.all()

    def get(self, request, *args, **kwargs):
        queryset = self.get_queryset(pk_param_name='ids', pk_field_name='id',
                 pk_skip_list=self.PRESERVED_ROLE_IDS, )
        serializer = self.get_serializer(queryset, many=True)
        self.formparams.update({'data': serializer.data,})
        template_name = UserMgtCfg.template_name[self.__class__.__name__]
        context = {'formparams': self.formparams,}
        return RestResponse(data=context, template_name=template_name)


class QuotaUsageTypeHTMLparamMixin:
    @property
    def formparams(self):
        if not hasattr(self, '_formparams'):
            self._formparams = {
                'max_num_form': MAX_NUM_FORM,  'app_name': UserMgtCfg.app_url,
                'non_field_errors':drf_settings.NON_FIELD_ERRORS_KEY,
                'submit_uri': UserMgtCfg.api_url['QuotaUsageTypeAPIView'],
                'success_redirect_uri': UserMgtCfg.api_url['DashBoardView'],
                'material_type': self.load_quota_material_type(),
                }
        return self._formparams

    def load_quota_material_type(self):
        exclude_apps = ['admin','auth','contenttypes','sessions','softdelete']
        app_models = ContentType.objects.values('id','app_label','model').exclude(app_label__in=exclude_apps)
        out = {}
        for item in app_models: # TODO, find better way of creating 2D array
            app_label = item.pop('app_label')
            if out.get(app_label, None) is None:
                out[app_label] = []
            out[app_label].append(item)
        return out



class QuotaUsageTypeAddHTMLView(AuthHTMLView, QuotaUsageTypeHTMLparamMixin):
    def get(self, request, *args, **kwargs):
        template_name = UserMgtCfg.template_name[self.__class__.__name__]
        return RestResponse(data={'formparams': self.formparams}, template_name=template_name)


class QuotaUsageTypeUpdateHTMLView(LimitQuerySetMixin, AuthHTMLView, QuotaUsageTypeHTMLparamMixin):
    serializer_class = QuotaUsageTypeSerializer
    queryset = serializer_class.Meta.model.objects.all()

    def get(self, request, *args, **kwargs):
        serializer = self.get_serializer(self.get_queryset(pk_param_name='ids', pk_field_name='id'), many=True)
        self.formparams.update({'data': serializer.data,})
        template_name = UserMgtCfg.template_name[self.__class__.__name__]
        return RestResponse(data={'formparams': self.formparams}, template_name=template_name)


class UserGroupsHTMLparamMixin:
    formparams = {
            'max_num_form': MAX_NUM_FORM,  'app_name': UserMgtCfg.app_url,
            'non_field_errors':drf_settings.NON_FIELD_ERRORS_KEY,
            'submit_uri': UserMgtCfg.api_url['UserGroupsAPIView'][0],
            'success_redirect_uri': UserMgtCfg.api_url['DashBoardView'],
            'authrole_api_uri':  UserMgtCfg.api_url['AuthRoleAPIView'][0],
            'quotatype_api_uri': UserMgtCfg.api_url['QuotaUsageTypeAPIView'],
            }

class UserGroupsAddHTMLView(AuthHTMLView, UserGroupsHTMLparamMixin):
    def get(self, request, *args, **kwargs):
        template_name = UserMgtCfg.template_name[self.__class__.__name__]
        return RestResponse(data={'formparams': self.formparams}, template_name=template_name)


class UserGroupsUpdateHTMLView(LimitQuerySetMixin, AuthHTMLView, UserGroupsHTMLparamMixin):
    serializer_class = GenericUserGroupSerializer

    def get(self, request, *args, **kwargs):
        exc_rd_fields=['roles__name', 'quota__usage_type__label', 'ancestors__ancestor__name',
                'ancestors__id']
        queryset = self.get_queryset(pk_param_name='ids', pk_field_name='id')
        serializer = self.get_serializer(queryset, many=True, exc_rd_fields=exc_rd_fields,
                        account=request.user,  from_edit_view=True,)
        self.formparams.update({'data': serializer.data})
        template_name = UserMgtCfg.template_name[self.__class__.__name__]
        return RestResponse(data={'formparams': self.formparams}, template_name=template_name)


class UserProfileHTMLparamMixin:
    formparams = {
            'max_num_form': MAX_NUM_FORM,  'app_name': UserMgtCfg.app_url,
            'non_field_errors':drf_settings.NON_FIELD_ERRORS_KEY,
            'submit_uri': UserMgtCfg.api_url['UserProfileAPIView'][0],
            'success_redirect_uri': UserMgtCfg.api_url['DashBoardView'],
            'authrole_api_uri':  UserMgtCfg.api_url['AuthRoleAPIView'][0],
            'quotatype_api_uri': UserMgtCfg.api_url['QuotaUsageTypeAPIView'],
            'usrgrp_api_uri': UserMgtCfg.api_url['UserGroupsAPIView'][0],
            }

class UserProfileAddHTMLView(AuthHTMLView, UserProfileHTMLparamMixin):
    def get(self, request, *args, **kwargs):
        template_name = UserMgtCfg.template_name[self.__class__.__name__]
        return RestResponse(data={'formparams': self.formparams}, template_name=template_name)


class UserProfileUpdateHTMLView(LimitQuerySetMixin, AuthHTMLView, UserProfileHTMLparamMixin):
    serializer_class = GenericUserProfileSerializer

    def get(self, request, *args, **kwargs):
        queryset = self.get_queryset(pk_param_name='ids', pk_field_name='id')
        serializer = self.get_serializer(queryset, many=True, account=request.user, from_edit_view=True,
                exc_rd_fields=['roles__name', '',],)
        self.formparams.update({'data': serializer.data })
        template_name = UserMgtCfg.template_name[self.__class__.__name__]
        return RestResponse(data={'formparams': self.formparams,}, template_name=template_name)





class DashBoardView(LoginRequiredMixin, View, ContextMixin, TemplateResponseMixin):
    template_name = UserMgtCfg.template_name[__qualname__]
    redirect_field_name = 'redirect'
    login_url = LOGIN_URL

    def get(self, request, *args, **kwargs):
        formparams = {'uri_dict': self._get_required_uris(),}
        context = ContextMixin.get_context_data(self, formparams=formparams)
        return TemplateResponseMixin.render_to_response(self, context)

    def _get_required_uris(self):
        appname = UserMgtCfg.app_url
        out = {}
        for k,v in UserMgtCfg.api_url:
            if isinstance(v, list):
                out[k] = ['/{}/{}'.format(appname,u) for u in v]
            elif isinstance(v, str):
                if v.endswith('<slug:pk>'):
                    v = v[:v.find('<slug:pk>')]
                out[k] = '/{}/{}'.format(appname,v)
            else:
                err_args = ["unknown_type_key", k, "value", v]
                _logger.error(None, *err_args, request=self.request)
                #raise TypeError
        return out


class LoginAccountCreateView(APIView, LoginAccountCommonEntryMixin, UserEditViewLogMixin):
    renderer_classes = [TemplateHTMLRenderer, JSONRenderer]

    def _post_token_valid(self, request, *args, **kwargs):
        auth_req = kwargs.pop('auth_req')
        account = auth_req.profile.account
        if account is None:
            serializer_kwargs = {
                'mail_kwargs': {
                    'msg_template_path': 'user_management/data/mail/body/user_activated.html',
                    'subject_template' : 'user_management/data/mail/subject/user_activated.txt',
                },
                'data': request.data, 'passwd_required':True, 'confirm_passwd': True, 'uname_required': True,
                'account': None, 'auth_req': auth_req,  'many': False,
            }
            _serializer = LoginAccountSerializer(**serializer_kwargs)
            _serializer.is_valid(raise_exception=True)
            profile_id = auth_req.profile.pk
            account = _serializer.save()
            self._log_action(action_type='create', request=request, affected_items=[account.minimum_info],
                    model_cls=type(account),  profile_id=profile_id)
        response_data = None
        status = None
        return response_data, status, None

    post = check_auth_req_token(fn_succeed=_post_token_valid, fn_failure=LoginAccountCommonEntryMixin._post_token_expired)



class UsernameRecoveryRequestView(APIView, UserEditViewLogMixin):
    """ for unauthenticated users who registered but forget their username """
    renderer_classes = [TemplateHTMLRenderer, JSONRenderer]

    def get(self, request, *args, **kwargs):
        if "CSRF_COOKIE" not in request.META:
            rotate_token(request=request)
            request.csrf_cookie_age = 51
        context = {'formparams': {},}
        template_name = UserMgtCfg.template_name[self.__class__.__name__]
        return RestResponse(data=context, template_name=template_name)

    @csrf_protect_m
    def post(self, request, *args, **kwargs):
        addr = request.data.get('addr', '').strip()
        useremail,  profile, account = get_profile_account_by_email(addr=addr, request=request)
        self._send_recovery_mail(profile=profile, account=account, addr=addr)
        # don't respond with success / failure status purposely, to avoid malicious email enumeration
        return RestResponse(data=None, status=RestStatus.HTTP_202_ACCEPTED)

    def _send_recovery_mail(self, profile, account, addr):
        if profile is None or account is None:
            return
        # send email directly, no need to create auth user request
        # note in this application, username is not PII (Personable Identifible Information)
        # so username can be sent directly to user mailbox. TODO: how to handle it if it's PII ?
        msg_data = {'first_name': profile.first_name, 'last_name': profile.last_name,
                'username': account.username, 'request_time': datetime.now(timezone.utc), }
        msg_template_path = 'user_management/data/mail/body/username_recovery.html'
        subject_template  = 'user_management/data/mail/subject/username_recovery.txt'
        to_addr = addr
        from_addr = django_settings.DEFAULT_FROM_EMAIL
        task_kwargs = {
            'to_addrs': [to_addr], 'from_addr':from_addr, 'msg_data':msg_data,
            'subject_template': subject_template, 'msg_template_path':msg_template_path,
        }
        # Do not return result backend or task ID to unauthorized frontend user.
        # Log errors raising in async task
        async_send_mail.apply_async( kwargs=task_kwargs, link_error=async_default_error_handler.s() )
        self._log_action(action_type='recover_username', request=self.request, affected_items=[account.minimum_info],
                model_cls=type(account),  profile_id=profile.pk)



class UnauthPasswordResetRequestView(LimitQuerySetMixin, GenericAPIView, BulkUpdateModelMixin, GetProfileIDMixin):
    """ for unauthenticated users who registered but  forget their password """
    serializer_class = AuthUserResetRequestSerializer
    renderer_classes = [TemplateHTMLRenderer, JSONRenderer]

    def get(self, request, *args, **kwargs):
        if "CSRF_COOKIE" not in request.META:
            rotate_token(request=request)
        context = {'formparams': {},}
        template_name = UserMgtCfg.template_name[self.__class__.__name__]
        return RestResponse(data=context, template_name=template_name)

    @csrf_protect_m
    def post(self, request, *args, **kwargs):
        addr = request.data.get('addr', '').strip()
        useremail, profile, account = get_profile_account_by_email(addr=addr, request=request)

        if useremail and profile and account:
            resource_path = [UserMgtCfg.app_url] + UserMgtCfg.api_url[UnauthPasswordResetView.__name__].split('/')
            resource_path.pop() # last one should be <slug:token>
            serializer_kwargs = {
                'msg_template_path': 'user_management/data/mail/body/passwd_reset_request.html',
                'subject_template' : 'user_management/data/mail/subject/passwd_reset_request.txt',
                'url_host': "{protocol}://{hostname}".format(
                    protocol=request._request.scheme,
                    hostname=request._request._get_raw_host()
                ),
                'url_resource':'/'.join(resource_path),
            }
            err_args = ["url_host", serializer_kwargs['url_host']]
            _logger.debug(None, *err_args, request=request)
            extra_kwargs = {
                'many':False, 'return_data_after_done':True, 'pk_field_name':'profile', 'allow_create':True,
                'status_ok':RestStatus.HTTP_202_ACCEPTED, 'pk_src':LimitQuerySetMixin.REQ_SRC_BODY_DATA,
                'serializer_kwargs' :serializer_kwargs,
            }
            kwargs.update(extra_kwargs)
            # do not use frontend request data in this view, TODO, find better way of modifying request data
            request._full_data = {'profile': profile.pk, 'email': useremail.pk,}
            # `many = False` indicates that the view gets single model instance by calling get_object(...)
            # , which requires unique primary key value on URI, so I fake it by adding user profile ID field
            # and value to kwargs, because frontend (unauthorized) user shouldn't be aware of any user profile ID
            self.kwargs['profile'] = profile.pk
            user_bak = request.user
            try: # temporarily set request.user to the anonymous user who provide this valid email address
                request.user = account
                self.update(request, *args, **kwargs)
            except Exception as e:
                fully_qualified_cls_name = '%s.%s' % (type(e).__module__, type(e).__qualname__)
                log_msg = ['excpt_type', fully_qualified_cls_name, 'excpt_msg', e, 'email', addr,
                        'email_id', useremail.pk, 'profile', profile.minimum_info]
                _logger.error(None, *log_msg, exc_info=True, request=request)
            request.user = user_bak
        # always respond OK status even if it failed, to avoid malicious email enumeration
        return RestResponse(data=None, status=RestStatus.HTTP_202_ACCEPTED)
        #  HTTP_401_UNAUTHORIZED
    ## end of post()


class UnauthPasswordResetView(APIView, LoginAccountCommonEntryMixin, UserEditViewLogMixin):
    renderer_classes = [TemplateHTMLRenderer, JSONRenderer]

    def _patch_token_valid(self, request, *args, **kwargs):
        auth_req = kwargs.pop('auth_req')
        account = auth_req.profile.account
        if account:
            serializer_kwargs = {
                'mail_kwargs': {
                    'msg_template_path': 'user_management/data/mail/body/unauth_passwd_reset.html',
                    'subject_template' : 'user_management/data/mail/subject/unauth_passwd_reset.txt',
                },
                'data': request.data, 'passwd_required':True,  'confirm_passwd': True,
                'account': request.user, 'auth_req': auth_req,  'many': False,
            }
            _serializer = LoginAccountSerializer(**serializer_kwargs)
            _serializer.is_valid(raise_exception=True)
            profile_id = auth_req.profile.pk
            _serializer.save()
            user_bak = request.user
            request.user = account
            self._log_action(action_type='reset_password', request=request, affected_items=[account.minimum_info],
                    model_cls=type(account),  profile_id=profile_id)
            request.user = user_bak
        response_data = None
        status = None # always return 200 OK even if the request is invalid
        return response_data, status, None

    patch = check_auth_req_token(fn_succeed=_patch_token_valid, fn_failure=LoginAccountCommonEntryMixin._post_token_expired)



class LoginView(BaseLoginView, GetProfileIDMixin):
    template_name = UserMgtCfg.template_name[__qualname__]
    submit_url    = LOGIN_URL
    default_url_redirect =  "/{}/{}".format(UserMgtCfg.app_url, UserMgtCfg.api_url[DashBoardView.__name__])
    is_staff_only = True


