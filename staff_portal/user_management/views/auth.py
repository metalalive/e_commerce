import copy
import logging
from datetime import datetime, timezone, timedelta

from django.conf   import  settings as django_settings
from django.core.exceptions     import ValidationError
from django.utils.http          import http_date
from django.utils.module_loading import import_string
from django.contrib.contenttypes.models  import ContentType
from django.contrib.auth  import authenticate

from rest_framework             import status as RestStatus
from rest_framework.generics    import GenericAPIView
from rest_framework.views       import APIView
from rest_framework.renderers   import JSONRenderer
from rest_framework.response    import Response as RestResponse
from rest_framework.permissions import IsAuthenticated
from rest_framework.exceptions  import PermissionDenied
from rest_framework.settings    import api_settings as drf_settings

from common.auth.jwt      import JWT
from common.auth.keystore import create_keystore_helper
from common.auth.django.login import  jwt_based_login
from common.auth.django.authentication import RefreshJWTauthentication, IsStaffUser
from common.cors import config as cors_cfg
from common.csrf.middleware    import  csrf_protect_m
from common.views.mixins   import  LimitQuerySetMixin, UserEditViewLogMixin, BulkUpdateModelMixin
from common.views.api      import  AuthCommonAPIView, AuthCommonAPIReadView

from ..apps        import UserManagementConfig
from ..models.common import AppCodeOptions
from ..models.base import QuotaMaterial
from ..serializers import PermissionSerializer
from ..serializers.auth import UnauthRstAccountReqSerializer
from ..permissions import ModelLvlPermsPermissions
from .common    import check_auth_req_token, AuthTokenCheckMixin, get_profile_account_by_email

_logger = logging.getLogger(__name__)


class PermissionView(AuthCommonAPIReadView):
    """
    this API class should provide a set of pre-defined roles, which include a set of permissions
    granted to the role, in order for users in staff site to easily apply those permissions to application
    , instead of simply passing all django-defined permissions to mess user interface
    """
    serializer_class = PermissionSerializer
    permission_classes = copy.copy(AuthCommonAPIReadView.permission_classes) + [ModelLvlPermsPermissions]
    queryset = serializer_class.get_default_queryset()



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


class UnauthPasswordResetRequestView(LimitQuerySetMixin, GenericAPIView, BulkUpdateModelMixin):
    """ for unauthenticated users who registered but  forget their password """
    serializer_class = UnauthRstAccountReqSerializer
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



class AuthUsernameEditAPIView(AuthCommonAPIView, CommonAuthAccountEditMixin):
    """ for authenticated user who attempts to update username of their account """
    log_action_type = "update_username"

    def patch(self, request, *args, **kwargs):
        serializer_kwargs = {
            'data': request.data, 'uname_required':True,  'old_uname_required': True,
            'account': request.user, 'auth_req': None,  'many': False,
        }
        return self.run(**serializer_kwargs)


class AuthPasswdEditAPIView(AuthCommonAPIView, CommonAuthAccountEditMixin):
    """ for authenticated user who attempts to update password of their account """
    log_action_type = "update_password"

    def patch(self, request, *args, **kwargs):
        serializer_kwargs = {
            'data': request.data, 'passwd_required':True,  'confirm_passwd': True, 'old_passwd_required': True,
            'account': request.user, 'auth_req': None,  'many': False,
        }
        return self.run(**serializer_kwargs)



class LoginView(APIView):
    renderer_classes = [JSONRenderer]

    @csrf_protect_m
    def post(self, request, *args, **kwargs):
        username = request.data.get('username','')
        password = request.data.get('password','')
        account = authenticate(request, username=username, password=password)
        log_msg = ['action', 'login', 'result', account is not None, 'username', username or '__EMPTY__']
        if account and account.is_authenticated:
            profile = account.profile
            jwt = jwt_based_login(request, user=account)
            status = RestStatus.HTTP_200_OK
            context = {}
            log_msg += ['profile_id', profile.id]
        else:
            jwt = None
            status = RestStatus.HTTP_401_UNAUTHORIZED
            context = {drf_settings.NON_FIELD_ERRORS_KEY: ['authentication failure'], }
        response = RestResponse(data=context, status=status)
        self._set_refresh_token_to_cookie(response, jwt=jwt)
        _logger.info(None, *log_msg, request=request)
        return response

    def _set_refresh_token_to_cookie(self, response, jwt):
        if not jwt:
            return
        jwt_name_refresh_token = getattr(django_settings, 'JWT_NAME_REFRESH_TOKEN', None)
        err_msg = 'all of the parameters have to be set when applying JWTbaseMiddleware , but some of them are unconfigured, JWT_NAME_REFRESH_TOKEN = %s'
        assert jwt_name_refresh_token, err_msg % (jwt_name_refresh_token)
        _keystore = create_keystore_helper(cfg=django_settings.AUTH_KEYSTORE, import_fn=import_string)
        encoded = jwt.encode(keystore=_keystore)
        max_age_td = jwt.payload['exp'] - jwt.payload['iat']
        max_age = max_age_td.seconds
        response.set_cookie(
            key=jwt_name_refresh_token, value=encoded,  max_age=max_age,  domain=None,
            expires=http_date(jwt.payload['exp'].timestamp()),
            path=django_settings.SESSION_COOKIE_PATH,
            secure=django_settings.SESSION_COOKIE_SECURE or None,
            samesite=django_settings.SESSION_COOKIE_SAMESITE,
            httponly=True
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
        log_msg = ['action', 'logout', 'username', username, 'profile_id', profile.id]
        _logger.info(None, *log_msg, request=request)
        response = RestResponse(data=None, status=RestStatus.HTTP_200_OK)
        jwt_name_refresh_token = django_settings.JWT_NAME_REFRESH_TOKEN
        response.set_cookie(
            key=jwt_name_refresh_token, value='',  max_age=0, domain=None,
            path=django_settings.SESSION_COOKIE_PATH,
            secure=django_settings.SESSION_COOKIE_SECURE or None,
            samesite=django_settings.SESSION_COOKIE_SAMESITE,
            httponly=True
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
        audience = request.query_params.get('audience', '').split(',')
        audience = self._filter_resource_services(audience, exclude=['web',])
        if audience:
            signed = self._gen_signed_token(request=request, audience=audience)
            app_label = UserManagementConfig.name
            # To be compilant with OAuth2 specification, the toekn response should
            # at least contain `access_token` and `token_type` field in JSON form
            data = {'access_token': signed, 'token_type': 'bearer',
                    'jwks_url':'%s/jwks' % (cors_cfg.ALLOWED_ORIGIN[app_label]),}
            status = RestStatus.HTTP_200_OK
        else:
            data = {drf_settings.NON_FIELD_ERRORS_KEY: ['invalid audience field']}
            status = RestStatus.HTTP_400_BAD_REQUEST
        return RestResponse(data=data, status=status)

    def _filter_resource_services(self, audience, exclude=None):
        exclude = exclude or []
        allowed = cors_cfg.ALLOWED_ORIGIN.keys()
        allowed = set(allowed) - set(exclude)
        filtered = filter(lambda a: a in allowed, audience) # avoid untrusted inputs
        return list(filtered)

    def _serialize_auth_info(self, audience, profile):
        role_types = ('direct', 'inherit')
        out = {'id':profile.id , 'priv_status':profile.privilege_status, 'perms': [], 'quota':[] }
        # --- fetch low-level permissions relevant to the audience ---
        all_roles = profile.all_roles
        for role_type in role_types:
            perm_qset = all_roles[role_type].get_permissions(app_labels=audience)
            vals = perm_qset.values_list('content_type__app_label', 'codename')
            vals = filter(lambda d: getattr(AppCodeOptions, d[0], None), vals)
            vals = map(lambda d: {'app_code':getattr(AppCodeOptions, d[0]).value, 'codename':d[1]}, vals)
            out['perms'].extend(vals)
        if not any(out['perms']) and out['priv_status'] != type(profile).SUPERUSER:
            errmsg = "the user does not have access to these resource services listed in audience field"
            err_detail = {drf_settings.NON_FIELD_ERRORS_KEY: [errmsg],}
            raise PermissionDenied(detail=err_detail) ##  SuspiciousOperation
        # --- fetch quota ---
        mat_qset = QuotaMaterial.get_for_apps(app_labels=audience)
        mat_qset = mat_qset.values('id', 'app_code', 'mat_code')
        quota_mat_map = dict(map(lambda d: (d['id'], d), mat_qset))
        fetch_mat_ids = quota_mat_map.keys()
        all_quota = profile.all_quota
        filtered_quota = filter(lambda kv: kv[0] in fetch_mat_ids, all_quota.items())
        filtered_quota = map(lambda kv: {'app_code': quota_mat_map[kv[0]]['app_code'], \
            'mat_code': quota_mat_map[kv[0]]['mat_code'], 'maxnum': kv[1]} , filtered_quota)
        filtered_quota = list(filtered_quota)
        out['quota'].extend(filtered_quota)
        return out

    def _gen_signed_token(self, request, audience):
        account = request.user
        profile = account.profile
        profile_serial = self._serialize_auth_info(audience=audience, profile=profile)
        keystore = create_keystore_helper(cfg=django_settings.AUTH_KEYSTORE, import_fn=import_string)
        now_time = datetime.utcnow()
        expiry = now_time + timedelta(seconds=django_settings.JWT_ACCESS_TOKEN_VALID_PERIOD)
        token = JWT()
        payload = {
            'profile' :profile_serial.pop('id'),
            'aud':audience,  'iat':now_time,  'exp':expiry,
        }
        payload.update(profile_serial) # roles, quota
        token.payload.update(payload)
        return token.encode(keystore=keystore)

