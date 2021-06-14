import logging

from django.contrib.auth        import authenticate, get_user_model
from rest_framework.settings    import api_settings as drf_settings
from rest_framework.views       import APIView , exception_handler as drf_exception_handler
from rest_framework.generics    import GenericAPIView
from rest_framework.renderers   import JSONRenderer
from rest_framework.response    import Response as RestResponse
from rest_framework.permissions import IsAuthenticated
from rest_framework             import status as RestStatus
from celery.result import AsyncResult

from common.csrf.middleware import  csrf_protect_m
from common.auth.django.login import logout, login
from common.auth.backends import ExtendedSessionAuthentication, ForwardClientAuthentication, IsStaffUser
from common.models.db     import db_conn_retry_wrapper, get_db_error_response
from common.views.proxy.mixins import RemoteGetProfileIDMixin
from .mixins  import LimitQuerySetMixin, ExtendedListModelMixin, ExtendedRetrieveModelMixin
from .mixins  import BulkCreateModelMixin, BulkUpdateModelMixin, BulkDestroyModelMixin

_logger = logging.getLogger(__name__)
auth_user_cls = get_user_model()


class CommonAPIReadView(LimitQuerySetMixin, GenericAPIView, ExtendedListModelMixin, ExtendedRetrieveModelMixin):
    renderer_classes = [JSONRenderer]
    max_page_size = 11
    max_retry_db_conn = 5
    wait_intvl_sec = 0.02

    @property
    def paginator(self):
        if hasattr(self, '_paginator'):
            return self._paginator
        obj = super().paginator
        page_sz   = self.request.query_params.get('page_size', '')
        page_sz   = int(page_sz)   if page_sz.isdigit()   else -1
        # REST framework paginator can handle it if page_sz > max_page_size
        if page_sz > 0:
            obj.page_size = page_sz
            obj.page_size_query_param = 'page_size'
            obj.max_page_size = self.max_page_size
        else:
            obj.page_size = None
            obj.page_size_query_param = None
            obj.max_page_size = None
        return obj

    @db_conn_retry_wrapper
    def paginate_queryset(self, queryset):
        return  super().paginate_queryset(queryset=queryset)

    def get(self, request, *args, **kwargs):
        pk = kwargs.get('pk', None)
        if pk:
            return self.retrieve(request, *args, **kwargs)
        else:
            return self.list(request, *args, **kwargs)


def _perform_authentication(view, request):
    # override original function because account is required to make 
    account = request.user
    if isinstance(account, auth_user_cls):
        # Note that the module path of all Django views start with
        # applicatoin label
        app_label = view.__module__.split('.')[0]
        kwargs = {'account':account, 'services_label':[app_label]}
        view.get_profile(**kwargs)
    #if not hasattr(request, '_unfinished_rpc_replies'):
    #    setattr(request, '_unfinished_rpc_replies', [])
    #request._unfinished_rpc_replies.append(evt)


class BaseCommonAPIView(CommonAPIReadView, BulkCreateModelMixin, BulkUpdateModelMixin, BulkDestroyModelMixin):
    pass

class AuthCommonAPIView(BaseCommonAPIView):
    """ base view for authorized users to perform CRUD operation  """
    authentication_classes = [ExtendedSessionAuthentication, ForwardClientAuthentication]
    permission_classes = [IsAuthenticated, IsStaffUser] # api_settings.DEFAULT_PERMISSION_CLASSES
    def perform_authentication(self, request):
        _perform_authentication(view=self, request=request)


class AuthCommonAPIReadView(CommonAPIReadView):
    """  base view for authorized users to retrieve data through API  """
    authentication_classes = [ExtendedSessionAuthentication, ForwardClientAuthentication]
    permission_classes = [IsAuthenticated, IsStaffUser]
    def perform_authentication(self, request):
        _perform_authentication(view=self, request=request)


class BaseLoginView(APIView, RemoteGetProfileIDMixin):
    renderer_classes = [JSONRenderer]
    is_staff_only = False
    use_session   = False
    use_token     = False

    @csrf_protect_m
    def post(self, request, *args, **kwargs):
        username = request.data.get('username','')
        password = request.data.get('password','')
        user = authenticate(request, username=username, password=password, is_staff_only=self.is_staff_only)
        log_msg = ['action', 'login', 'result', user is not None, 'username', username or '__EMPTY__']
        if user:
            reply_evt = self.get_profile(account=user) # make async RPC to usermgt service
            login(request, user, backend=None, use_session=self.use_session, use_token=self.use_token)
            status = RestStatus.HTTP_200_OK
            context = {}
            profile_id = self.get_profile_id(request=request, num_of_msgs_fetch=2)
            log_msg += ['profile_id', profile_id]
        else:
            status = RestStatus.HTTP_401_UNAUTHORIZED
            context = {drf_settings.NON_FIELD_ERRORS_KEY: ['authentication failure'], }
        _logger.info(None, *log_msg, request=request)
        return RestResponse(data=context, status=status)


class BaseLogoutView(APIView, RemoteGetProfileIDMixin):
    renderer_classes = [JSONRenderer]
    use_session   = False
    use_token     = False

    def post(self, request, *args, **kwargs):
        # anonymous users are NOT allowed to consume this endpoint
        status = RestStatus.HTTP_401_UNAUTHORIZED
        account = request.user
        if isinstance(account, auth_user_cls) :
            self.get_profile(account=account)
            logout(request, use_token=self.use_token, use_session=self.use_session)
            status = RestStatus.HTTP_200_OK
            username = account.username
            profile_id = self.get_profile_id(request=request, num_of_msgs_fetch=2)
            log_msg = ['action', 'logout', 'username', username, 'profile_id', profile_id]
            _logger.info(None, *log_msg, request=request)
        return  RestResponse(data={}, status=status)



# TODO, figure out appropriate use case for this view
class AsyncTaskResultView(GenericAPIView):
    renderer_classes = [JSONRenderer]

    def get(self, request, *args, **kwargs):
        # print("kwargs  : "+ str(kwargs))
        r = AsyncResult(kwargs.pop('id', None))
        status = None
        headers = {}
        s_data = {'status': r.status, 'result': r.result or '' }
        return RestResponse(s_data, status=status, headers=headers)


def exception_handler(exc, context):
    """
    extend default handler to process low-level database / model related exceptions
    """
    response = drf_exception_handler(exc, context)
    if response is None:
        headers = {}
        status = get_db_error_response(e=exc, headers=headers, raise_if_not_handled=False)
        if status and status != RestStatus.HTTP_500_INTERNAL_SERVER_ERROR:
            response = RestResponse(data={}, status=status, headers=headers)
    _logger.error("%s", exc, request=context['request'])
    print('RESTful exception handling : %s' % exc)
    return response



