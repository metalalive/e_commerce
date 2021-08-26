import logging

from django.core.validators  import EMPTY_VALUES
from django.core.exceptions  import ObjectDoesNotExist, FieldError
from django.db.utils         import OperationalError
from django.db.models.constants import LOOKUP_SEP
from rest_framework.response    import Response as RestResponse
from rest_framework             import status as RestStatus
from rest_framework.settings    import api_settings
from rest_framework.mixins      import CreateModelMixin, UpdateModelMixin, DestroyModelMixin

from common.models.db  import db_conn_retry_wrapper

_logger = logging.getLogger(__name__)


class LimitQuerySetMixin:
    # the constants below specify where to get list of IDs from client request
    REQ_SRC_QUERY_PARAMS = 0
    REQ_SRC_BODY_DATA = 1

    def get_IDs(self, pk_param_name='pks', pk_field_name='pk', delimiter=',',
            pk_src=REQ_SRC_QUERY_PARAMS, pk_skip_list=None):
        IDs = None
        if pk_src == self.REQ_SRC_QUERY_PARAMS:
            IDs = self.request.query_params.get(pk_param_name, None)
            if IDs:
                IDs = IDs.split(delimiter)
                IDs = [i for i in IDs if not i in EMPTY_VALUES]
        elif pk_src == self.REQ_SRC_BODY_DATA:
            IDs = [x[pk_field_name] for x in self.request.data if x.get(pk_field_name,None)]
            err_args = ["req_body_id", IDs]
            _logger.debug(None, *err_args, request=self.request)
        else:
            IDs = None

        if IDs and any(IDs) and pk_skip_list:
            IDs = list(set(IDs) - set(pk_skip_list))
            err_args = ["remove_list", pk_skip_list, "filtered_ids", IDs,]
            _logger.debug(None, *err_args, request=self.request)
        return IDs


    def get_queryset(self, pk_param_name='pks', pk_field_name='pk', delimiter=',',
            pk_src=REQ_SRC_QUERY_PARAMS, pk_skip_list=None, fetch_all=False):
        """
        filter queryset  at the beginning, from following source:
        * a list of IDs in URL query string.
        * a list of IDs in request body (usually sent in POST/PUT/PATCH/DELETE HTTP method)
        Or return empty queryset if not specifying list of IDs
        """
        manager = self.serializer_class.Meta.model.objects
        if fetch_all:
            queryset = super().get_queryset()
        else:
            IDs = self.get_IDs(pk_param_name=pk_param_name, pk_field_name=pk_field_name, delimiter=delimiter,
                    pk_src=pk_src, pk_skip_list=pk_skip_list)
            if IDs :
                try:
                    pk_contain =  LOOKUP_SEP.join([pk_field_name, 'in'])
                    kwargs = {pk_contain: IDs}
                    queryset = manager.filter(**kwargs)
                except (FieldError, ValueError) as e: # invalid data type in ID list, or invalid field name
                    queryset = manager.none()
                    queryset._error_msgs = e.args
                    fully_qualified_cls_name = '%s.%s' % (type(e).__module__, type(e).__qualname__)
                    err_args = ["field", pk_field_name, "value", IDs, "excpt_msg", e, "excpt_type", fully_qualified_cls_name]
                    _logger.warning(None, *err_args, request=self.request)
            else:
                queryset = manager.none()
            ## self.queryset = queryset
        return queryset


    def get_object(self, pk_field_name='pk', skip_if_none=False):
        # Perform the lookup filtering.
        self.lookup_field = pk_field_name
        lookup_url_kwarg = self.lookup_url_kwarg or self.lookup_field
        assert lookup_url_kwarg in self.kwargs, (
            'Expected view %s to be called with a URL keyword argument '
            'named "%s". Fix your URL conf, or set the `.lookup_field` '
            'attribute on the view correctly.' %
            (self.__class__.__name__, lookup_url_kwarg)
        )
        filter_kwargs = {self.lookup_field: self.kwargs[lookup_url_kwarg]}
        try:
            obj = self.serializer_class.Meta.model.objects.get(**filter_kwargs)
            # May raise a permission denied
            self.check_object_permissions(self.request, obj)
        except (FieldError, ValueError, ObjectDoesNotExist) as e:
            fully_qualified_cls_name = '%s.%s' % (type(e).__module__, type(e).__qualname__)
            err_args = ["field", self.lookup_field, "value", self.kwargs[lookup_url_kwarg], "excpt_msg", e,
                    "excpt_type", fully_qualified_cls_name]
            _logger.warning(None, *err_args, request=self.request)
            if not skip_if_none:
                raise
            obj = None
        return obj

# end of LimitQuerySetMixin


@db_conn_retry_wrapper
def _get_serializer_data(view, serializer):
    s_data = serializer.data
    return s_data


class ExtendedListModelMixin:
    # this mixin class has to work with any subclass of 
    # common.views.proxy.mixins.BaseGetProfileIDMixin
    def list(self, request, *args, **kwargs):
        s_data = {}
        page = None
        fetch_all = self.request.query_params.get('ids', None) is None
        pk_skip_list = kwargs.pop('pk_skip_list', None)
        queryset = self.get_queryset(pk_param_name='ids', pk_field_name='id', \
                fetch_all=fetch_all, pk_skip_list=pk_skip_list)
        queryset = self.filter_queryset(queryset)
        page = self.paginate_queryset(queryset=queryset)
        serializer_kwargs = {'many': True, 'account': request.user,}
        serializer_kwargs.update(kwargs.pop('serializer_kwargs', {}))
        instances = page or queryset
        serializer = self.get_serializer(instances, **serializer_kwargs)
        s_data = _get_serializer_data(self, serializer)
        if page: # following part is only for logging user activity
            _id_list = [str(obj.pk) for obj in instances]
            _id_list = ",".join(_id_list)
        else:
            _id_list = "FETCH_ALL"
        model_cls_hier = "%s.%s" % (queryset.model.__module__ , queryset.model.__name__)
        log_msg = ['action', 'view_list', 'profile_id', self.get_profile_id(request=request),
                'model_cls', model_cls_hier, 'IDs', _id_list,]
        _logger.debug(None, *log_msg, request=request, stacklevel=1)
        if page:
            resp = self.get_paginated_response(s_data)
        else:
            resp = RestResponse(data=s_data)
        return resp


class ExtendedRetrieveModelMixin:
    # this mixin class has to work with any subclass of 
    # common.views.proxy.mixins.BaseGetProfileIDMixin
    def retrieve(self, request, *args, **kwargs):
        status = None
        s_data = {}
        try:
            instance = self.get_object()
            serializer_kwargs = {'many': False, 'account': request.user,}
            serializer_kwargs.update(kwargs.pop('serializer_kwargs', {}))
            serializer = self.get_serializer(instance, **serializer_kwargs)
            s_data = _get_serializer_data(self, serializer)
            model_cls_hier = "%s.%s" % (type(instance).__module__ , type(instance).__name__)
            log_msg = ['action', 'view_item', 'profile_id', self.get_profile_id(request=request),
                    'model_cls', model_cls_hier, 'ID', instance.pk,]
            _logger.debug(None, *log_msg, request=request)
        except ObjectDoesNotExist as e:
            status = RestStatus.HTTP_404_NOT_FOUND
        return RestResponse(s_data, status=status)


class UserEditViewLogMixin:
    # this mixin class has to work with any subclass of 
    # common.views.proxy.mixins.BaseGetProfileIDMixin
    def log_action(self, action_type, request, many, serializer):
        ## self.get_serializer_class()
        if many: # instance should be either QuerySet or Model
            item_labels = map(lambda obj: obj.minimum_info , serializer.instance)
            item_labels = list(item_labels)
            model_cls = type(serializer.child).Meta.model
        else:
            item_labels = [serializer.instance.minimum_info]
            model_cls = type(serializer).Meta.model
        profile_id = self.get_profile_id(request=request)
        self._log_action(action_type=action_type, request=request, affected_items=item_labels,
                model_cls=model_cls, profile_id=profile_id, stacklevel=3)

    def _log_action(self, action_type, request, affected_items, model_cls, profile_id, extra_kwargs=None,
            stacklevel=2, loglevel=logging.INFO):
        """
        request : HTTP request
        action_type : could be any verb e.g. create, update, delete, deactivate_account, recover_username ...etc
        affected_items : list of field values that represent affected model instances due to the action
        model_cls : corresponding model classes due to this action
        profile_id: the user profile who performed this action
        stacklevel: indicate to grab caller's information from which stack frame, 1 means this function,
                    2 means caller function, 3 means the function that invoked caller function ... etc
        """
        model_cls_hier = "%s.%s" % (model_cls.__module__ , model_cls.__qualname__)
        log_msg = ['action', action_type, 'profile_id', profile_id, 'model_cls', model_cls_hier,
                'affected_items', affected_items]
        if extra_kwargs:
            log_msg.extend([i for k,v in extra_kwargs.items() for i in (k,v)])
        _logger.log(loglevel, None, *log_msg, request=request, stacklevel=stacklevel)


class BulkCreateModelMixin(CreateModelMixin, UserEditViewLogMixin):
    """ override create() method to add more arguments """
    def create(self, request, *args, **kwargs):
        many = kwargs.pop('many', False)
        return_data_after_done = kwargs.pop('return_data_after_done', True)
        exc_wr_fields = kwargs.pop('exc_wr_fields', None)
        serializer_kwargs = {'data': request.data, 'many': many, 'account': request.user,}
        serializer_kwargs.update(kwargs.pop('serializer_kwargs', {}))
        if exc_wr_fields:
            serializer_kwargs['exc_wr_fields'] = exc_wr_fields
        serializer = self.get_serializer( **serializer_kwargs )
        serializer.is_valid(raise_exception=True)
        self.perform_create(serializer)
        if return_data_after_done:
            headers = self.get_success_headers(serializer.data)
            return_data = serializer.data
        else :
            headers = {}
            return_data = [] if many else {}
        self.log_action(action_type='create', request=request, many=many, serializer=serializer)
        response = RestResponse(return_data, status=RestStatus.HTTP_201_CREATED, headers=headers)
        return response


class BulkUpdateModelMixin(UpdateModelMixin, UserEditViewLogMixin):
    """ override update() method to add more arguments """
    def update(self, request, *args, **kwargs):
        many = kwargs.pop('many', False)
        return_data_after_done = kwargs.pop('return_data_after_done', True)
        partial = kwargs.pop('partial', False)
        allow_create = kwargs.pop('allow_create', False)

        pk_src = kwargs.pop('pk_src', LimitQuerySetMixin.REQ_SRC_QUERY_PARAMS)
        pk_param_name = kwargs.pop('pk_param_name', 'ids')
        pk_field_name = kwargs.pop('pk_field_name', 'id')
        pk_skip_list = kwargs.pop('pk_skip_list', None)
        if many:
            instance = self.get_queryset(pk_param_name=pk_param_name, pk_field_name=pk_field_name,
                        pk_src=pk_src, pk_skip_list=pk_skip_list )
        else:
            instance = self.get_object(pk_field_name=pk_field_name, skip_if_none=allow_create)

        if not instance and not allow_create:
            err_msgs = ["no instance found in update operation"]
            if hasattr(instance, '_error_msgs'):
                err_msgs.extend(instance._error_msgs)
            return_data = {api_settings.NON_FIELD_ERRORS_KEY:err_msgs}
            status = RestStatus.HTTP_400_BAD_REQUEST
        else:
            exc_wr_fields = kwargs.pop('exc_wr_fields', None)
            serializer_kwargs = {'data': request.data, 'partial':partial,'many': many, 'account': request.user,}
            serializer_kwargs.update(kwargs.pop('serializer_kwargs', {}))
            if exc_wr_fields:
                serializer_kwargs['exc_wr_fields'] = exc_wr_fields
            serializer = self.get_serializer(instance, **serializer_kwargs)
            serializer.is_valid(raise_exception=True)
            self.perform_update(serializer)
            if getattr(instance, '_prefetched_objects_cache', None):
                # If 'prefetch_related' has been applied to a queryset, we need to
                # forcibly invalidate the prefetch cache on the instance.
                instance._prefetched_objects_cache = {}
            if return_data_after_done:
                return_data = serializer.data
            else :
                return_data = [] if many else {}
            self.log_action(action_type='update', request=request, many=many, serializer=serializer)
            # TODO: always return async task ID if the API call put some tasks to message queue
            # , let frontend determine whether to check progress / status afterward
            status = kwargs.pop('status_ok', None) ## default is RestStatus.HTTP_200_OK
        return RestResponse(return_data, status=status)


class BulkDestroyModelMixin(DestroyModelMixin, UserEditViewLogMixin):
    # this mixin class has to work with any subclass of 
    # common.views.proxy.mixins.BaseGetProfileIDMixin
    def destroy(self, request, *args, **kwargs):
        many = kwargs.pop('many', False)
        status_ok = kwargs.pop('status_ok', RestStatus.HTTP_204_NO_CONTENT)
        pk_src = kwargs.pop('pk_src', LimitQuerySetMixin.REQ_SRC_QUERY_PARAMS)
        pk_param_name = kwargs.pop('pk_param_name', 'ids')
        pk_field_name = kwargs.pop('pk_field_name', 'id')
        pk_skip_list = kwargs.pop('pk_skip_list', None)
        extra_log_kwargs = {}
        if many:
            instance = self.get_queryset(pk_param_name=pk_param_name, pk_field_name=pk_field_name,
                        pk_src=pk_src, pk_skip_list=pk_skip_list)
            # must access database from here, to get ID values before deletion
            # after deletion, the ID values will be lost.
            _id_list = list(instance.values_list('pk', flat=True))
            model_cls = instance.model # get everything ready before they're deleted for logging
            item_labels = [obj.minimum_info for obj in instance]
        else:
            instance = self.get_object(pk_field_name=pk_field_name, skip_if_none=True,)
            _id_list = [instance.pk]
            model_cls = type(instance)
            item_labels = [instance.minimum_info]
        self.perform_destroy(instance=instance, id_list=_id_list)
        self._log_action(action_type='delete', request=request, affected_items=item_labels,
                model_cls=model_cls, profile_id=self.get_profile_id(request=request))
        return RestResponse(status=status_ok)


    def perform_destroy(self, instance, id_list):
        from softdelete.models import SoftDeleteObjectMixin, SoftDeleteQuerySet
        is_softdelete = isinstance(instance, (SoftDeleteObjectMixin, SoftDeleteQuerySet))
        _logger.debug("is_softdelete = %s", is_softdelete, request=self.request)
        kwargs = {}
        if is_softdelete:
            kwargs['profile_id'] = self.get_profile_id(request=self.request)
        # add callback accessibility because deletion is NOT handled by serializer
        failure_callback = getattr(self, 'delete_failure_callback', None)
        success_callback = getattr(self, 'delete_success_callback', None)
        try:
            instance.delete(**kwargs)
            if success_callback:
                success_callback(id_list=id_list)
        except Exception as e:
            if failure_callback:
                failure_callback(err=e, id_list=id_list)
            raise


