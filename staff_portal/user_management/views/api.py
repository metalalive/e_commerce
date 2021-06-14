import copy
import logging
from datetime import datetime, timezone, timedelta

from django.conf   import  settings as django_settings
from django.core.exceptions     import ValidationError
from django.http.response    import HttpResponseBase
from django.db.models        import Count
from django.utils.module_loading import import_string
from django.contrib.contenttypes.models  import ContentType

from rest_framework             import status as RestStatus
from rest_framework.generics    import GenericAPIView
from rest_framework.views       import APIView
from rest_framework.viewsets    import ModelViewSet
from rest_framework.filters     import OrderingFilter, SearchFilter
from rest_framework.renderers   import JSONRenderer
from rest_framework.response    import Response as RestResponse
from rest_framework.permissions import DjangoModelPermissions, DjangoObjectPermissions
from rest_framework.exceptions  import PermissionDenied
from rest_framework.settings    import api_settings as drf_settings

from softdelete.views import RecoveryModelMixin
from common.cors import config as cors_cfg
from common.auth.jwt import JWT
from common.auth.keystore import create_keystore_helper
from common.views.mixins   import  LimitQuerySetMixin, UserEditViewLogMixin, BulkUpdateModelMixin
from common.views.api      import  AuthCommonAPIView, AuthCommonAPIReadView
from common.views.filters  import  ClosureTableFilter
from common.util.python.async_tasks  import  sendmail as async_send_mail, default_error_handler as async_default_error_handler

from ..apps   import UserManagementConfig as UserMgtCfg
from ..models import GenericUserGroup, GenericUserGroupClosure, GenericUserProfile, UsermgtChangeSet
from ..async_tasks import update_roles_on_accounts

from ..serializers import AuthPermissionSerializer, AuthRoleSerializer, QuotaUsageTypeSerializer, GenericUserGroupSerializer
from ..serializers import GenericUserProfileSerializer, AuthUserResetRequestSerializer
from ..serializers import LoginAccountSerializer
from ..serializers import GenericUserAppliedRoleSerializer, GenericUserGroupRelationSerializer

from ..permissions import AuthRolePermissions, AppliedRolePermissions, AppliedGroupPermissions, UserGroupsPermissions
from ..permissions import UserDeactivationPermission, UserActivationPermission, UserProfilesPermissions

from .constants import  _PRESERVED_ROLE_IDS, MAX_NUM_FORM, WEB_HOST, API_GATEWAY_HOST
from .common    import GetProfileIDMixin, check_auth_req_token, AuthTokenCheckMixin, get_profile_account_by_email


# * All classes within this module can share one logger, because logger is unique by given name
#   as the argument on invoking getLogger(), that means subsequent call with the same logger name
#   will get the same logger instance.
# * Logger is thread safe, multiple view/serializer/model instances in project can access the same
#   logger instance simultaneously without data corruption.
# * It seems safe to load logger at module level, because django framework loads this module
#   after parsing logging configuration at settings.py
_logger = logging.getLogger(__name__)



class AuthPermissionView(AuthCommonAPIReadView, GetProfileIDMixin):
    """
    this API class should provide a set of pre-defined roles, which include a set of permissions
    granted to the role, in order for users in staff site to easily apply those permissions to application
    , instead of simply passing all django-defined permissions to mess user interface
    """
    queryset = AuthPermissionSerializer.Meta.model.objects.order_by('id')
    serializer_class = AuthPermissionSerializer


class AuthRoleAPIView(AuthCommonAPIView, GetProfileIDMixin):
    serializer_class = AuthRoleSerializer
    filter_backends = [AuthRolePermissions, SearchFilter, OrderingFilter,]
    ordering_fields  = ['id', 'name']
    search_fields  = ['name']
    # add django model permission obj
    permission_classes = copy.copy(AuthCommonAPIView.permission_classes) + [AuthRolePermissions]
    PRESERVED_ROLE_IDS = _PRESERVED_ROLE_IDS
    queryset = serializer_class.Meta.model.objects.all()

    def get(self, request, *args, **kwargs):
        if request.query_params.get('skip_preserved_role', None) is not None:
            kwargs['pk_skip_list'] = self.PRESERVED_ROLE_IDS
        return  super().get(request, *args, **kwargs)

    def post(self, request, *args, **kwargs):
        kwargs['many'] = True
        kwargs['return_data_after_done'] = False
        return  self.create(request, *args, **kwargs)

    def put(self, request, *args, **kwargs):
        kwargs['many'] = True
        kwargs['return_data_after_done'] = False
        kwargs['pk_src'] =  LimitQuerySetMixin.REQ_SRC_BODY_DATA
        kwargs['pk_skip_list'] = self.PRESERVED_ROLE_IDS
        # filter out any attempt to modify admin role
        req_data = filter(lambda x: not x.get('id',None) in self.PRESERVED_ROLE_IDS, request.data)
        request._full_data = [r for r in req_data]
        log_msg = ["filtered_request_data", request.data]
        _logger.debug(None, *log_msg, request=request)
        return  self.update(request, *args, **kwargs)

    def delete(self, request, *args, **kwargs):
        # conflict happenes if frontend attempts to delete preserved roles (e.g. admin role)
        IDs = self.get_IDs(pk_param_name='ids', pk_field_name='id',)
        diff = set(self.PRESERVED_ROLE_IDS) & set(IDs)
        if diff:
            errmsg = 'not allowed to delete preserved role ID = {}'.format(str(diff))
            context = {drf_settings.NON_FIELD_ERRORS_KEY: errmsg}
            response = RestResponse(data=context, status=RestStatus.HTTP_409_CONFLICT)
        else:
            kwargs['many'] = True
            kwargs['pk_skip_list'] = self.PRESERVED_ROLE_IDS
            response = self.destroy(request, *args, **kwargs)
        return response


class AppliedRoleReadAPIView(AuthCommonAPIReadView, GetProfileIDMixin):
    serializer_class = GenericUserAppliedRoleSerializer
    queryset =  serializer_class.Meta.model.objects.order_by('-last_updated')
    permission_classes = copy.copy(AuthCommonAPIView.permission_classes) + [AppliedRolePermissions]
    filter_backends = [AppliedRolePermissions,]

    def get(self, request, *args, **kwargs):
        # filter before calling get(), which makes get() invoke list()
        role_id = kwargs.pop('pk', 0)
        self.queryset = self.queryset.filter(role__pk=role_id)
        err_args = ["role_id", role_id, "num_grps_profs_apply_this_role", self.queryset.count(),]
        _logger.debug(None, *err_args, request=request)
        return super().get(request, *args, **kwargs)


class AppliedGroupReadAPIView(AuthCommonAPIReadView, GetProfileIDMixin):
    serializer_class = GenericUserGroupRelationSerializer
    queryset = serializer_class.Meta.model.objects.order_by('-id')
    permission_classes = copy.copy(AuthCommonAPIView.permission_classes) + [AppliedGroupPermissions]
    filter_backends = [AppliedGroupPermissions]

    def get(self, request, *args, **kwargs):
        # filter before calling get(), which makes get() invoke list()
        grp_id = kwargs.pop('pk', 0)
        self.queryset = self.queryset.filter(group__pk=grp_id)
        return super().get(request, *args, **kwargs)



class QuotaMaterialReadAPIView(AuthCommonAPIView):
    """
    In quota arrangement, material simply represents source of supply,
    e.g.
    number of resources like database table rows, memory space, hardware
    ... etc. which can be used by individual user or user group.

    * Since quota arrangement is about restricting users' access to resources,
      it makes sense to maintain the material types and the arrangement of
      each user in this user management service.
    * In this project , each material type represents one single model class,
      and each material is tied to Django ContentType model because Django is
      used to implement this service.
    * Fortunately, Django ContentType can also record path of model classes
      that are from other services implemented in different web frameworks and
      languages, this gives flexibility to gather information of all model
      classes from all other services for quota arrangment
    """
    # TODO:
    # * There would be API endpoints (e.g. REST, RPC) in this view class for interal use
    #   , so other downstream services are able to maintain materials which are linked to
    #   the model classes installed in other services (possible use case ?)
    def get(self, request, *args, **kwargs):
        from django.contrib.contenttypes.models import ContentType
        exclude_apps = ['admin','auth','contenttypes','sessions','softdelete']
        app_models = ContentType.objects.values('id','app_label','model').exclude(app_label__in=exclude_apps)
        out = {}
        for item in app_models: # TODO, find better way of creating 2D array
            app_label = item.pop('app_label')
            if out.get(app_label, None) is None:
                out[app_label] = []
            out[app_label].append(item)
        return RestResponse(data=out, status=None)


class QuotaUsageTypeAPIView(AuthCommonAPIView, GetProfileIDMixin):
    serializer_class = QuotaUsageTypeSerializer
    filter_backends = [SearchFilter, OrderingFilter,]
    ordering_fields  = ['id', 'label']
    search_fields  = ['label']
    permission_classes = copy.copy(AuthCommonAPIView.permission_classes) + [DjangoModelPermissions]
    queryset = serializer_class.Meta.model.objects.all()

    def post(self, request, *args, **kwargs):
        kwargs['many'] = True
        kwargs['return_data_after_done'] = False
        return  self.create(request, *args, **kwargs)

    def put(self, request, *args, **kwargs):
        kwargs['many'] = True
        kwargs['return_data_after_done'] = False
        kwargs['pk_src'] =  LimitQuerySetMixin.REQ_SRC_BODY_DATA
        return  self.update(request, *args, **kwargs)

    def delete(self, request, *args, **kwargs):
        kwargs['many'] = True
        return self.destroy(request, *args, **kwargs)




class UserGroupsAPIView(AuthCommonAPIView, RecoveryModelMixin, GetProfileIDMixin):
    serializer_class = GenericUserGroupSerializer
    filter_backends = [UserGroupsPermissions, SearchFilter, ClosureTableFilter, OrderingFilter,] #  
    closure_model_cls = GenericUserGroupClosure
    ordering_fields  = ['id', 'name', 'usr_cnt']
    # `ancestors__ancestor__name` already covers `name` field of each model instance
    search_fields  = ['ancestors__ancestor__name']
    permission_classes = copy.copy(AuthCommonAPIView.permission_classes) + [UserGroupsPermissions]
    queryset = serializer_class.Meta.model.objects.annotate(usr_cnt=Count('profiles'))
    SOFTDELETE_CHANGESET_MODEL = UsermgtChangeSet

    def get(self, request, *args, **kwargs):
        exc_rd_fields = request.query_params.get('exc_rd_fields', None)
        if exc_rd_fields is None:
            exc_rd_fields = ['ancestors__id']
        elif isinstance(exc_rd_fields, str):
            exc_rd_fields = [exc_rd_fields, 'ancestors__id']
        elif isinstance(exc_rd_fields, list):
            exc_rd_fields.extend(['ancestors__id'])
        kwargs['serializer_kwargs'] = {'from_read_view':True, 'exc_rd_fields': exc_rd_fields,}
        return super().get(request=request, *args, **kwargs)

    def post(self, request, *args, **kwargs):
        kwargs['many'] = True
        kwargs['return_data_after_done'] = False
        kwargs['exc_wr_fields'] = ['quota__user_type', 'quota__user_id']
        kwargs['serializer_kwargs'] = {'from_edit_view':True}
        return  self.create(request, *args, **kwargs)

    def put(self, request, *args, **kwargs):
        kwargs['many'] = True
        kwargs['return_data_after_done'] = False
        kwargs['pk_src'] =  LimitQuerySetMixin.REQ_SRC_BODY_DATA
        kwargs['serializer_kwargs'] = {'from_edit_view':True}
        return  self.update(request, *args, **kwargs)

    def delete(self, request, *args, **kwargs):
        kwargs['many'] = True
        kwargs['status_ok'] = RestStatus.HTTP_202_ACCEPTED
        # semantic: accepted, will be deleted after a point of time which no undelete operation is performed
        return self.destroy(request, *args, **kwargs)

    def patch(self, request, *args, **kwargs):
        kwargs['resource_content_type'] = ContentType.objects.get(app_label='user_management',
                model=self.serializer_class.Meta.model.__name__)
        return self.recovery(request=request, *args, **kwargs)

    def delete_success_callback(self, id_list):
        update_roles_on_accounts.delay(affected_groups=id_list, deleted=True)

    def recover_success_callback(self, id_list):
        update_roles_on_accounts.delay(affected_groups=id_list, deleted=False)



class UserProfileAPIView(AuthCommonAPIView, RecoveryModelMixin, GetProfileIDMixin):
    serializer_class = GenericUserProfileSerializer
    filter_backends  = [UserProfilesPermissions, SearchFilter, OrderingFilter,]
    ordering_fields  = ['id', 'time_created', 'last_updated', 'first_name', 'last_name']
    search_fields  = ['first_name', 'last_name']
    permission_classes = copy.copy(AuthCommonAPIView.permission_classes) + [UserProfilesPermissions]
    queryset = serializer_class.Meta.model.objects.order_by('-time_created')
    SOFTDELETE_CHANGESET_MODEL = UsermgtChangeSet

    def get(self, request, *args, **kwargs):
        exc_rd_fields = request.query_params.get('exc_rd_fields', [])
        if exc_rd_fields and isinstance(exc_rd_fields, str):
            exc_rd_fields = [exc_rd_fields]
        kwargs = self.kwargs_map(request, kwargs)
        kwargs['serializer_kwargs'] = {'from_read_view':True, 'exc_rd_fields': exc_rd_fields}
        #print('user profile get() , kwargs : %s, %s' % (kwargs, self.kwargs))
        return super().get(request=request, *args, **kwargs)

    def post(self, request, *args, **kwargs):
        kwargs['many'] = True
        kwargs['return_data_after_done'] = False
        kwargs['exc_wr_fields'] = ['quota__user_type', 'quota__user_id', 'emails__user_type', 'emails__user_id',
                'phones__user_type', 'phones__user_id','locations__user_type', 'locations__user_id',]
        kwargs['serializer_kwargs'] = {'from_edit_view':True}
        return  self.create(request, *args, **kwargs)

    def put(self, request, *args, **kwargs):
        kwargs['many'] = True
        kwargs['return_data_after_done'] = False
        kwargs['pk_src'] =  LimitQuerySetMixin.REQ_SRC_BODY_DATA
        kwargs['serializer_kwargs'] = {'from_edit_view':True}
        return  self.update(request, *args, **kwargs)

    def delete(self, request, *args, **kwargs):
        kwargs['many'] = True
        kwargs['status_ok'] = RestStatus.HTTP_202_ACCEPTED
        return self.destroy(request, *args, **kwargs)

    def patch(self, request, *args, **kwargs):
        kwargs['resource_content_type'] = ContentType.objects.get(app_label='user_management',
                model=self.serializer_class.Meta.model.__name__)
        return self.recovery(request=request, *args, **kwargs)

    def kwargs_map(self, request, kwargs):
        # if the argument `pk` is `me`, then update the value to profile ID of current login user
        if kwargs.get('pk', None) == 'me':
            account = request.user
            my_id = str(account.genericuserauthrelation.profile.pk)
            kwargs['pk'] = my_id
            self.kwargs['pk'] = my_id
        return kwargs


## --------------------------------------------------------------------------------------
class UserActivationView(AuthCommonAPIView, GetProfileIDMixin):
    serializer_class = AuthUserResetRequestSerializer
    permission_classes = copy.copy(AuthCommonAPIView.permission_classes) + [UserActivationPermission]

    def _reactivate_existing_account(self, req_body):
        _map = {str(x['profile']) : x for x in req_body if x.get('profile', None)}
        IDs = [str(x['profile']) for x in req_body if x.get('profile', None)]
        IDs = [int(x) for x in IDs if x.isdigit()]
        filter_kwargs = {'auth__isnull': False, 'pk__in': IDs,}
        profiles = GenericUserProfile.objects.filter( **filter_kwargs )
        for prof in profiles:
            prof.activate(new_account_data=None)
            data_item = _map.get(str(prof.pk), None)
            if data_item:
                req_body.remove(data_item)
        if profiles.exists():
            affected_items = list(profiles.values(*profiles.model.min_info_field_names))
            _login_profile = self.request.user.genericuserauthrelation.profile
            self._log_action(action_type='reactivate_account', request=self.request, affected_items=affected_items,
                model_cls=profiles.model,  profile_id=_login_profile.pk)

    def _create_new_auth_req(self, request, *args, **kwargs):
        resource_path = [UserMgtCfg.app_url] + UserMgtCfg.api_url[LoginAccountCreateView.__name__].split('/')
        resource_path.pop() # last one should be <slug:token>
        kwargs['many'] = True
        kwargs['return_data_after_done'] = True
        kwargs['status_ok'] = RestStatus.HTTP_202_ACCEPTED
        kwargs['pk_src'] =  LimitQuerySetMixin.REQ_SRC_BODY_DATA
        kwargs['pk_field_name'] = 'profile'
        kwargs['allow_create'] = True
        kwargs['serializer_kwargs'] = {
            # 'exc_rd_fields': ['id',],
            'msg_template_path': 'user_management/data/mail/body/user_activation_link_send.html',
            'subject_template' : 'user_management/data/mail/subject/user_activation_link_send.txt',
            'url_host': WEB_HOST,
            'url_resource':'/'.join(resource_path),
        }
        return self.update(request, *args, **kwargs)


    def post(self, request, *args, **kwargs):
        """
        Activate login account by admin, frontend has to provide:
        * user profile ID (only admin knows the profile ID)
        * chosen email ID that belongs to the profile, in order to send email with activation URL
        Once validation succeeds, backend has to do the following :
        * Turn on user profile active field
        * create new request in AuthUserResetRequest, for creating login account by the user later
        * Send mail with activation URL to notify the users
        """
        self._reactivate_existing_account(req_body=request.data)
        if any(request.data):
            err_args = ["rest_of_request_body", request.data]
            _logger.debug(None, *err_args, request=request)
            response = self._create_new_auth_req(request=request, *args, **kwargs)
        else:
            return_data = []
            response = RestResponse(return_data, status=RestStatus.HTTP_202_ACCEPTED)
        return response



class UserDeactivationView(AuthCommonAPIView, GetProfileIDMixin):
    serializer_class = GenericUserProfileSerializer
    permission_classes = copy.copy(AuthCommonAPIView.permission_classes) + [UserDeactivationPermission]

    def post(self, request, *args, **kwargs):
        """
        set active field to `False` in selected user profile
        delete valid auth request issued by the user (if exists)
        optionally delete login accounts
        """
        # each authorized user can only deactivate his/her own account,
        # while superuser can deactivate several accounts (of other users) in one API call.
        prof_qset = self.get_queryset(pk_field_name='id', pk_src=LimitQuerySetMixin.REQ_SRC_BODY_DATA)
        _map = {str(x['id']) : x.get('remove_account', False) for x in request.data if x.get('id',None) is not None}
        for prof in prof_qset:
            remove_account = _map[str(prof.pk)]
            prof.deactivate(remove_account=remove_account)

        _profile = request.user.genericuserauthrelation.profile
        _item_list = prof_qset.values( *prof_qset.model.min_info_field_names )
        self._log_action(action_type='deactivate_account', request=request, affected_items=list(_item_list),
                model_cls=type(_profile),  profile_id=_profile.pk)
        return RestResponse(status=None)


## --------------------------------------------------------------------------------------
class AuthTokenReadAPIView(APIView):
    renderer_classes = [JSONRenderer]
    get = check_auth_req_token(
            fn_succeed=AuthTokenCheckMixin.token_valid,
            fn_failure=AuthTokenCheckMixin.token_expired
        )


class LoginAccountCreateView(APIView, UserEditViewLogMixin):
    renderer_classes = [JSONRenderer]

    def _post_token_valid(self, request, *args, **kwargs):
        response_data = {}
        auth_req = kwargs.pop('auth_req', None)
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
            response_data['created'] = True
        else:
            response_data['created'] = False
            response_data['reason'] = 'already created before'
        return {'data': response_data, 'status':None}

    post = check_auth_req_token(fn_succeed=_post_token_valid, fn_failure=AuthTokenCheckMixin.token_expired)


class UsernameRecoveryRequestView(APIView, UserEditViewLogMixin):
    """ for unauthenticated users who registered but forget their username """
    renderer_classes = [JSONRenderer]

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
    renderer_classes = [JSONRenderer]

    def post(self, request, *args, **kwargs):
        addr = request.data.get('addr', '').strip()
        useremail, profile, account = get_profile_account_by_email(addr=addr, request=request)
        self._send_req_mail(request=request, kwargs=kwargs, profile=profile,
                account=account, useremail=useremail, addr=addr)
        # always respond OK status even if it failed, to avoid malicious email enumeration
        return RestResponse(data=None, status=RestStatus.HTTP_202_ACCEPTED)  #  HTTP_401_UNAUTHORIZED

    def _send_req_mail(self, request, kwargs, profile, account, useremail, addr):
        if profile is None or account is None or useremail is None:
            return
        resource_path = [UserMgtCfg.app_url] + UserMgtCfg.api_url[UnauthPasswordResetView.__name__].split('/')
        resource_path.pop() # last one should be <slug:token>
        serializer_kwargs = {
            'msg_template_path': 'user_management/data/mail/body/passwd_reset_request.html',
            'subject_template' : 'user_management/data/mail/subject/passwd_reset_request.txt',
            'url_host': WEB_HOST,  'url_resource':'/'.join(resource_path),
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
            self.update(request, **kwargs)
        except Exception as e:
            fully_qualified_cls_name = '%s.%s' % (type(e).__module__, type(e).__qualname__)
            log_msg = ['excpt_type', fully_qualified_cls_name, 'excpt_msg', e, 'email', addr,
                    'email_id', useremail.pk, 'profile', profile.minimum_info]
            _logger.error(None, *log_msg, exc_info=True, request=request)
        request.user = user_bak


class UnauthPasswordResetView(APIView, UserEditViewLogMixin):
    renderer_classes = [JSONRenderer]

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
        return {'data': response_data, 'status':status}

    patch = check_auth_req_token(fn_succeed=_patch_token_valid, fn_failure=AuthTokenCheckMixin.token_expired)




class CommonAuthAccountEditMixin:
    def run(self, **kwargs):
        account = kwargs.get('account', None)
        profile_id = account.genericuserauthrelation.profile.pk
        _serializer = LoginAccountSerializer(**kwargs)
        _serializer.is_valid(raise_exception=True)
        _serializer.save()
        self._log_action(action_type=self.log_action_type, request=self.request, affected_items=[account.minimum_info],
                    model_cls=type(account),  profile_id=profile_id)
        return RestResponse(data={}, status=None)


class AuthUsernameEditAPIView(AuthCommonAPIView, CommonAuthAccountEditMixin, GetProfileIDMixin):
    """ for authenticated user who attempts to update username of their account """
    log_action_type = "update_username"

    def patch(self, request, *args, **kwargs):
        serializer_kwargs = {
            'data': request.data, 'uname_required':True,  'old_uname_required': True,
            'account': request.user, 'auth_req': None,  'many': False,
        }
        return self.run(**serializer_kwargs)


class AuthPasswdEditAPIView(AuthCommonAPIView, CommonAuthAccountEditMixin, GetProfileIDMixin):
    """ for authenticated user who attempts to update password of their account """
    log_action_type = "update_password"

    def patch(self, request, *args, **kwargs):
        serializer_kwargs = {
            'data': request.data, 'passwd_required':True,  'confirm_passwd': True, 'old_passwd_required': True,
            'account': request.user, 'auth_req': None,  'many': False,
        }
        return self.run(**serializer_kwargs)


class RemoteAccessTokenAPIView(AuthCommonAPIView, GetProfileIDMixin):
    """
    API endpoint for authenticated user requesting a new access token
    which is used only in specific service in different network domain
    """
    def post(self, request, *args, **kwargs):
        audience = request.data.get('audience', [])
        audience = self._filter_resource_services(audience, exclude=['web', 'api', 'usermgt'])
        if audience:
            signed = self._gen_signed_token(request=request, audience=audience)
            data = {'access_token': signed}
            status = RestStatus.HTTP_200_OK
        else:
            data = {drf_settings.NON_FIELD_ERRORS_KEY: 'invalid audience field'}
            status = RestStatus.HTTP_400_BAD_REQUEST
        return RestResponse(data=data, status=status)

    def _filter_resource_services(self, audience, exclude=None):
        exclude = exclude or []
        allowed = cors_cfg.ALLOWED_ORIGIN.keys()
        allowed = set(allowed) - set(exclude)
        filtered = filter(lambda a: a in allowed, audience)
        return list(filtered)

    def _gen_signed_token(self, request, audience):
        account = request.user
        profile = self.get_profile(account=account)
        profile_serial = profile.serializable(present=['id', 'roles','quota'], services_label=audience)
        if not profile_serial['roles']:
            errmsg = "the user does not have access to these resource services listed in audience field"
            err_detail = {drf_settings.NON_FIELD_ERRORS_KEY: [errmsg],}
            raise PermissionDenied(detail=err_detail) ##  SuspiciousOperation
        keystore = create_keystore_helper(cfg=django_settings.AUTH_KEYSTORE, import_fn=import_string)
        now_time = datetime.utcnow()
        expiry = now_time + timedelta(seconds=django_settings.JWT_REMOTE_ACCESS_TOKEN_VALID_PERIOD)
        token = JWT()
        payload = {
            'acc_id'  :account._meta.pk.value_to_string(account),
            'prof_id' :profile_serial.pop('id'),
            'jwks_url':'%s/jwks' % (API_GATEWAY_HOST), # link to fetch all valid JWK,
            'aud':audience,  'iat':now_time,  'exp':expiry,
        }
        payload.update(profile_serial) # roles, quota
        token.payload.update(payload)
        return token.encode(keystore=keystore)



