import copy
import logging

from django.contrib.contenttypes.models  import ContentType
from rest_framework             import status as RestStatus
from rest_framework.filters     import OrderingFilter

from ecommerce_common.auth.django.authentication import RemoteAccessJWTauthentication
from ecommerce_common.views.mixins  import  LimitQuerySetMixin
from ecommerce_common.views.api     import  AuthCommonAPIView, AuthCommonAPIReadView
from softdelete.views import RecoveryModelMixin

from ..models.common import ProductmgtChangeSet
#from ..serializers.base import  AttributeTypeSerializer
from ..serializers.development import FabricationIngredientSerializer
from ..permissions import FabricationIngredientPermissions
from  .common import BaseIngredientSearchFilter

_logger = logging.getLogger(__name__)


class FabricationIngredientView(AuthCommonAPIView, RecoveryModelMixin):
    serializer_class  = FabricationIngredientSerializer
    filter_backends = [BaseIngredientSearchFilter, OrderingFilter,] # TODO, figure out how to search with attribute type/value
    ordering_fields  = ['-id', 'name', 'category']
    search_fields  = ['name', 'category']
    authentication_classes = [RemoteAccessJWTauthentication]
    permission_classes = copy.copy(AuthCommonAPIView.permission_classes) + [FabricationIngredientPermissions]
    queryset = serializer_class.Meta.model.objects.all()
    SOFTDELETE_CHANGESET_MODEL = ProductmgtChangeSet

    def post(self, request, *args, **kwargs):
        kwargs['many'] = True
        kwargs['return_data_after_done'] = True
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



