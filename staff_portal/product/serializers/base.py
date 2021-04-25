import logging
import pdb

from django.core.exceptions     import ValidationError as DjangoValidationError
from rest_framework.fields      import IntegerField, CharField, BooleanField, empty as DRFEmptyData

from common.util.python.messaging.rpc  import  RpcReplyEvent
from common.serializers  import  BulkUpdateListSerializer, ExtendedModelSerializer, DjangoBaseClosureBulkSerializer
from common.serializers.mixins  import  BaseClosureNodeMixin
from ..models.base import ProductTag, ProductTagClosure

_logger = logging.getLogger(__name__)


class ConnectedTagField(ExtendedModelSerializer):
    class Meta(ExtendedModelSerializer.Meta):
        model = ProductTag
        fields = ['id', 'name']
        read_only_fields = ['name']

class TagClosureSerializer(ExtendedModelSerializer):
    class Meta(ExtendedModelSerializer.Meta):
        model = ProductTagClosure
        fields = ['id', 'depth', 'ancestor', 'descendant']
        read_only_fields = ['depth']
    ancestor   = ConnectedTagField(read_only=True)
    descendant = ConnectedTagField(read_only=True)

class BulkTagSerializer(DjangoBaseClosureBulkSerializer):
    CLOSURE_MODEL_CLS     = TagClosureSerializer.Meta.model
    PK_FIELD_NAME         = TagClosureSerializer.Meta.model.id.field.name
    DEPTH_FIELD_NAME      = TagClosureSerializer.Meta.model.depth.field.name
    ANCESTOR_FIELD_NAME   = TagClosureSerializer.Meta.model.ancestor.field.name
    DESCENDANT_FIELD_NAME = TagClosureSerializer.Meta.model.descendant.field.name


class TagSerializer(BaseClosureNodeMixin, ExtendedModelSerializer):
    class Meta(BaseClosureNodeMixin.Meta, ExtendedModelSerializer.Meta):
        model = ProductTag
        fields = ['id', 'name', 'ancestors', 'descendants', 'usrprof',
                'item_cnt', 'pkg_cnt', 'desc_cnt',]
        read_only_fields = ['usrprof']
        list_serializer_class = BulkTagSerializer

    ancestors   = TagClosureSerializer(many=True, read_only=True)
    descendants = TagClosureSerializer(many=True, read_only=True)
    item_cnt = IntegerField(read_only=True)
    pkg_cnt  = IntegerField(read_only=True)
    desc_cnt = IntegerField(read_only=True)

    def __init__(self, instance=None, data=DRFEmptyData, **kwargs):
        self.usrprof_id = kwargs.pop('usrprof_id', None)
        super().__init__(instance=instance, data=data, **kwargs)

    def to_representation(self, instance):
        out = super().to_represent(instance=instance, _logger=_logger)
        field_names = self.fields.keys()
        if 'desc_cnt' in field_names:
            out['desc_cnt'] = instance.descendants.filter(depth=1).count()
        if 'item_cnt' in field_names:
            out['item_cnt'] = instance.tagged_products.count()
        if 'pkg_cnt' in field_names:
            out['pkg_cnt'] = instance.tagged_packages.count()
        #pdb.set_trace()
        return out

    def validate(self, value):
        return super().validate(value=value, exception_cls=DjangoValidationError, _logger=_logger)

    def create(self, validated_data):
        validated_data['usrprof'] = self.usrprof_id
        return  super().create(validated_data=validated_data)

    def update(self, instance, validated_data):
        validated_data['usrprof'] = self.usrprof_id
        return  super().update(instance=instance, validated_data=validated_data)


