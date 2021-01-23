import copy
import operator
import logging

from django.conf   import  settings as django_settings
from django.core.exceptions     import ValidationError
from django.core.cache          import caches as DjangoBuiltinCaches
from django.http.response    import HttpResponseBase
from django.db.models        import Prefetch, Count, QuerySet
from django.contrib.contenttypes.models  import ContentType
from django.contrib.auth     import logout

from rest_framework             import status as RestStatus
from rest_framework.views       import APIView
from rest_framework.viewsets    import ModelViewSet
from rest_framework.filters     import BaseFilterBackend, OrderingFilter, SearchFilter
from rest_framework.renderers   import JSONRenderer
from rest_framework.response    import Response as RestResponse
from rest_framework.permissions import DjangoModelPermissions, DjangoObjectPermissions
from rest_framework.settings    import api_settings as drf_settings

from softdelete.views import RecoveryModelMixin
from common.views.mixins      import  LimitQuerySetMixin
from common.views.filters     import  DateTimeRangeFilter
from common.views             import  AuthCommonAPIView, AuthCommonAPIReadView
from common.auth.backends     import  IsSuperUser

from ..apps   import UserManagementConfig as UserMgtCfg
from ..models import GenericUserGroup, GenericUserGroupClosure, GenericUserProfile, UsermgtChangeSet
from ..queryset import UserActionSet
from ..async_tasks import update_roles_on_accounts

from ..serializers import AuthPermissionSerializer, AuthRoleSerializer, QuotaUsageTypeSerializer, GenericUserGroupSerializer
from ..serializers import GenericUserProfileSerializer, AuthUserResetRequestSerializer
from ..serializers import LoginAccountSerializer
from ..serializers import GenericUserAppliedRoleSerializer, GenericUserGroupRelationSerializer

from ..permissions import AuthRolePermissions, AppliedRolePermissions, AppliedGroupPermissions, UserGroupsPermissions
from ..permissions import UserDeactivationPermission, UserActivationPermission, UserProfilesPermissions

from .constants import LOGIN_URL, _PRESERVED_ROLE_IDS, MAX_NUM_FORM


# * All classes within this module can share one logger, because logger is unique by given name
#   as the argument on invoking getLogger(), that means subsequent call with the same logger name
#   will get the same logger instance.
# * Logger is thread safe, multiple view/serializer/model instances in project can access the same
#   logger instance simultaneously without data corruption.
# * It seems safe to load logger at module level, because django framework loads this module
#   after parsing logging configuration at settings.py
_logger = logging.getLogger(__name__)


class GetProfileIDMixin:
    def get_profile_id(self, request):
        # TODO, should be not-implemented error , let other apps subclass this mixin
        account = request.user
        from django.contrib.auth.models import User as AuthUser
        if account and isinstance(account, AuthUser):
            profile = account.genericuserauthrelation.profile
            profile_id = profile.pk
        else:
            # which means unauthenticated accesses happened to model instances,
            # application developers should analyze log data and determine whether this
            # part of the system has been compromised.
            profile_id = -1
        return str(profile_id)



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




class ClosureTableFilter(BaseFilterBackend):
    def filter_queryset(self, request, queryset, view):
        # filter out the instance whose depth = 0 only in read view
        if (not hasattr(view, 'closure_model_cls')) or (view.closure_model_cls is None):
            return queryset
        closure_qset = view.closure_model_cls.objects.filter(depth__gt=0)
        field_names  = request.query_params.get('fields', '').split(',')
        prefetch_objs = []
        if 'ancestors' in field_names :
            prefetch_objs.append(Prefetch('ancestors',   queryset=closure_qset))
        if 'descendants' in field_names :
            prefetch_objs.append(Prefetch('descendants', queryset=closure_qset))
        queryset = queryset.prefetch_related( *prefetch_objs )
        ####err_args = ["low_level_prefetch_query", queryset.query] # TODO, find better way to log raw SQL
        ####_logger.debug(None, *err_args, request=request)
        return queryset



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
        kwargs['serializer_kwargs'] = {'from_read_view':True, 'exc_rd_fields': ['ancestors__id'],}
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
        kwargs['serializer_kwargs'] = {'from_read_view':True,}
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
        from .html import LoginAccountCreateView
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
            'url_host': "{protocol}://{hostname}".format(
                protocol=request._request.scheme,
                hostname=request._request._get_raw_host()
            ),
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
class LogoutView(APIView):
    renderer_classes = [JSONRenderer]

    def post(self, request, *args, **kwargs):
        status = RestStatus.HTTP_401_UNAUTHORIZED
        account = request.user
        if account is not None and account.is_staff:
            username = account.username
            profile_id = account.genericuserauthrelation.profile.pk
            logout(request)
            status = RestStatus.HTTP_200_OK
            log_msg = ['action', 'logout', 'username', username, 'profile_id', profile_id]
            _logger.info(None, *log_msg, request=request)
        return RestResponse(data={}, status=status)




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




class UserActionHistoryAPIReadView(AuthCommonAPIReadView, GetProfileIDMixin):
    queryset = None
    serializer_class = None
    max_page_size = 13
    filter_backends  = [DateTimeRangeFilter,] #[OrderingFilter,]
    #ordering_fields  = ['action', 'timestamp']
    #search_fields  = ['action', 'ipaddr',]
    search_field_map = {
        DateTimeRangeFilter.search_param[0] : {'field_name':'timestamp', 'operator': operator.and_},
    }

    def get(self, request, *args, **kwargs):
        # this API endpoint doesn't need to retrieve single action log
        queryset = UserActionSet(request=request, paginator=self.paginator)
        queryset = self.filter_queryset(queryset)
        page = self.paginate_queryset(queryset=queryset)
        log_args = ['user_action_page', page]
        _logger.debug(None, *log_args, request=request)
        return  self.paginator.get_paginated_response(data=page)


class DynamicLoglevelAPIView(AuthCommonAPIView, GetProfileIDMixin):
    permission_classes = copy.copy(AuthCommonAPIView.permission_classes) + [IsSuperUser]
    # unique logger name by each module hierarchy
    def get(self, request, *args, **kwargs):
        status = RestStatus.HTTP_200_OK
        data = []
        cache_loglvl_change = DjangoBuiltinCaches['log_level_change']
        logger_names = django_settings.LOGGING['loggers'].keys()
        for name in logger_names:
            modified_setup = cache_loglvl_change.get(name, None)
            if modified_setup:
                level = modified_setup #['level']
            else:
                level = logging.getLogger(name).level
            data.append({'name': name, 'level': level, 'modified': modified_setup is not None})
        return RestResponse(data=data, status=status)

    def _change_level(self, request):
        status = RestStatus.HTTP_200_OK
        logger_names = django_settings.LOGGING['loggers'].keys()
        err_args = []
        validated_data = {} if request.method == 'PUT' else []
        for change in request.data:
            err_arg = {}
            logger_name = change.get('name', None)
            new_level = change.get('level', None)
            if not logger_name in logger_names:
                err_arg['name'] = ['logger name not found']
            if request.method == 'PUT':
                try:
                    new_level = int(new_level)
                except (ValueError, TypeError) as e:
                    err_arg['level'] = [str(e)]
            if any(err_arg):
                status = RestStatus.HTTP_400_BAD_REQUEST
            else:
                if request.method == 'PUT':
                    validated_data[logger_name] = new_level
                elif request.method == 'DELETE':
                    validated_data.append(logger_name)
            err_args.append(err_arg)

        log_msg = ['action', 'set_log_level', 'request.method', request.method, 'request_data', request.data,
                'validated_data', validated_data]
        if status == RestStatus.HTTP_200_OK:
            cache_loglvl_change = DjangoBuiltinCaches['log_level_change']
            resp_data = None
            if request.method == 'PUT':
                cache_loglvl_change.set_many(validated_data)
                for name,level in validated_data.items():
                    logging.getLogger(name).setLevel(level)
            elif request.method == 'DELETE':
                resp_data = []
                cache_loglvl_change.delete_many(validated_data)
                for name in validated_data:
                    level = django_settings.LOGGING['loggers'][name]['level']
                    level = getattr(logging, level)
                    logging.getLogger(name).setLevel(level)
                    resp_data.append({'name': name, 'default_level':level})
            loglevel = logging.INFO
        else:
            loglevel = logging.WARNING
            resp_data = err_args
        _logger.log(loglevel, None, *log_msg, request=request)
        return RestResponse(data=resp_data, status=status)


    def put(self, request, *args, **kwargs):
        return self._change_level(request=request)

    def delete(self, request, *args, **kwargs): # clean up cached log lovel
        return self._change_level(request=request)


class SessionManageAPIView(AuthCommonAPIView, GetProfileIDMixin):
    """
    Provide log-in user an option to view a list of sessions he/she started,
    so user can invalidate any session of the list.
    It depends on whether your application/site needs to restrict number of sessions on each logged-in users,
    for this staff-only backend site, it would be better to restrict only one session per logged-in user,
    while customer frontend portal could allow multiple sessions for each user, which implicitly means user
    can log into customer frontend portal on different device (e.g. laptops/modile devices ... etc.)
    """
    # TODO, finish implementation
    pass


