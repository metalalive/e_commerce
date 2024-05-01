import copy
import logging
from datetime import datetime, timezone, timedelta

from django.conf import settings as django_settings
from django.core.exceptions import ValidationError
from django.http.response import HttpResponseBase
from django.db.models import Count
from django.db.models.constants import LOOKUP_SEP
from django.contrib.contenttypes.models import ContentType

from rest_framework import status as RestStatus
from rest_framework.generics import GenericAPIView
from rest_framework.views import APIView
from rest_framework.viewsets import ModelViewSet
from rest_framework.filters import OrderingFilter, SearchFilter
from rest_framework.renderers import JSONRenderer
from rest_framework.response import Response as RestResponse
from rest_framework.permissions import DjangoModelPermissions, DjangoObjectPermissions
from rest_framework.exceptions import PermissionDenied, ParseError
from rest_framework.settings import api_settings as drf_settings

from softdelete.views import RecoveryModelMixin
from ecommerce_common.auth.jwt import JWT
from ecommerce_common.views.mixins import (
    LimitQuerySetMixin,
    UserEditViewLogMixin,
    BulkUpdateModelMixin,
)
from ecommerce_common.views.api import AuthCommonAPIView, AuthCommonAPIReadView
from ecommerce_common.views.filters import ClosureTableFilter

from ..apps import UserManagementConfig as UserMgtCfg
from ..models.base import (
    GenericUserGroup,
    GenericUserGroupClosure,
    GenericUserProfile,
    UsermgtChangeSet,
)
from ..async_tasks import update_accounts_privilege

from ..serializers import (
    RoleSerializer,
    GenericUserGroupSerializer,
    GenericUserProfileSerializer,
)
from ..serializers import GenericUserRoleAssigner, GenericUserGroupRelationAssigner
from ..serializers.auth import UnauthRstAccountReqSerializer

from ..permissions import RolePermissions, UserGroupsPermissions
from ..permissions import (
    AccountDeactivationPermission,
    AccountActivationPermission,
    UserProfilesPermissions,
)

from .constants import _PRESERVED_ROLE_IDS, MAX_NUM_FORM, WEB_HOST

# * All classes within this module can share one logger, because logger is unique by given name
#   as the argument on invoking getLogger(), that means subsequent call with the same logger name
#   will get the same logger instance.
# * Logger is thread safe, multiple view/serializer/model instances in project can access the same
#   logger instance simultaneously without data corruption.
# * It seems safe to load logger at module level, because django framework loads this module
#   after parsing logging configuration at settings.py
_logger = logging.getLogger(__name__)

MAIL_DATA_BASEPATH = django_settings.BASE_DIR.joinpath("user_management/data/mail")


class RoleAPIView(AuthCommonAPIView):
    serializer_class = RoleSerializer
    filter_backends = [
        RolePermissions,
        SearchFilter,
        OrderingFilter,
    ]
    ordering_fields = ["id", "name"]
    search_fields = ["name"]
    # add django model permission obj
    permission_classes = copy.copy(AuthCommonAPIView.permission_classes) + [
        RolePermissions
    ]
    PRESERVED_ROLE_IDS = _PRESERVED_ROLE_IDS
    queryset = serializer_class.Meta.model.objects.all()

    def post(self, request, *args, **kwargs):
        kwargs["many"] = True
        kwargs["return_data_after_done"] = True
        return self.create(request, *args, **kwargs)

    def put(self, request, *args, **kwargs):
        kwargs["many"] = True
        kwargs["return_data_after_done"] = False
        kwargs["pk_src"] = LimitQuerySetMixin.REQ_SRC_BODY_DATA
        kwargs["pk_skip_list"] = self.PRESERVED_ROLE_IDS
        log_msg = ["filtered_request_data", request.data]
        _logger.debug(None, *log_msg, request=request)
        return self.update(request, *args, **kwargs)

    def delete(self, request, *args, **kwargs):
        try:
            IDs = self.get_IDs(
                pk_param_name="ids",
                pk_field_name="id",
            )
            IDs = list(map(int, IDs))
        except (ValueError, TypeError) as e:
            raise ParseError("ids field has to be a list of number")
        # conflict happenes if frontend attempts to delete preserved roles (e.g. admin role)
        reserved_role_ids = set(self.PRESERVED_ROLE_IDS) & set(IDs)
        if reserved_role_ids:
            errmsg = "not allowed to delete preserved role ID = {}".format(
                str(reserved_role_ids)
            )
            context = {drf_settings.NON_FIELD_ERRORS_KEY: [errmsg]}
            response = RestResponse(data=context, status=RestStatus.HTTP_409_CONFLICT)
        else:
            kwargs["many"] = True
            kwargs["pk_skip_list"] = self.PRESERVED_ROLE_IDS
            response = self.destroy(request, *args, **kwargs)
        return response


## end of class RoleAPIView


class UserGroupsAPIView(AuthCommonAPIView, RecoveryModelMixin):
    serializer_class = GenericUserGroupSerializer
    filter_backends = [
        UserGroupsPermissions,
        SearchFilter,
        ClosureTableFilter,
        OrderingFilter,
    ]  #
    closure_model_cls = GenericUserGroupClosure
    ordering_fields = ["id", "usr_cnt"]
    # `ancestors__ancestor__name` already covers `name` field of each model instance
    search_fields = [
        "ancestors__ancestor__name",
        "emails__addr",
        "locations__locality",
        "roles__role__name",
        "locations__street",
        "locations__detail",
    ]
    permission_classes = copy.copy(AuthCommonAPIView.permission_classes) + [
        UserGroupsPermissions
    ]
    SOFTDELETE_CHANGESET_MODEL = UsermgtChangeSet

    def get(self, request, *args, **kwargs):
        self.queryset = self.serializer_class.Meta.model.objects.annotate(
            usr_cnt=Count("profiles__profile")
        )
        return super().get(request=request, *args, **kwargs)

    def post(self, request, *args, **kwargs):
        kwargs["many"] = True
        kwargs["return_data_after_done"] = True
        return self.create(request, *args, **kwargs)

    def put(self, request, *args, **kwargs):
        kwargs["many"] = True
        kwargs["return_data_after_done"] = False
        kwargs["pk_src"] = LimitQuerySetMixin.REQ_SRC_BODY_DATA
        return self.update(request, *args, **kwargs)

    def delete(self, request, *args, **kwargs):
        kwargs["many"] = True
        kwargs["pk_src"] = LimitQuerySetMixin.REQ_SRC_BODY_DATA
        kwargs["status_ok"] = RestStatus.HTTP_202_ACCEPTED
        # semantic: accepted, will be deleted after a point of time which no undelete operation is performed
        return self.destroy(request, *args, **kwargs)

    def patch(self, request, *args, **kwargs):
        kwargs["return_data_after_done"] = True
        kwargs["resource_content_type"] = ContentType.objects.get(
            app_label="user_management", model=self.serializer_class.Meta.model.__name__
        )
        return self.recovery(
            request=request, profile_id=request.user.profile.id, *args, **kwargs
        )

    def delete_success_callback(self, id_list):
        update_accounts_privilege.delay(affected_groups=id_list, deleted=True)

    def recover_success_callback(self, id_list):
        update_accounts_privilege.delay(affected_groups=id_list, deleted=False)


class UserProfileAPIView(AuthCommonAPIView, RecoveryModelMixin):
    serializer_class = GenericUserProfileSerializer
    filter_backends = [
        SearchFilter,
        OrderingFilter,
    ]
    ordering_fields = ["id", "time_created", "last_updated", "first_name", "last_name"]
    search_fields = [
        "first_name",
        "last_name",
        "emails__addr",
        "locations__province",
        "locations__locality",
        "locations__street",
        "locations__detail",
        "groups__group__name",
        "roles__role__name",
    ]
    permission_classes = copy.copy(AuthCommonAPIView.permission_classes) + [
        UserProfilesPermissions
    ]
    SOFTDELETE_CHANGESET_MODEL = UsermgtChangeSet

    def get(self, request, *args, **kwargs):
        self.queryset = self.serializer_class.Meta.model.objects.order_by(
            "-time_created"
        )
        # if the argument `pk` is `me`, then update the value to profile ID of current login user
        if kwargs.get("pk", None) == "me":
            account = request.user
            my_id = str(account.profile.pk)
            kwargs["pk"] = my_id
            self.kwargs["pk"] = my_id
        return super().get(request=request, *args, **kwargs)

    def post(self, request, *args, **kwargs):
        kwargs["many"] = True
        kwargs["return_data_after_done"] = True
        return self.create(request, *args, **kwargs)

    def put(self, request, *args, **kwargs):
        kwargs["many"] = True
        kwargs["return_data_after_done"] = False
        kwargs["pk_src"] = LimitQuerySetMixin.REQ_SRC_BODY_DATA
        return self.update(request, *args, **kwargs)

    def delete(self, request, *args, **kwargs):
        kwargs["many"] = True
        kwargs["status_ok"] = RestStatus.HTTP_202_ACCEPTED
        kwargs["pk_src"] = LimitQuerySetMixin.REQ_SRC_BODY_DATA
        kwargs["return_data_after_done"] = False
        response = self.destroy(request, *args, **kwargs)
        if getattr(self, "_force_logout", False):
            jwt_name_refresh_token = django_settings.JWT_NAME_REFRESH_TOKEN
            response.set_cookie(
                key=jwt_name_refresh_token,
                value="",
                max_age=0,
                domain=None,
                path=django_settings.SESSION_COOKIE_PATH,
                secure=django_settings.SESSION_COOKIE_SECURE or None,
                samesite=django_settings.SESSION_COOKIE_SAMESITE,
                httponly=True,
            )
            response.data = {"message": "force logout"}
        return response

    def patch(self, request, *args, **kwargs):
        kwargs["return_data_after_done"] = True
        kwargs["resource_content_type"] = ContentType.objects.get(
            app_label="user_management", model=self.serializer_class.Meta.model.__name__
        )
        return self.recovery(
            request=request, profile_id=request.user.profile.id, *args, **kwargs
        )

    def delete_success_callback(self, id_list):
        account = self.request.user
        profile_id = account.profile.id
        self._force_logout = profile_id in id_list


## --------------------------------------------------------------------------------------
class AccountActivationView(AuthCommonAPIView):
    serializer_class = UnauthRstAccountReqSerializer
    permission_classes = copy.copy(AuthCommonAPIView.permission_classes) + [
        AccountActivationPermission
    ]

    def _reactivate_existing_account(self, request):
        req_body = request.data
        _get_id_fn = lambda d: d.get("profile")
        IDs = filter(_get_id_fn, req_body)
        IDs = list(map(_get_id_fn, IDs))
        filter_kwargs = {
            LOOKUP_SEP.join(["account", "isnull"]): False,
            LOOKUP_SEP.join(["id", "in"]): IDs,
        }
        try:
            profiles = GenericUserProfile.objects.filter(**filter_kwargs)
            for prof in profiles:
                prof.activate(new_account_data=None)
            remove_req_items = filter(
                lambda d: profiles.filter(id=d.get("profile")).exists(), req_body
            )
            for req_item in remove_req_items:
                req_body.remove(req_item)
            log_args = ["rest_of_request_body", request.data]
            _logger.debug(None, *log_args, request=request)
            if profiles.exists():
                affected_items = list(profiles.values("id", "first_name", "last_name"))
                _login_profile = request.user.profile
                self._log_action(
                    action_type="reactivate_account",
                    request=request,
                    affected_items=affected_items,
                    model_cls=profiles.model,
                    profile_id=_login_profile.id,
                )
        except ValueError as e:
            err_args = ["err_msg", e.args[0]]
            _logger.debug(None, *err_args, request=request)
            raise ParseError()

    def _create_new_auth_req(self, request, *args, **kwargs):
        resource_path = UserMgtCfg.api_url["LoginAccountCreateView"].split("/")
        resource_path.pop()  # last one should be <slug:token>
        kwargs["many"] = True
        kwargs["return_data_after_done"] = True
        kwargs["status_ok"] = RestStatus.HTTP_202_ACCEPTED
        kwargs["pk_src"] = LimitQuerySetMixin.REQ_SRC_BODY_DATA
        kwargs["serializer_kwargs"] = {
            "msg_template_path": MAIL_DATA_BASEPATH.joinpath(
                "body/user_activation_link_send.html"
            ),
            "subject_template": MAIL_DATA_BASEPATH.joinpath(
                "subject/user_activation_link_send.txt"
            ),
            "url_host": WEB_HOST,
            "url_resource": "/".join(resource_path),
        }
        return self.create(request, *args, **kwargs)

    def post(self, request, *args, **kwargs):
        """
        Activate login account of given users in admin site, frontend client has to provide:
        * user profile ID
        * chosen email ID recorded in the user profile
        Once validation succeeds, backend has to do the following :
        * create new request in UnauthResetAccountRequest, for creating login account for the user
        * Send mail with activation URL to notify the users
        """
        self._reactivate_existing_account(request=request)
        if any(request.data):
            response = self._create_new_auth_req(request=request, *args, **kwargs)
        else:
            return_data = []
            response = RestResponse(return_data, status=RestStatus.HTTP_200_OK)
        return response


class AccountDeactivationView(AuthCommonAPIView):
    permission_classes = copy.copy(AuthCommonAPIView.permission_classes) + [
        AccountDeactivationPermission
    ]

    def post(self, request, *args, **kwargs):
        """
        * delete valid reset requests associated to the user (if exists)
        * either deleting login accounts or setting `is_active` field to false
        """
        pk_field_name = "profile"
        prof_ids = self.get_IDs(
            pk_field_name=pk_field_name, pk_src=LimitQuerySetMixin.REQ_SRC_BODY_DATA
        )
        prof_qset = GenericUserProfile.objects.filter(id__in=prof_ids)
        _map = {
            x[pk_field_name]: x.get("remove_account", False)
            for x in request.data
            if x.get(pk_field_name, None)
        }
        for prof in prof_qset:
            remove_account = _map.get(prof.pk, False)
            prof.deactivate(remove_account=remove_account)

        _profile = request.user.profile
        _item_list = prof_qset.values("id", "first_name", "last_name")
        self._log_action(
            action_type="deactivate_account",
            request=request,
            affected_items=list(_item_list),
            model_cls=type(_profile),
            profile_id=_profile.pk,
        )
        return RestResponse(status=None)
