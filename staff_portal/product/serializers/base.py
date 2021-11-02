import logging
import pdb

from django.db.models import Q
from django.db.models.constants import LOOKUP_SEP
from django.core.exceptions     import ValidationError as DjangoValidationError
from rest_framework.fields      import IntegerField, CharField, BooleanField, empty as DRFEmptyData
from rest_framework.serializers import PrimaryKeyRelatedField, ListField

from common.validators   import  NumberBoundaryValidator, UnprintableCharValidator
from common.serializers  import  BulkUpdateListSerializer, ExtendedModelSerializer, DjangoBaseClosureBulkSerializer
from common.serializers.mixins  import  BaseClosureNodeMixin
from common.serializers.mixins.internal import AugmentEditFieldsMixin
from ..models.base import ProductTag, ProductTagClosure, ProductAttributeType, ProductSaleableItem, ProductSaleableItemMedia, ProductSaleableItemComposite, ProductSaleablePackage, ProductSaleablePackageMedia, ProductSaleablePackageComposite
from ..models.common import _atomicity_fn

from .common import BaseIngredientSerializer

_logger = logging.getLogger(__name__)


class TagClosureSerializer(ExtendedModelSerializer):
    atomicity = _atomicity_fn
    class Meta(ExtendedModelSerializer.Meta):
        model = ProductTagClosure
        fields = ['depth', 'ancestor', 'descendant']
        read_only_fields = ['depth']
    ancestor   = PrimaryKeyRelatedField(many=False,  queryset=ProductTag.objects.all())
    descendant = PrimaryKeyRelatedField(many=False,  queryset=ProductTag.objects.all())

    def to_representation(self, instance):
        if instance.depth > 0:
            out = super().to_representation(instance=instance)
        else:
            out = {}
        return out


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
                'item_cnt', 'pkg_cnt', 'num_children',]
        read_only_fields = ['usrprof']
        list_serializer_class = BulkTagSerializer

    ancestors   = TagClosureSerializer(many=True, read_only=True)
    descendants = TagClosureSerializer(many=True, read_only=True)
    item_cnt = IntegerField(read_only=True)
    pkg_cnt  = IntegerField(read_only=True)
    num_children = IntegerField(read_only=True)

    def __init__(self, instance=None, data=DRFEmptyData, **kwargs):
        self.usrprof_id = kwargs.pop('usrprof_id', None)
        super().__init__(instance=instance, data=data, **kwargs)

    def to_representation(self, instance):
        out = super().to_representation(instance=instance, _logger=_logger)
        if out.get('ancestors') is not None:
            out['ancestors']   = list(filter(any, out['ancestors']))
        if out.get('descendants') is not None:
            out['descendants'] = list(filter(any, out['descendants'] ))
        field_names = self.fields.keys()
        if 'num_children' in field_names:
            out['num_children'] = instance.descendants.filter(depth=1).count()
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


class CommonSaleableMediaMetaField(ListField):
    def __init__(self, *args, model=None, ingredient_field_name=None, **kwargs):
        assert model and ingredient_field_name, 'both of class variables `model` and `ingredient_field_name` must NOT be null'
        self.model = model
        self.ingredient_field_name = ingredient_field_name
        super().__init__(*args, **kwargs)
        extra_unprintable_set = (' ', '"', '\'', '\\')
        self.validators.append(UnprintableCharValidator(extra_unprintable_set))

    def to_representation(self, instance):
        assert instance.model is self.model, "model mismatch, failed to serialize media set"
        data = instance.values_list('media', flat=True)
        return list(data)

    def create(self, validated_data, ingredient):
        validated_data = validated_data or []
        def _new_obj_fn(res_id):
            _kwargs = {'media':res_id, self.ingredient_field_name:ingredient}
            return self.model(**_kwargs)
        objs = list(map(_new_obj_fn, validated_data))
        objs = self.model.objects.bulk_create(objs)
        return objs

    def update(self, validated_data, ingredient):
        validated_data = validated_data or []
        if any(validated_data):
            discarding =  ingredient.media_set.filter(~Q(media__in=validated_data))
            editing    =  ingredient.media_set.filter(media__in=validated_data)
            resource_ids = editing.values_list('media', flat=True)
            _new_item_fn = lambda resource_id: resource_id not in resource_ids
            discarding.delete(hard=True)
            new_validated_data = tuple(filter(_new_item_fn, validated_data))
            if new_validated_data:
                self.create(new_validated_data, ingredient)
        else:
            ingredient.media_set.all().delete(hard=True)
## end of class CommonSaleableMediaMetaField




class AbstractSaleableCompositeListSerializer(AugmentEditFieldsMixin, BulkUpdateListSerializer):
    _field_name_map = {}
    _assert_pk_field_name = ''

    def _retrieve_ingredient_ids(self, data):
        assert self.pk_field_name == self._assert_pk_field_name, 'Name of pk field should be %s , but git %s' % \
                (self._assert_pk_field_name, self.pk_field_name)
        _fn = lambda d: d[self.pk_field_name]
        ids = map(_fn, filter(_fn, data))
        conditions = {LOOKUP_SEP.join([self.pk_field_name,'in']) : ids}
        qset = self._current_ingredients_applied.filter(**conditions)
        qset = qset.values_list(self.pk_field_name, flat=True)
        return qset

    def _update_data_map(self, data):
        # the model of this serializer has compound key which consists 2 referential fields that always exists.
        # I cannot rely on the same function at parent class to determine whether an input data item should
        # go to insertion map or update map, therefore override the same function at here
        ingredient_ids = self._retrieve_ingredient_ids(data)
        return {item[self.pk_field_name]: item for item in data if \
                item[self.pk_field_name].pk in ingredient_ids}

    def _insert_data_map(self, data):
        ingredient_ids = self._retrieve_ingredient_ids(data)
        return [item for item in data if item[self.pk_field_name].pk not in ingredient_ids]

    def update(self, current_ingredients_applied=None, **kwargs):
        self._current_ingredients_applied = current_ingredients_applied
        return super().update(instance=current_ingredients_applied, allow_insert=True,
                allow_delete=True, **kwargs)


class SaleItemIngredientsAppliedListSerializer(AbstractSaleableCompositeListSerializer):
    _field_name_map = {'sale_item' :'sale_item',}
    _assert_pk_field_name = 'ingredient'
    def update(self, validated_data, sale_item, **kwargs):
        qset = sale_item.ingredients_applied.all()
        return super().update(validated_data=validated_data, sale_item=sale_item,
                current_ingredients_applied=qset, **kwargs)

class SalePkgItemsAppliedListSerializer(AbstractSaleableCompositeListSerializer):
    _field_name_map = {'package' :'package',}
    _assert_pk_field_name = 'sale_item'
    def update(self, validated_data, package, **kwargs):
        qset = package.saleitems_applied.all()
        return super().update(validated_data=validated_data, package=package,
                current_ingredients_applied=qset, **kwargs)



class AbstractSaleableCompositeSerializer(ExtendedModelSerializer):
    atomicity = _atomicity_fn

    def run_validation(self, data=DRFEmptyData):
        validated_value = super().run_validation(data=data, set_null_if_obj_not_found=True)
        return validated_value

    @property
    def presentable_fields_name(self):
        return set(self.Meta.fields) - set(self.Meta.read_only_fields)
## end of class AbstractSaleableCompositeSerializer


class SaleItemIngredientsAppliedSerializer(AbstractSaleableCompositeSerializer):
    class Meta(AbstractSaleableCompositeSerializer.Meta):
        model = ProductSaleableItemComposite
        fields = ['unit', 'quantity','ingredient', 'sale_item']
        read_only_fields = ['sale_item']
        list_serializer_class = SaleItemIngredientsAppliedListSerializer

    def __init__(self, *args, data=DRFEmptyData, **kwargs):
        # ignore `data` argument to parent class serializer to avoid creating default id field
        self.fields['quantity'].validators.append(NumberBoundaryValidator(limit=0.0, larger_than=True, include=False))
        # the reference model has composite primary key, but there is no need to
        # pass `sale_item` field as part of pk_field_name, also DO NOT pass `data` to ExtendedModelSerializer
        super().__init__(*args, pk_field_name='ingredient', **kwargs)


class SalePkgItemsAppliedSerializer(AbstractSaleableCompositeSerializer):
    class Meta(AbstractSaleableCompositeSerializer.Meta):
        model = ProductSaleablePackageComposite
        fields = ['unit', 'quantity','package', 'sale_item']
        read_only_fields = ['package']
        list_serializer_class = SalePkgItemsAppliedListSerializer

    def __init__(self, *args, data=DRFEmptyData, **kwargs):
        self.fields['quantity'].validators.append(NumberBoundaryValidator(limit=0.0, larger_than=True, include=False))
        super().__init__(*args, pk_field_name='sale_item', **kwargs)


class AbstractSaleableSerializer(BaseIngredientSerializer):
    atomicity = _atomicity_fn

    class Meta(BaseIngredientSerializer.Meta):
        fields = ['id','name', 'visible', 'price','usrprof']
        read_only_fields = ['usrprof']

    def __init__(self, instance=None, data=DRFEmptyData, usrprof_id=None, **kwargs):
        self.usrprof_id = usrprof_id
        self.fields['tags']  = PrimaryKeyRelatedField(many=True, queryset=ProductTag.objects.all())
        self.fields['price'].validators.append(NumberBoundaryValidator(limit=0.0, larger_than=True, include=False))
        super().__init__(instance=instance, data=data, **kwargs)

    def extra_setup_before_validation(self, instance, data):
        super().extra_setup_before_validation(instance=instance, data=data)
        # the ID of saleable item model is NOT auto-incremental column
        # DRF will not automatically clear `required` flag of ID field, so I
        # reset the `required` flag at here.
        self._mark_as_creation_on_update(pk_field_name='id', instance=instance, data=data)

    def extract_nested_form(self, formdata, nested_fields):
        out = {fname: formdata.pop(fname, []) for fname in nested_fields}
        return out

    def create(self, validated_data):
        validated_data['usrprof'] = self.usrprof_id
        nested_validated_data = self.extract_nested_form(formdata=validated_data, nested_fields=['media_set','tags',])
        instance = super().create(validated_data=validated_data)
        instance.tags.set(nested_validated_data['tags'])
        self.fields['media_set'].create(nested_validated_data['media_set'], instance)
        return  instance

    def update(self, instance, validated_data):
        validated_data.pop('usrprof', None)
        nested_validated_data = self.extract_nested_form(formdata=validated_data, nested_fields=['media_set','tags',])
        instance = super().update(instance=instance, validated_data=validated_data)
        instance.tags.set(nested_validated_data['tags'])
        self.fields['media_set'].update(nested_validated_data['media_set'], instance)
        return instance
## end of class AbstractSaleableSerializer


class SaleableItemSerializer(AbstractSaleableSerializer):
    class Meta(AbstractSaleableSerializer.Meta):
        model =  ProductSaleableItem

    def __init__(self, instance=None, data=DRFEmptyData, **kwargs):
        self.fields['media_set'] = CommonSaleableMediaMetaField(child=CharField(max_length=42),
                model=ProductSaleableItemMedia, ingredient_field_name='sale_item')
        self.fields['ingredients_applied'] = SaleItemIngredientsAppliedSerializer(many=True, instance=instance)
        super().__init__(instance=instance, data=data, **kwargs)

    def extra_setup_before_validation(self, instance, data):
        super().extra_setup_before_validation(instance=instance, data=data)
        self._setup_subform_instance(name='ingredients_applied', instance=instance, data=data,
                pk_field_name=('ingredient', 'sale_item'))

    def create(self, validated_data):
        nested_validated_data = self.extract_nested_form(formdata=validated_data, nested_fields=['ingredients_applied'])
        instance = super().create(validated_data=validated_data)
        self.fields['ingredients_applied'].create(nested_validated_data['ingredients_applied'],  sale_item=instance)
        return  instance

    def update(self, instance, validated_data):
        nested_validated_data = self.extract_nested_form(formdata=validated_data, nested_fields=['ingredients_applied'])
        instance = super().update(instance=instance, validated_data=validated_data)
        self.fields['ingredients_applied'].update(sale_item=instance,  validated_data=nested_validated_data['ingredients_applied'])
        return instance
## end of class SaleableItemSerializer



class SaleablePackageSerializer(AbstractSaleableSerializer):
    class Meta(AbstractSaleableSerializer.Meta):
        model =  ProductSaleablePackage

    def __init__(self, instance=None, data=DRFEmptyData, **kwargs):
        self.fields['media_set'] = CommonSaleableMediaMetaField(child=CharField(max_length=42),
                model=ProductSaleablePackageMedia, ingredient_field_name='sale_pkg')
        self.fields['saleitems_applied'] = SalePkgItemsAppliedSerializer(many=True, instance=instance)
        super().__init__(instance=instance, data=data, **kwargs)

    def extra_setup_before_validation(self, instance, data):
        super().extra_setup_before_validation(instance=instance, data=data)
        self._setup_subform_instance(name='saleitems_applied', instance=instance, data=data,
                pk_field_name=('package', 'sale_item'))

    def create(self, validated_data):
        nested_validated_data = self.extract_nested_form(formdata=validated_data, nested_fields=['saleitems_applied'])
        instance = super().create(validated_data=validated_data)
        self.fields['saleitems_applied'].create(nested_validated_data['saleitems_applied'], package=instance)
        return  instance

    def update(self, instance, validated_data):
        nested_validated_data = self.extract_nested_form(formdata=validated_data, nested_fields=['saleitems_applied'])
        instance = super().update(instance=instance, validated_data=validated_data)
        self.fields['saleitems_applied'].update(package=instance,  validated_data=nested_validated_data['saleitems_applied'])
        return instance
## end of class SaleablePackageSerializer

