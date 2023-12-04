import logging

from rest_framework.settings    import api_settings as drf_settings
from rest_framework.views       import APIView , exception_handler as drf_exception_handler
from rest_framework.generics    import GenericAPIView
from rest_framework.renderers   import JSONRenderer
from rest_framework.response    import Response as RestResponse
from rest_framework.permissions import IsAuthenticated
from rest_framework             import status as RestStatus
from celery.result import AsyncResult

from common.auth.django.authentication import AccessJWTauthentication, IsStaffUser
from common.models.db     import db_conn_retry_wrapper, get_db_error_response
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

    def get(self, request, *args, pk=None, **kwargs):
        if pk:
            return self.retrieve(request, *args, **kwargs)
        else:
            return self.list(request, *args, **kwargs)



class BaseCommonAPIView(CommonAPIReadView, BulkCreateModelMixin, BulkUpdateModelMixin, BulkDestroyModelMixin):
    pass

class AuthCommonAPIView(BaseCommonAPIView):
    """ base view for authorized users to perform CRUD operation  """
    authentication_classes = [AccessJWTauthentication]
    permission_classes = [IsAuthenticated, IsStaffUser] # api_settings.DEFAULT_PERMISSION_CLASSES


class AuthCommonAPIReadView(CommonAPIReadView):
    """  base view for authorized users to retrieve data through API  """
    authentication_classes = [AccessJWTauthentication]
    permission_classes = [IsAuthenticated, IsStaffUser]



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
    return response



