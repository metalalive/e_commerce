import logging
import pdb

from django.db.models import Q
from django.core.exceptions     import ValidationError as DjangoValidationError
from rest_framework.fields      import IntegerField, CharField, BooleanField, empty as DRFEmptyData
from rest_framework.serializers import PrimaryKeyRelatedField, ListField

from common.serializers  import  BulkUpdateListSerializer, ExtendedModelSerializer, DjangoBaseClosureBulkSerializer
from common.serializers.mixins  import  BaseClosureNodeMixin
from common.serializers.mixins.internal import AugmentEditFieldsMixin
from ..models.base import ProductTag, ProductTagClosure, ProductAttributeType, ProductSaleableItem, ProductSaleableItemMedia, ProductSaleableItemComposite
from ..models.common import _atomicity_fn

from .common import BaseIngredientSerializer

_logger = logging.getLogger(__name__)


class ConnectedTagField(ExtendedModelSerializer):
    class Meta(ExtendedModelSerializer.Meta):
        model = ProductTag
        fields = ['id', 'name']
        read_only_fields = ['name']

class TagClosureSerializer(ExtendedModelSerializer):
    atomicity = _atomicity_fn
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
    atomicity = _atomicity_fn
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
        self.exc_rd_fields = kwargs.pop('exc_rd_fields', None)
        self.usrprof_id = kwargs.pop('usrprof_id', None)
        super().__init__(instance=instance, data=data, **kwargs)

    def to_representation(self, instance):
        out = super().to_representation(instance=instance, _logger=_logger)
        field_names = self.fields.keys()
        if 'desc_cnt' in field_names:
            out['desc_cnt'] = instance.descendants.filter(depth=1).count()
        if 'item_cnt' in field_names:
            out['item_cnt'] = instance.tagged_products.count()
        if 'pkg_cnt' in field_names:
            out['pkg_cnt'] = instance.tagged_packages.count()
        return out

    def validate(self, value):
        return super().validate(value=value, exception_cls=DjangoValidationError, _logger=_logger)

    def create(self, validated_data):
        validated_data['usrprof'] = self.usrprof_id
        return  super().create(validated_data=validated_data)

    def update(self, instance, validated_data):
        validated_data['usrprof'] = self.usrprof_id
        return  super().update(instance=instance, validated_data=validated_data)


class AttributeTypeSerializer(ExtendedModelSerializer):
    atomicity = _atomicity_fn
    class Meta(ExtendedModelSerializer.Meta):
        model = ProductAttributeType
        fields = ['id', 'name', 'dtype',]


class SaleItemMediaMetaField(ListField):
    model = ProductSaleableItemMedia

    def create(self, validated_data, sale_item):
        validated_data = validated_data or []
        _new_obj_fn = lambda res_id: self.model(media=res_id, sale_item=sale_item)
        objs = list(map(_new_obj_fn, validated_data))
        objs = self.model.objects.bulk_create(objs)
        return objs

    def update(self, validated_data, sale_item):
        validated_data = validated_data or []
        if any(validated_data):
            discarding = sale_item.media_set.filter(~Q(media__in=validated_data))
            editing    = sale_item.media_set.filter(media__in=validated_data)
            resource_ids = editing.values_list('media', flat=True)
            _new_item_fn = lambda resource_id: resource_id not in resource_ids
            discarding.delete(hard=True)
            new_validated_data = tuple(filter(_new_item_fn, validated_data))
            if new_validated_data:
                self.create(new_validated_data, sale_item)
        else:
            sale_item.media_set.all().delete(hard=True)


class SaleItemIngredientsAppliedListSerializer(AugmentEditFieldsMixin, BulkUpdateListSerializer):
    _field_name_map = {'sale_item' :'sale_item',}

    def _retrieve_ingredient_ids(self, data):
        assert self.pk_field_name == 'ingredient', ''
        _fn = lambda d: d[self.pk_field_name]
        ids = map(_fn, filter(_fn, data))
        qset = self._current_ingredients_applied.filter(ingredient__in=ids)
        qset = qset.values_list(self.pk_field_name, flat=True)
        return qset

    def _update_data_map(self, data):
        ingredient_ids = self._retrieve_ingredient_ids(data)
        return {item[self.pk_field_name]: item for item in data if \
                item[self.pk_field_name].pk in ingredient_ids}

    def _insert_data_map(self, data):
        ingredient_ids = self._retrieve_ingredient_ids(data)
        return [item for item in data if item[self.pk_field_name].pk not in ingredient_ids]

    def update(self, validated_data, sale_item, **kwargs):
        qset = sale_item.ingredients_applied.all()
        self._current_ingredients_applied = qset
        return super().update(instance=qset, validated_data=validated_data, sale_item=sale_item,
                allow_insert=True, allow_delete=True, **kwargs)

class SaleItemIngredientsAppliedSerializer(ExtendedModelSerializer):
    atomicity = _atomicity_fn

    class Meta(ExtendedModelSerializer.Meta):
        model = ProductSaleableItemComposite
        fields = ['unit', 'quantity', 'ingredient', 'sale_item']
        read_only_fields = ['sale_item']
        list_serializer_class = SaleItemIngredientsAppliedListSerializer

    def __init__(self, *args, data=DRFEmptyData, **kwargs):
        # ignore `data` argument to parent class serializer to avoid creating
        # default id field
        # the reference model has composite primary key, but there is no need to
        # pass `sale_item` field as part of pk_field_name
        super().__init__(*args, pk_field_name='ingredient', **kwargs)

    def run_validation(self, data=DRFEmptyData):
        validated_value = super().run_validation(data=data, set_null_if_obj_not_found=True)
        return validated_value



class SaleableItemSerializer(BaseIngredientSerializer):
    atomicity = _atomicity_fn

    class Meta(BaseIngredientSerializer.Meta):
        model =  ProductSaleableItem
        fields = ['id','name', 'visible', 'price',]

    def __init__(self, instance=None, data=DRFEmptyData, **kwargs):
        self.fields['tags']  = PrimaryKeyRelatedField(many=True, queryset=ProductTag.objects.all())
        self.fields['media_set'] = SaleItemMediaMetaField(child=CharField(max_length=42))
        self.fields['ingredients_applied'] = SaleItemIngredientsAppliedSerializer(many=True, instance=instance)
        super().__init__(instance=instance, data=data, **kwargs)

    def extra_setup_before_validation(self, instance, data):
        super().extra_setup_before_validation(instance=instance, data=data)
        self._setup_subform_instance(name='ingredients_applied', instance=instance, data=data,
                pk_field_name=('ingredient', 'sale_item'))
        # the ID of saleable item model is NOT auto-incremental column
        # DRF will not automatically clear `required` flag of ID field, so I
        # reset the `required` flag at here.
        self._mark_as_creation_on_update(pk_field_name='id', instance=instance, data=data)

    def run_validation(self, data=DRFEmptyData):
        try:
            validated_value = super().run_validation(data=data)
        except Exception as e:
            raise
        return validated_value

    def extract_nested_form(self, formdata):
        nested_fields = ['tags','media_set','ingredients_applied']
        out = {fname: formdata.pop(fname, []) for fname in nested_fields}
        return out

    def create(self, validated_data):
        nested_validated_data = self.extract_nested_form(formdata=validated_data)
        validated_data['usrprof'] = self._account.pk
        instance = super().create(validated_data=validated_data)
        instance.tags.set(nested_validated_data['tags'])
        self.fields['media_set'].create(nested_validated_data['media_set'], sale_item=instance)
        self.fields['ingredients_applied'].create(nested_validated_data['ingredients_applied'],  sale_item=instance)
        return  instance

    def update(self, instance, validated_data):
        nested_validated_data = self.extract_nested_form(formdata=validated_data)
        instance = super().update(instance=instance, validated_data=validated_data)
        instance.tags.set(nested_validated_data['tags'])
        self.fields['media_set'].update(sale_item=instance,  validated_data=nested_validated_data['media_set'])
        self.fields['ingredients_applied'].update(sale_item=instance,  validated_data=nested_validated_data['ingredients_applied'])
        return instance
## end of class SaleableItemSerializer


