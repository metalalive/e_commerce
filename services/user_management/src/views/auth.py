import copy
import logging
from datetime import datetime, timezone, timedelta

from django.conf import settings as django_settings
from django.http.response import StreamingHttpResponse as DjangoStreamingHttpResponse
from django.utils.http import http_date
from django.utils.module_loading import import_string
from django.contrib.auth import authenticate

from rest_framework import status as RestStatus
from rest_framework.generics import GenericAPIView
from rest_framework.views import APIView
from rest_framework.renderers import JSONRenderer
from rest_framework.response import Response as RestResponse
from rest_framework.permissions import IsAuthenticated
from rest_framework.exceptions import PermissionDenied
from rest_framework.settings import api_settings as drf_settings

from ecommerce_common.auth.jwt import JWT, stream_jwks_file
from ecommerce_common.auth.keystore import create_keystore_helper
from ecommerce_common.auth.django.login import jwt_based_login
from ecommerce_common.auth.django.authentication import (
    RefreshJWTauthentication,
    IsStaffUser,
)
from ecommerce_common.cors import config as cors_cfg
from ecommerce_common.csrf.middleware import csrf_protect_m
from ecommerce_common.util.async_tasks import (
    sendmail as async_send_mail,
    default_error_handler as async_default_error_handler,
)
from ecommerce_common.views.mixins import (
    LimitQuerySetMixin,
    UserEditViewLogMixin,
    BulkCreateModelMixin,
)
from ecommerce_common.views.api import AuthCommonAPIView, AuthCommonAPIReadView

from ..apps import UserManagementConfig as UserMgtCfg
from ..serializers import PermissionSerializer
from ..serializers.auth import UnauthRstAccountReqSerializer, LoginAccountSerializer
from ..serializers.common import serialize_profile_quota, serialize_profile_permissions
from ..permissions import ModelLvlPermsPermissions
from ..util import render_mail_content
from .common import check_auth_req_token, get_profile_by_email
from .constants import WEB_HOST

_logger = logging.getLogger(__name__)

MAIL_DATA_BASEPATH = django_settings.APP_DIR.joinpath("data/mail")


class PermissionView(AuthCommonAPIReadView):
    """
    this API class should provide a set of pre-defined roles, which include a set of permissions
    granted to the role, in order for users in staff site to easily apply those permissions to application
    , instead of simply passing all django-defined permissions to mess user interface
    """

    serializer_class = PermissionSerializer
    permission_classes = copy.copy(AuthCommonAPIReadView.permission_classes) + [
        ModelLvlPermsPermissions
    ]
    queryset = serializer_class.get_default_queryset()


def _rst_req_token_expired(view, request, *args, **kwargs):
    context = {drf_settings.NON_FIELD_ERRORS_KEY: ["invalid token"]}
    status = RestStatus.HTTP_401_UNAUTHORIZED
    return {"data": context, "status": status}


class LoginAccountCreateView(APIView, UserEditViewLogMixin):
    renderer_classes = [JSONRenderer]

    def _post_token_valid(self, request, *args, rst_req=None, **kwargs):
        response_data = {}
        prof_model_cls = rst_req.email.user_type.model_class()
        profile = prof_model_cls.objects.get(id=rst_req.email.user_id)
        try:
            account = profile.account
        except type(profile).account.RelatedObjectDoesNotExist:
            account = None
        if account:
            response_data.update({"created": False, "reason": "already created before"})
        else:
            serializer_kwargs = {
                "mail_kwargs": {
                    "msg_template_path": MAIL_DATA_BASEPATH.joinpath("body/user_activated.html"),
                    "subject_template": MAIL_DATA_BASEPATH.joinpath("subject/user_activated.txt"),
                },
                "data": request.data,
                "passwd_required": True,
                "confirm_passwd": True,
                "uname_required": True,
                "account": None,
                "rst_req": rst_req,
                "many": False,
            }
            _serializer = LoginAccountSerializer(**serializer_kwargs)
            _serializer.is_valid(raise_exception=True)
            account = _serializer.save()
            self._log_action(
                action_type="create",
                request=request,
                affected_items=[account.pk],
                model_cls=type(account),
                profile_id=profile.id,
            )
            response_data["created"] = True
        return {"data": response_data, "status": None}

    post = check_auth_req_token(fn_succeed=_post_token_valid, fn_failure=_rst_req_token_expired)


class UsernameRecoveryRequestView(APIView, UserEditViewLogMixin):
    """for unauthenticated users who registered but forget their username"""

    renderer_classes = [JSONRenderer]

    def post(self, request, *args, **kwargs):
        addr = request.data.get("addr", "").strip()
        email, profile = get_profile_by_email(addr=addr, request=request)
        self._send_recovery_mail(request, profile=profile, email=email)
        # don't respond with success / failure status purposely, to avoid malicious email enumeration
        return RestResponse(data=None, status=RestStatus.HTTP_202_ACCEPTED)

    def _send_recovery_mail(self, request, profile, email):
        if not profile or not email:
            return
        account = profile.account
        # send email directly, no need to create auth user request
        # note in this application, username is not PII (Personable Identifible Information)
        # so username can be sent directly to user mailbox. TODO: how to handle it if it's PII ?
        msg_data = {
            "first_name": profile.first_name,
            "last_name": profile.last_name,
            "username": account.username,
            "request_time": datetime.now(timezone.utc),
        }
        content, subject = render_mail_content(
            msg_template_path=MAIL_DATA_BASEPATH.joinpath("body/username_recovery.html"),
            subject_template_path=MAIL_DATA_BASEPATH.joinpath("subject/username_recovery.txt"),
            msg_data=msg_data,
        )
        task_kwargs = {
            "to_addrs": [email.addr],
            "from_addr": django_settings.DEFAULT_FROM_EMAIL,
            "content": content,
            "subject": subject,
        }
        # Do not return result backend or task ID to unauthorized frontend user.
        # Log errors raising in async task
        async_send_mail.apply_async(kwargs=task_kwargs, link_error=async_default_error_handler.s())
        self._log_action(
            action_type="recover_username",
            request=request,
            affected_items=[account.pk],
            model_cls=type(account),
            profile_id=profile.id,
        )


class UnauthPasswordResetRequestView(LimitQuerySetMixin, GenericAPIView, BulkCreateModelMixin):
    """for unauthenticated users who registered but  forget their password"""

    serializer_class = UnauthRstAccountReqSerializer
    renderer_classes = [JSONRenderer]

    def post(self, request, *args, **kwargs):
        addr = request.data.get("addr", "").strip()
        email, profile = get_profile_by_email(addr=addr, request=request)
        self._send_req_mail(request=request, kwargs=kwargs, profile=profile, email=email)
        # always respond OK status even if it failed, to avoid malicious email enumeration
        return RestResponse(
            data=None, status=RestStatus.HTTP_202_ACCEPTED
        )  #  HTTP_401_UNAUTHORIZED

    def _send_req_mail(self, request, kwargs, profile, email):
        if not profile or not email:
            return
        resource_path = UserMgtCfg.api_url[UnauthPasswordResetView.__name__].split("/")
        resource_path.pop()  # last one should be <slug:token>
        serializer_kwargs = {
            "msg_template_path": MAIL_DATA_BASEPATH.joinpath("body/passwd_reset_request.html"),
            "subject_template": MAIL_DATA_BASEPATH.joinpath("subject/passwd_reset_request.txt"),
            "url_host": WEB_HOST,
            "url_resource": "/".join(resource_path),
        }
        err_args = ["url_host", serializer_kwargs["url_host"]]
        _logger.debug(None, *err_args, request=request)
        extra_kwargs = {
            "many": False,
            "return_data_after_done": False,
            "serializer_kwargs": serializer_kwargs,
            "pk_src": LimitQuerySetMixin.REQ_SRC_BODY_DATA,
        }
        kwargs.update(extra_kwargs)
        # do not use frontend request data in this view, TODO, find better way of modifying request data
        request._full_data = {"email": email.id}
        try:
            self.create(request, **kwargs)
        except Exception as e:
            fully_qualified_cls_name = "%s.%s" % (
                type(e).__module__,
                type(e).__qualname__,
            )
            log_msg = [
                "excpt_type",
                fully_qualified_cls_name,
                "excpt_msg",
                e.args[0],
                "email",
                email.addr,
                "email_id",
                email.id,
                "profile",
                profile.id,
            ]
            _logger.error(None, *log_msg, exc_info=True, request=request)


class UnauthPasswordResetView(APIView, UserEditViewLogMixin):
    renderer_classes = [JSONRenderer]

    def _patch_token_valid(self, request, rst_req=None, *args, **kwargs):
        prof_model_cls = rst_req.email.user_type.model_class()
        profile = prof_model_cls.objects.get(id=rst_req.email.user_id)
        try:
            account = profile.account
        except type(profile).account.RelatedObjectDoesNotExist:
            account = None
        if account and account.is_active:
            serializer_kwargs = {
                "mail_kwargs": {
                    "msg_template_path": MAIL_DATA_BASEPATH.joinpath(
                        "body/unauth_passwd_reset.html"
                    ),
                    "subject_template": MAIL_DATA_BASEPATH.joinpath(
                        "subject/unauth_passwd_reset.txt"
                    ),
                },
                "data": request.data,
                "passwd_required": True,
                "confirm_passwd": True,
                "account": account,
                "rst_req": rst_req,
                "many": False,
            }
            serializer = LoginAccountSerializer(**serializer_kwargs)
            serializer.is_valid(raise_exception=True)
            serializer.save()
            self._log_action(
                action_type="reset_password",
                request=request,
                affected_items=[account.pk],
                model_cls=type(account),
                profile_id=profile.id,
            )
            response_data = None
            status = None  # 200 OK
        else:
            # this should be considered as suspicious operation because an user who doesn't have login
            # account (or with inactive account) should NOT have reset token, and all requests acquired
            # by this user are deleted when deactivating the user account. so current request has to be deleted
            rst_req.delete()
            response_data = {drf_settings.NON_FIELD_ERRORS_KEY: ["reset failure"]}
            status = RestStatus.HTTP_403_FORBIDDEN
        return {"data": response_data, "status": status}

    patch = check_auth_req_token(fn_succeed=_patch_token_valid, fn_failure=_rst_req_token_expired)


class CommonAuthAccountEditMixin:
    def run(self, **kwargs):
        account = kwargs.get("account", None)
        profile_id = account.profile.id
        serializer = LoginAccountSerializer(**kwargs)
        serializer.is_valid(raise_exception=True)
        serializer.save()
        self._log_action(
            action_type=self.log_action_type,
            request=self.request,
            affected_items=[account.pk],
            model_cls=type(account),
            profile_id=profile_id,
        )
        return RestResponse(data={}, status=None)


class AuthUsernameEditAPIView(AuthCommonAPIView, CommonAuthAccountEditMixin):
    """for authenticated user who attempts to update username of their account"""

    log_action_type = "update_username"

    def patch(self, request, *args, **kwargs):
        serializer_kwargs = {
            "data": request.data,
            "uname_required": True,
            "old_uname_required": True,
            "account": request.user,
            "rst_req": None,
            "many": False,
        }
        return self.run(**serializer_kwargs)


class AuthPasswdEditAPIView(AuthCommonAPIView, CommonAuthAccountEditMixin):
    """for authenticated user who attempts to update password of their account"""

    log_action_type = "update_password"

    def patch(self, request, *args, **kwargs):
        serializer_kwargs = {
            "data": request.data,
            "passwd_required": True,
            "confirm_passwd": True,
            "old_passwd_required": True,
            "account": request.user,
            "rst_req": None,
            "many": False,
        }
        return self.run(**serializer_kwargs)


class LoginView(APIView):
    renderer_classes = [JSONRenderer]

    @csrf_protect_m
    def post(self, request, *args, **kwargs):
        username = request.data.get("username", "")
        password = request.data.get("password", "")
        account = authenticate(request, username=username, password=password)
        log_msg = [
            "action",
            "login",
            "result",
            account is not None,
            "username",
            username or "__EMPTY__",
        ]
        if account and account.is_authenticated:
            profile = account.profile
            jwt = jwt_based_login(request, user=account)
            status = RestStatus.HTTP_200_OK
            context = {}
            log_msg += ["profile_id", profile.id]
        else:
            jwt = None
            status = RestStatus.HTTP_401_UNAUTHORIZED
            context = {
                drf_settings.NON_FIELD_ERRORS_KEY: ["authentication failure"],
            }
        response = RestResponse(data=context, status=status)
        self._set_refresh_token_to_cookie(response, jwt=jwt)
        _logger.info(None, *log_msg, request=request)
        return response

    def _set_refresh_token_to_cookie(self, response, jwt):
        if not jwt:
            return
        jwt_name_refresh_token = getattr(django_settings, "JWT_NAME_REFRESH_TOKEN", None)
        err_msg = "all of the parameters have to be set when applying JWTbaseMiddleware , but some of them are unconfigured, JWT_NAME_REFRESH_TOKEN = %s"
        assert jwt_name_refresh_token, err_msg % (jwt_name_refresh_token)
        _keystore = create_keystore_helper(
            cfg=django_settings.AUTH_KEYSTORE, import_fn=import_string
        )
        encoded = jwt.encode(keystore=_keystore)
        max_age_td = jwt.payload["exp"] - jwt.payload["iat"]
        max_age = max_age_td.seconds
        response.set_cookie(
            key=jwt_name_refresh_token,
            value=encoded,
            max_age=max_age,
            domain=None,
            expires=http_date(jwt.payload["exp"].timestamp()),
            path=django_settings.SESSION_COOKIE_PATH,
            secure=django_settings.SESSION_COOKIE_SECURE or None,
            samesite=django_settings.SESSION_COOKIE_SAMESITE,
            httponly=True,
        )


## end of class LoginView


class LogoutView(APIView):
    renderer_classes = [JSONRenderer]
    # anonymous users are NOT allowed to consume this endpoint
    authentication_classes = [RefreshJWTauthentication]
    permission_classes = [IsAuthenticated, IsStaffUser]

    @csrf_protect_m
    def post(self, request, *args, **kwargs):
        account = request.user
        profile = account.profile
        username = account.username
        log_msg = ["action", "logout", "username", username, "profile_id", profile.id]
        _logger.info(None, *log_msg, request=request)
        response = RestResponse(data=None, status=RestStatus.HTTP_200_OK)
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
        return response


class RefreshAccessTokenView(APIView):
    """
    API endpoint for client with valid refresh token to request a new access token
    The token generated at this endpoint is used in any other specific service
    (could also be in different network domain)
    """

    authentication_classes = [RefreshJWTauthentication]
    permission_classes = [IsAuthenticated, IsStaffUser]
    # TODO, should this authentication server provide different endpoint which
    # forces frontend client to give username / password to get a token in return ?

    def get(self, request, *args, **kwargs):
        audience = request.query_params.get("audience", "").split(",")
        audience = self._filter_resource_services(
            audience,
            exclude=["web"],
        )
        if audience:
            signed = self._gen_signed_token(request=request, audience=audience)
            app_label = UserMgtCfg.name
            # To be compilant with OAuth2 specification, the toekn response should
            # at least contain `access_token` and `token_type` field in JSON form
            data = {
                "access_token": signed,
                "token_type": "bearer",
                "jwks_url": "%s/jwks" % (cors_cfg.ALLOWED_ORIGIN[app_label]),
            }
            status = RestStatus.HTTP_200_OK
        else:
            data = {drf_settings.NON_FIELD_ERRORS_KEY: ["invalid audience field"]}
            status = RestStatus.HTTP_400_BAD_REQUEST
        return RestResponse(data=data, status=status)

    def _filter_resource_services(self, audience, exclude=None):
        exclude = exclude or []
        allowed = cors_cfg.ALLOWED_ORIGIN.keys()
        allowed = set(allowed) - set(exclude)
        filtered = filter(lambda a: a in allowed, audience)  # avoid untrusted inputs
        return list(filtered)

    def _serialize_auth_info(self, audience, profile):
        out = {
            "id": profile.id,
            "priv_status": profile.privilege_status,
            "perms": None,
            "quota": None,
        }
        # --- fetch low-level permissions relevant to the audience ---
        out["perms"] = serialize_profile_permissions(profile, app_labels=audience)
        if not any(out["perms"]) and out["priv_status"] != type(profile).SUPERUSER:
            errmsg = (
                "the user does not have access to these resource services listed in audience field"
            )
            err_detail = {
                drf_settings.NON_FIELD_ERRORS_KEY: [errmsg],
            }
            raise PermissionDenied(detail=err_detail)  ##  SuspiciousOperation
        # --- fetch quota ---
        out["quota"] = serialize_profile_quota(profile, app_labels=audience)
        return out

    def _gen_signed_token(self, request, audience):
        account = request.user
        profile = account.profile
        profile_serial = self._serialize_auth_info(audience=audience, profile=profile)
        keystore = create_keystore_helper(
            cfg=django_settings.AUTH_KEYSTORE, import_fn=import_string
        )
        now_time = datetime.utcnow()
        expiry = now_time + timedelta(seconds=django_settings.JWT_ACCESS_TOKEN_VALID_PERIOD)
        token = JWT()
        issuer_url = "%s/%s" % (
            cors_cfg.ALLOWED_ORIGIN[UserMgtCfg.name],
            UserMgtCfg.api_url[LoginView.__name__],
        )
        payload = {
            "profile": profile_serial.pop("id"),
            "aud": audience,
            "iat": now_time,
            "exp": expiry,
            "iss": issuer_url,
        }
        payload.update(profile_serial)  # roles, quota
        token.payload.update(payload)
        return token.encode(keystore=keystore)


## end of class RefreshAccessTokenView


class JWKSPublicKeyView(APIView):
    def get(self, request, *args, **kwargs):
        try:
            filepath = django_settings.AUTH_KEYSTORE["persist_pubkey_handler"]["init_kwargs"][
                "filepath"
            ]
            status = RestStatus.HTTP_200_OK
        except (AttributeError, KeyError) as e:
            status = RestStatus.HTTP_404_NOT_FOUND
            filepath = ""
            log_args = ["msg", e]
            _logger.warning(None, *log_args, request=request)
        return DjangoStreamingHttpResponse(
            streaming_content=stream_jwks_file(filepath=filepath),
            content_type="application/json",
            status=status,
        )
