import copy
import logging
from datetime import datetime, timezone, timedelta

from django.conf   import  settings as django_settings
from django.core.exceptions     import ValidationError
from django.http.response    import HttpResponseBase
from django.db.models        import Count
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
from common.auth.jwt import JWT
from common.views.mixins   import  LimitQuerySetMixin, UserEditViewLogMixin, BulkUpdateModelMixin
from common.views.api      import  AuthCommonAPIView, AuthCommonAPIReadView
from common.views.filters  import  ClosureTableFilter
from common.util.python.async_tasks  import  sendmail as async_send_mail, default_error_handler as async_default_error_handler

from ..apps   import UserManagementConfig as UserMgtCfg
from ..models.base import GenericUserGroup, GenericUserGroupClosure, GenericUserProfile, UsermgtChangeSet
from ..async_tasks import update_roles_on_accounts

from ..serializers import RoleSerializer, GenericUserGroupSerializer
from ..serializers import GenericUserProfileSerializer, AuthUserResetRequestSerializer
from ..serializers import LoginAccountSerializer
from ..serializers import GenericUserRoleAssigner, GenericUserGroupRelationAssigner

from ..permissions import RolePermissions, AppliedRolePermissions, AppliedGroupPermissions, UserGroupsPermissions
from ..permissions import UserDeactivationPermission, UserActivationPermission, UserProfilesPermissions

from .constants import  _PRESERVED_ROLE_IDS, MAX_NUM_FORM, WEB_HOST

# * All classes within this module can share one logger, because logger is unique by given name
#   as the argument on invoking getLogger(), that means subsequent call with the same logger name
#   will get the same logger instance.
# * Logger is thread safe, multiple view/serializer/model instances in project can access the same
#   logger instance simultaneously without data corruption.
# * It seems safe to load logger at module level, because django framework loads this module
#   after parsing logging configuration at settings.py
_logger = logging.getLogger(__name__)


class RoleAPIView(AuthCommonAPIView):
    serializer_class = RoleSerializer
    filter_backends = [RolePermissions, SearchFilter, OrderingFilter,]
    ordering_fields  = ['id', 'name']
    search_fields  = ['name']
    # add django model permission obj
    permission_classes = copy.copy(AuthCommonAPIView.permission_classes) + [RolePermissions]
    PRESERVED_ROLE_IDS = _PRESERVED_ROLE_IDS
    queryset = serializer_class.Meta.model.objects.all()

    def get(self, request, *args, **kwargs):
        if request.query_params.get('skip_preserved_role', None) is not None:
            kwargs['pk_skip_list'] = self.PRESERVED_ROLE_IDS
        return  super().get(request, *args, **kwargs)

    def post(self, request, *args, **kwargs):
        kwargs['many'] = True
        kwargs['return_data_after_done'] = True
        return  self.create(request, *args, **kwargs)

    def put(self, request, *args, **kwargs):
        kwargs['many'] = True
        kwargs['return_data_after_done'] = False
        kwargs['pk_src'] =  LimitQuerySetMixin.REQ_SRC_BODY_DATA
        kwargs['pk_skip_list'] = self.PRESERVED_ROLE_IDS
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


class AppliedRoleReadAPIView(AuthCommonAPIReadView):
    serializer_class = GenericUserRoleAssigner
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


class AppliedGroupReadAPIView(AuthCommonAPIReadView):
    serializer_class = GenericUserGroupRelationAssigner
    queryset = serializer_class.Meta.model.objects.order_by('-id')
    permission_classes = copy.copy(AuthCommonAPIView.permission_classes) + [AppliedGroupPermissions]
    filter_backends = [AppliedGroupPermissions]

    def get(self, request, *args, **kwargs):
        # filter before calling get(), which makes get() invoke list()
        grp_id = kwargs.pop('pk', 0)
        self.queryset = self.queryset.filter(group__pk=grp_id)
        return super().get(request, *args, **kwargs)



class UserGroupsAPIView(AuthCommonAPIView, RecoveryModelMixin):
    serializer_class = GenericUserGroupSerializer
    filter_backends = [UserGroupsPermissions, SearchFilter, ClosureTableFilter, OrderingFilter,] #  
    closure_model_cls = GenericUserGroupClosure
    ordering_fields  = ['id', 'name', 'usr_cnt']
    # `ancestors__ancestor__name` already covers `name` field of each model instance
    search_fields  = ['ancestors__ancestor__name']
    permission_classes = copy.copy(AuthCommonAPIView.permission_classes) + [UserGroupsPermissions]
    queryset = serializer_class.Meta.model.objects.annotate(usr_cnt=Count('profiles__profile'))
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
        return  self.create(request, *args, **kwargs)

    def put(self, request, *args, **kwargs):
        kwargs['many'] = True
        kwargs['return_data_after_done'] = False
        kwargs['pk_src'] =  LimitQuerySetMixin.REQ_SRC_BODY_DATA
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



class UserProfileAPIView(AuthCommonAPIView, RecoveryModelMixin):
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
        return  self.create(request, *args, **kwargs)

    def put(self, request, *args, **kwargs):
        kwargs['many'] = True
        kwargs['return_data_after_done'] = False
        kwargs['pk_src'] =  LimitQuerySetMixin.REQ_SRC_BODY_DATA
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
class UserActivationView(AuthCommonAPIView):
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



class UserDeactivationView(AuthCommonAPIView):
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


