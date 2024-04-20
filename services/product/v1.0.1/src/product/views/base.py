import copy
import logging

from django.db import models
from django.core.exceptions  import ObjectDoesNotExist
from django.contrib.contenttypes.models  import ContentType
from rest_framework            import status as RestStatus
from rest_framework.filters    import OrderingFilter, SearchFilter
from rest_framework.exceptions import NotFound

from ecommerce_common.auth.django.authentication import RemoteAccessJWTauthentication
from ecommerce_common.views.api     import  AuthCommonAPIView, AuthCommonAPIReadView
from ecommerce_common.views.mixins  import  LimitQuerySetMixin
from ecommerce_common.views.filters import  ClosureTableFilter, AggregateFieldOrderingFilter
from softdelete.views import RecoveryModelMixin

from ..models.base import ProductTagClosure
from ..models.common import ProductmgtChangeSet
from ..serializers.base import TagSerializer, AttributeTypeSerializer, SaleableItemSerializer, SaleablePackageSerializer
from ..permissions import TagsPermissions, AttributeTypePermissions, SaleableItemPermissions, SaleablePackagePermissions

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


class TagView(AuthCommonAPIView):
    serializer_class  = TagSerializer
    filter_backends = [TagsPermissions, TagSearchFilter, ClosureTableFilter, TagOrderFilter,]
    closure_model_cls = ProductTagClosure
    ordering_fields  = ['id', 'name', 'item_cnt', 'pkg_cnt', 'num_children']
    search_fields  = ['name']
    authentication_classes = [RemoteAccessJWTauthentication]
    permission_classes = copy.copy(AuthCommonAPIView.permission_classes) + [TagsPermissions]
    queryset = serializer_class.Meta.model.objects.all()

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
        return super().get(request=request, *args, **kwargs)

    def post(self, request, *args, **kwargs):
        kwargs['many'] = True
        kwargs['return_data_after_done'] = True
        kwargs['serializer_kwargs'] = {'usrprof_id': request.user.profile}
        return  self.create(request, *args, **kwargs)

    def put(self, request, *args, **kwargs):
        kwargs['many'] = True
        kwargs['pk_src'] =  LimitQuerySetMixin.REQ_SRC_BODY_DATA
        kwargs['return_data_after_done'] = False
        kwargs['serializer_kwargs'] = {'usrprof_id': request.user.profile}
        return  self.update(request, *args, **kwargs)

    def delete(self, request, *args, **kwargs):
        kwargs['many'] = True
        kwargs['pk_src'] =  LimitQuerySetMixin.REQ_SRC_BODY_DATA
        return self.destroy(request, *args, **kwargs)


class AttributeTypeView(AuthCommonAPIView, RecoveryModelMixin):
    serializer_class  = AttributeTypeSerializer
    # TODO, ffigure out whether I can use AggregateFieldOrderingFilter
    filter_backends = [SearchFilter, OrderingFilter,]
    ordering_fields  = ['id', 'name', 'dtype',]
    search_fields  = ['name']
    authentication_classes = [RemoteAccessJWTauthentication]
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
        return self.recovery(request=request, *args, profile_id=request.user.profile, **kwargs)


class TaggedSaleableView(AuthCommonAPIReadView):
    filter_backends = [BaseIngredientSearchFilter, OrderingFilter,]
    ordering_fields  = ['name', 'price']
    search_fields  = ['name',]
    authentication_classes = [RemoteAccessJWTauthentication]
    serializer_class  = None
    queryset = None

    def get(self, request, *args, **kwargs):
        tag_id = kwargs.get('tag_id', None)
        try:
            tag_obj = TagSerializer.Meta.model.objects.get(id=tag_id)
        except (ValueError, ObjectDoesNotExist) as e:
            raise NotFound()
        self.serializer_class = SaleableItemSerializer
        self.queryset = tag_obj.tagged_products.all()
        resp_saleitems = self.list(request, *args, **kwargs)
        self.serializer_class = SaleablePackageSerializer
        self.queryset = tag_obj.tagged_packages.all()
        resp_salepkgs  = self.list(request, *args, **kwargs)
        resp_data = {'items': resp_saleitems.data, 'pkgs':resp_salepkgs.data}
        resp_saleitems.data = resp_data
        return resp_saleitems


class SaleableBaseView(AuthCommonAPIView, RecoveryModelMixin):
    filter_backends = [BaseIngredientSearchFilter, OrderingFilter,]
    ordering_fields  = ['name', 'price']
    search_fields  = ['name',]
    authentication_classes = [RemoteAccessJWTauthentication]
    SOFTDELETE_CHANGESET_MODEL = ProductmgtChangeSet

    def post(self, request, *args, **kwargs):
        kwargs['many'] = True
        kwargs['return_data_after_done'] = True
        kwargs['serializer_kwargs'] = {'usrprof_id': request.user.profile}
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
        return self.recovery(request=request, *args, profile_id=request.user.profile, **kwargs)


class SaleableItemView(SaleableBaseView):
    serializer_class  = SaleableItemSerializer
    permission_classes = AuthCommonAPIView.permission_classes.copy() + [SaleableItemPermissions]
    queryset = serializer_class.Meta.model.objects.all()
    ordering_fields = SaleableBaseView.ordering_fields.copy() + ['ingredients_applied__ingredient__category']
    search_fields = SaleableBaseView.search_fields.copy() + ['ingredients_applied__ingredient__name', 'pkgs_applied__package__name']

class SaleablePackageView(SaleableBaseView):
    serializer_class  = SaleablePackageSerializer
    permission_classes = copy.copy(AuthCommonAPIView.permission_classes) + [SaleablePackagePermissions]
    queryset = serializer_class.Meta.model.objects.all()
    search_fields = SaleableBaseView.search_fields.copy() + ['saleitems_applied__sale_item__name']


