import logging

from django.contrib.auth        import authenticate, login
from django.contrib.auth.mixins import LoginRequiredMixin
from rest_framework.settings    import api_settings as drf_settings
from rest_framework.views       import APIView
from rest_framework.generics    import GenericAPIView
from rest_framework.permissions import IsAuthenticated
from rest_framework.renderers   import TemplateHTMLRenderer, JSONRenderer
from rest_framework.response    import Response as RestResponse
from rest_framework             import status as RestStatus
from celery.result import AsyncResult

from common.auth.backends import ExtendedSessionAuthentication, IsStaffUser
from common.models.db     import db_conn_retry_wrapper

from .mixins  import LimitQuerySetMixin, ExtendedListModelMixin, ExtendedRetrieveModelMixin
from .mixins  import BulkCreateModelMixin, BulkUpdateModelMixin, BulkDestroyModelMixin

_logger = logging.getLogger(__name__)


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


# from django.contrib.auth.decorators import login_required, permission_required
# django view decorator cannot be used in REST framework view
# @permission_required(perm=('auth.add_group',), login_url=CUSTOM_LOGIN_URL,)
# @login_required(login_url=CUSTOM_LOGIN_URL, redirect_field_name='redirect')

class BaseAuthHTMLView(LoginRequiredMixin,GenericAPIView):
    """ base view for authorized users to perform CRUD operation  """
    renderer_classes = [TemplateHTMLRenderer]
    redirect_field_name = 'redirect'
    login_url = None # subclasses have to override this attribute


class BaseCommonAPIView(CommonAPIReadView, BulkCreateModelMixin, BulkUpdateModelMixin, BulkDestroyModelMixin):
    pass

class AuthCommonAPIView(BaseCommonAPIView):
    """ base view for authorized users to perform CRUD operation  """
    authentication_classes = [ExtendedSessionAuthentication,]
    permission_classes = [IsAuthenticated, IsStaffUser] # api_settings.DEFAULT_PERMISSION_CLASSES

class AuthCommonAPIReadView(CommonAPIReadView):
    """  base view for authorized users to retrieve data through API  """
    authentication_classes = [ExtendedSessionAuthentication,]
    permission_classes = [IsAuthenticated, IsStaffUser]


class BaseLoginView(APIView):
    renderer_classes = [TemplateHTMLRenderer, JSONRenderer]
    template_name = None
    submit_url    = None
    default_url_redirect = None
    is_staff_only = False

    def __new__(cls, *args, **kwargs):
        assert cls.template_name and cls.submit_url and cls.default_url_redirect, \
            "class variables not properly configured"
        return super().__new__(cls, *args, **kwargs)

    def get(self, request, *args, **kwargs):
        redirect = request.query_params.get("redirect", self.default_url_redirect)
        formparams = {'non_field_errors':drf_settings.NON_FIELD_ERRORS_KEY, 'submit_url': self.submit_url,
                'redirect' : redirect, }
        context = {'formparams': formparams, }
        return RestResponse(data=context, template_name=self.template_name)

    # TODO, apply CSRF protection
    def post(self, request, *args, **kwargs):
        username = request.data.get('username','')
        password = request.data.get('password','')
        user = authenticate(request, username=username, password=password, is_staff_only=self.is_staff_only)
        log_msg = ['action', 'login', 'result', user is not None, 'username', username or '__EMPTY__']
        if user:
            login(request, user, backend=None)
            status = RestStatus.HTTP_200_OK
            context = {}
            profile_id = self.get_profile_id(request=request) # request.user is automatically updated by login()
            log_msg += ['profile_id', profile_id]
        else:
            status = RestStatus.HTTP_401_UNAUTHORIZED
            context = {drf_settings.NON_FIELD_ERRORS_KEY: ['authentication failure'], }
        _logger.info(None, *log_msg, request=request)
        return RestResponse(data=context, status=status)


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


