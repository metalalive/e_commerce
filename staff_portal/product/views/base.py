import copy
import logging

from django.db import models
from django.contrib.contenttypes.models  import ContentType
from rest_framework             import status as RestStatus
from rest_framework.filters     import OrderingFilter, SearchFilter

from common.views.api     import  AuthCommonAPIView, AuthCommonAPIReadView
from common.views.mixins  import  LimitQuerySetMixin
from common.views.filters import  ClosureTableFilter, AggregateFieldOrderingFilter
from common.views.proxy.mixins import RemoteGetProfileIDMixin
from softdelete.views import RecoveryModelMixin

from ..models.base import ProductTagClosure
from ..models.common import ProductmgtChangeSet
from ..serializers.base import TagSerializer, AttributeTypeSerializer, SaleableItemSerializer
from ..permissions import TagsPermissions, AttributeTypePermissions, SaleableItemPermissions

from .common import BaseIngredientSearchFilter

_logger = logging.getLogger(__name__)

class TagOrderFilter(AggregateFieldOrderingFilter):
    _aggregate_fields  = {
            'item_cnt' : ('tagged_products', models.Count('tagged_products')),
            'pkg_cnt'  : ('tagged_packages', models.Count('tagged_packages')),
        }

TagOrderFilter.mirror()

class TagSearchFilter(SearchFilter):
    def filter_queryset(self, request, queryset, view):
        origin_qs = queryset
        queryset = super().filter_queryset(request=request, queryset=queryset, view=view)
        if origin_qs is not queryset: # which implicitly means new queryset contains search result.
            matched_ids = queryset.values_list('pk', flat=True)
            ascs_ids = origin_qs.get_ascs_descs_id(IDs=matched_ids, fetch_asc=True,
                    fetch_desc=False, depth=-1, self_exclude=False)
            queryset = origin_qs.filter(pk__in=ascs_ids)
        return queryset


class TagView(AuthCommonAPIView, RemoteGetProfileIDMixin):
    serializer_class  = TagSerializer
    filter_backends = [TagsPermissions, TagSearchFilter, ClosureTableFilter, TagOrderFilter,]
    closure_model_cls = ProductTagClosure
    ordering_fields  = ['id', 'name', 'item_cnt', 'pkg_cnt', 'desc_cnt']
    search_fields  = ['name']
    permission_classes = copy.copy(AuthCommonAPIView.permission_classes) + [TagsPermissions]
    queryset = serializer_class.Meta.model.objects.all()
        #annotate(
        #    item_cnt=models.Count('tagged_products'),
        #    pkg_cnt=models.Count('tagged_packages'),
        #    desc_cnt=models.Count('descendants'),
        #    asc_cnt=models.Count('ancestors'),
        #)

    def get_IDs(self, pk_param_name='pks', pk_field_name='pk', delimiter=',',
            pk_src=LimitQuerySetMixin.REQ_SRC_QUERY_PARAMS, pk_skip_list=None):
        IDs = super().get_IDs(pk_param_name=pk_param_name, pk_field_name=pk_field_name,
                delimiter=delimiter, pk_src=pk_src, pk_skip_list=pk_skip_list)
        if self.request.method == 'GET' and  (self._fetch_desc or self._fetch_asc):
            depth = self.request.query_params.get('depth', '')
            IDs = self.queryset.get_ascs_descs_id(IDs=IDs, fetch_asc=self._fetch_asc,
                    fetch_desc=self._fetch_desc, depth=depth)
        #pdb.set_trace()
        return IDs
    # end of get_IDs()

    def get(self, request, *args, **kwargs):
        self._fetch_desc = request.path.endswith('descendants')
        self._fetch_asc  = request.path.endswith('ancestors')
        if self._fetch_desc or self._fetch_asc:
            # it is hacky, but I need query_param to be temporarily mutable
            query_params = self.request.query_params
            backup_mutable = query_params._mutable
            query_params._mutable = True
            query_params['ids'] = kwargs.pop('pk', 'root')
            query_params._mutable = backup_mutable
        exc_rd_fields = ['ancestors__id', 'descendants__id', 'ancestors__ancestor__name',
                'descendants__descendant__name']
        kwargs['serializer_kwargs'] = {'exc_rd_fields': exc_rd_fields,}
        return super().get(request=request, *args, **kwargs)

    def post(self, request, *args, **kwargs):
        prof_id = self.get_profile_id(request=request, num_of_msgs_fetch=2)
        kwargs['many'] = True
        kwargs['return_data_after_done'] = True
        kwargs['serializer_kwargs'] = {'usrprof_id': prof_id}
        return  self.create(request, *args, **kwargs)

    def put(self, request, *args, **kwargs):
        prof_id = self.get_profile_id(request=request, num_of_msgs_fetch=2)
        kwargs['many'] = True
        kwargs['pk_src'] =  LimitQuerySetMixin.REQ_SRC_BODY_DATA
        kwargs['return_data_after_done'] = False
        kwargs['serializer_kwargs'] = {'usrprof_id': prof_id}
        return  self.update(request, *args, **kwargs)

    def delete(self, request, *args, **kwargs):
        kwargs['many'] = True
        kwargs['pk_src'] =  LimitQuerySetMixin.REQ_SRC_BODY_DATA
        return self.destroy(request, *args, **kwargs)


class AttributeTypeView(AuthCommonAPIView, RemoteGetProfileIDMixin, RecoveryModelMixin):
    serializer_class  = AttributeTypeSerializer
    # TODO, ffigure out whether I can use AggregateFieldOrderingFilter
    filter_backends = [SearchFilter, OrderingFilter,]
    ordering_fields  = ['id', 'name', 'dtype',]
    search_fields  = ['name']
    permission_classes = copy.copy(AuthCommonAPIView.permission_classes) + [AttributeTypePermissions]
    queryset = serializer_class.Meta.model.objects.all()
    SOFTDELETE_CHANGESET_MODEL = ProductmgtChangeSet

    def post(self, request, *args, **kwargs):
        kwargs['many'] = True
        kwargs['return_data_after_done'] = True
        return  self.create(request, *args, **kwargs)

    def put(self, request, *args, **kwargs):
        kwargs['many'] = True
        kwargs['pk_src'] =  LimitQuerySetMixin.REQ_SRC_BODY_DATA
        kwargs['return_data_after_done'] = False
        return  self.update(request, *args, **kwargs)

    def delete(self, request, *args, **kwargs):
        # note that if attribute type is deleted, it will NOT cause the saleable items which
        # have the same attribute type losing its attribute value. Subsequent new saleable items
        # cannot have the deleted attribute from the point of time of the deletion on . 
        kwargs['many'] = True
        kwargs['pk_src'] =  LimitQuerySetMixin.REQ_SRC_BODY_DATA
        return self.destroy(request, *args, **kwargs)

    def patch(self, request, *args, **kwargs):
        kwargs['resource_content_type'] = ContentType.objects.get(app_label='product',
                model=self.serializer_class.Meta.model.__name__)
        kwargs['return_data_after_done'] = True
        return self.recovery(request=request, *args, **kwargs)


class SaleableItemSearchFilter(BaseIngredientSearchFilter):
    pass


class SaleableItemView(AuthCommonAPIView, RemoteGetProfileIDMixin, RecoveryModelMixin):
    serializer_class  = SaleableItemSerializer
    filter_backends = [SaleableItemSearchFilter, OrderingFilter,]
    #### ordering_fields  = ['-id', 'name', 'category']
    search_fields  = ['name',]
    permission_classes = copy.copy(AuthCommonAPIView.permission_classes) + [SaleableItemPermissions]
    queryset = serializer_class.Meta.model.objects.all()
    SOFTDELETE_CHANGESET_MODEL = ProductmgtChangeSet

    def post(self, request, *args, **kwargs):
        prof_id = self.get_profile_id(request=request, num_of_msgs_fetch=2)
        kwargs['many'] = True
        kwargs['return_data_after_done'] = True
        kwargs['serializer_kwargs'] = {'usrprof_id': prof_id}
        return  self.create(request, *args, **kwargs)

    def put(self, request, *args, **kwargs):
        kwargs['many'] = True
        kwargs['pk_src'] =  LimitQuerySetMixin.REQ_SRC_BODY_DATA
        kwargs['return_data_after_done'] = True
        return  self.update(request, *args, **kwargs)

    def delete(self, request, *args, **kwargs):
        kwargs['many'] = True
        kwargs['pk_src'] =  LimitQuerySetMixin.REQ_SRC_BODY_DATA
        kwargs['status_ok'] = RestStatus.HTTP_202_ACCEPTED
        return self.destroy(request, *args, **kwargs)

    def patch(self, request, *args, **kwargs):
        kwargs['resource_content_type'] = ContentType.objects.get(app_label='product',
                model=self.serializer_class.Meta.model.__name__)
        kwargs['return_data_after_done'] = True
        return self.recovery(request=request, *args, **kwargs)



