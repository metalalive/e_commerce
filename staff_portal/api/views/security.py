import copy
import operator
import logging

from django.conf   import  settings as django_settings
from django.core.cache          import caches as DjangoBuiltinCaches
from rest_framework             import status as RestStatus
from rest_framework.response    import Response as RestResponse

from common.views.api      import  AuthCommonAPIView, AuthCommonAPIReadView
from common.views.filters  import  DateTimeRangeFilter
from common.auth.backends  import  IsSuperUser
from ..queryset import UserActionSet

class GetProfileIDMixin:
    pass

class UserActionHistoryAPIReadView(AuthCommonAPIReadView):
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


