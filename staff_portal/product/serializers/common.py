import copy
import logging

from django.core.exceptions   import ValidationError as DjangoValidationError
from django.contrib.contenttypes.models  import ContentType
from rest_framework.fields    import empty as DRFEmptyData
from rest_framework.exceptions  import ValidationError as RestValidationError, ErrorDetail as RestErrorDetail

from common.serializers  import  ExtendedModelSerializer, BulkUpdateListSerializer
from common.serializers.mixins  import NestedFieldSetupMixin
from common.serializers.mixins.internal import AugmentEditFieldsMixin
from ..models.base import ProductAttributeValueStr, ProductAttributeValuePosInt, ProductAttributeValueInt, ProductAttributeValueFloat
from ..models.common import _atomicity_fn

_logger = logging.getLogger(__name__)

src_field_label = {'_list':'attributes', 'type':'type', 'value':'value'}
attribute_field_label = src_field_label

class AugmentIngredientRefMixin(AugmentEditFieldsMixin):
    _field_name_map = {'ingredient' :'_ingredient_instance', }

    def append_ingredient(self, name, instance, data):
        # append user field to data
        # instance must be user group or user profile
        # the 2 fields are never modified by client data
        if instance and instance.pk:
            for d in data.get(name, []):
                d['ingredient_type'] = ContentType.objects.get_for_model(instance).pk
                d['ingredient_id'] = instance.pk

class AttrValueListSerializer(AugmentIngredientRefMixin, BulkUpdateListSerializer):
    # the field name that should be given in the client request body
    def _get_valid_types(self, src):
        if not hasattr(self, '_valid_attr_types'):
            _label = src_field_label
            fn = lambda d:d.get(_label['type'])
            valid_attrs = filter(fn, src)
            attrtype_ids = list(map(fn, valid_attrs))
            qset = self.child.fields['attr_type'].queryset
            try:
                qset = qset.filter(pk__in=attrtype_ids)
                self._valid_attr_types = qset
            except ValueError as ve:
                errmsg = {_label['_list']: ['%s contains invalid data type of pk' % attrtype_ids]}
                raise DjangoValidationError(errmsg)
        return self._valid_attr_types

    def extract(self, data, dst_field_name):
        _label = src_field_label
        src = data.get(_label['_list'], [])
        valid_types = self._get_valid_types(src=src)
        valid_types = valid_types.filter(dtype=self.child.Meta.model.DATATYPE)
        valid_types = valid_types.values_list('id', flat=True)
        extracted = [d for d in src if d.get(_label['type']) in valid_types]
        if any(extracted):
            for d in extracted:
                src.remove(d)
            data[dst_field_name] = extracted
            self.read_only = False
        else:
            # consider parent serializer will call this function multiple
            # times for validation, reset the read-only state if frontend send
            # some attribute types to store
            self.read_only = True # TODO, what if it is update operation ?
        #if dst_field_name == 'attr_val_float':
        #    import pdb
        #    pdb.set_trace()

    def run_validation(self, data=DRFEmptyData):
        return super().run_validation(data=data)


class AbstractAttrValueSerializer(ExtendedModelSerializer, NestedFieldSetupMixin):
    atomicity = _atomicity_fn
    class Meta(ExtendedModelSerializer.Meta):
        fields = ['id', 'ingredient_type', 'ingredient_id', 'attr_type', 'value']
        read_only_fields = ['ingredient_type', 'ingredient_id']
        list_serializer_class = AttrValueListSerializer

    @property
    def presentable_fields_name(self):
        out = super().presentable_fields_name
        if not 'attr_type' in out:
            out.extend(['attr_type'])
        return out

    def to_representation(self, instance):
        out = super().to_representation(instance=instance)
        stored_attrtype = out.pop('attr_type' , None)
        if not stored_attrtype:
            log_msg = ['srlz_cls', type(self).__qualname__,
                    'msg', 'product attribute type must not be empty in type-value pair',
                    'id', out.get('id',None) , 'value',out.get('value',None) ]
            _logger.error(None, *log_msg)
        out[src_field_label['type']] = stored_attrtype
        return out

    def extra_setup_before_validation(self, instance, data):
        self._mark_as_creation_on_update(pk_field_name='id', instance=instance, data=data)

    def run_validation(self, data=DRFEmptyData):
        # internally change key `type` --> `attr_type` ... should seperate to extra function
        unverified_attrtype = data.pop(src_field_label['type'], None)
        if unverified_attrtype:
            data['attr_type'] = unverified_attrtype
        try:
            validated_data = super().run_validation(data=data, set_null_if_obj_not_found=True)
        except RestValidationError as ve:
            _value = data.get('value', None)
            _type  = data.get('attr_type', None)
            ve.detail['_origin'] = {src_field_label['value']:_value , src_field_label['type']:_type}
            raise
        return validated_data

    def create(self, validated_data):
        ingre = validated_data.pop('_ingredient_instance', None)
        validated_data['ingredient_type'] = ContentType.objects.get_for_model(ingre)
        validated_data['ingredient_id'] = ingre.pk
        log_msg = ['srlz_cls', type(self).__qualname__, 'validated_data', validated_data]
        _logger.debug(None, *log_msg)
        instance = super().create(validated_data=validated_data)
        return instance
## end of class AbstractAttrValueSerializer


class AttrValueStrSerializer(AbstractAttrValueSerializer):
    class Meta(AbstractAttrValueSerializer.Meta):
        model = ProductAttributeValueStr

class AttrValuePosIntSerializer(AbstractAttrValueSerializer):
    class Meta(AbstractAttrValueSerializer.Meta):
        model = ProductAttributeValuePosInt

class AttrValueIntSerializer(AbstractAttrValueSerializer):
    class Meta(AbstractAttrValueSerializer.Meta):
        model = ProductAttributeValueInt

class AttrValueFloatSerializer(AbstractAttrValueSerializer):
    class Meta(AbstractAttrValueSerializer.Meta):
        model = ProductAttributeValueFloat


class BaseIngredientSerializer(ExtendedModelSerializer, NestedFieldSetupMixin):
    atomicity = _atomicity_fn

    class Meta(ExtendedModelSerializer.Meta):
        nested_fields = ['attr_val_str', 'attr_val_pos_int', 'attr_val_int', 'attr_val_float']

    def __init__(self, instance=None, data=DRFEmptyData, **kwargs):
        self.exc_rd_fields = kwargs.pop('exc_rd_fields', None)
        self.exc_wr_fields = kwargs.pop('exc_wr_fields', None)
        self.fields['attr_val_str']     = AttrValueStrSerializer(many=True, instance=instance, data=data)
        self.fields['attr_val_pos_int'] = AttrValuePosIntSerializer(many=True, instance=instance, data=data)
        self.fields['attr_val_int']     = AttrValueIntSerializer(many=True, instance=instance, data=data)
        self.fields['attr_val_float']   = AttrValueFloatSerializer(many=True, instance=instance, data=data)
        super().__init__(instance=instance, data=data, **kwargs)

    @property
    def presentable_fields_name(self):
        out = super().presentable_fields_name
        out = copy.copy(out)
        if src_field_label['_list'] in out:
            out.remove(src_field_label['_list'])
            out.extend(self.Meta.nested_fields)
        return out

    def to_representation(self, instance):
        out = super().to_representation(instance=instance)
        presentable_fields_name = super().presentable_fields_name
        if src_field_label['_list'] in  presentable_fields_name:
            gather_all_attrs = []
            for s_name in self.Meta.nested_fields:
                attrs = out.pop(s_name, [])
                gather_all_attrs.extend(attrs)
            out[src_field_label['_list']] = gather_all_attrs
        return out

    def extra_setup_before_validation(self, instance, data):
        for s_name in self.Meta.nested_fields:
            self.fields[s_name].extract(data=data, dst_field_name=s_name)
            self._setup_subform_instance(name=s_name, instance=instance, data=data, pk_field_name='id')
            self.fields[s_name].append_ingredient(name=s_name, instance=instance, data=data)
        unclassified_attributes = data.get(src_field_label['_list'], [])
        if any(unclassified_attributes):
            err_msg = 'request body contains unclassified attributes %s' % unclassified_attributes
            err_detail = {src_field_label['_list']: [RestErrorDetail(err_msg)] }
            raise RestValidationError(detail=err_detail)

    def run_validation(self, data=DRFEmptyData):
        try:
            validated_data = super().run_validation(data=data)
        except RestValidationError as ve:
            errs_attr = []
            for s_name in self.Meta.nested_fields:
                err_attr = ve.detail.pop(s_name, [])
                errs_attr.extend(err_attr)
            if any(errs_attr):
                ve.detail[src_field_label['_list']] = errs_attr
            raise
        return validated_data


    @atomicity()
    def create(self, validated_data):
        validated_subform_data = {  }
        for k in self.Meta.nested_fields:
            v = validated_data.pop(k, None)
            if v:
                validated_subform_data[k] = v
        instance = super().create(validated_data=validated_data)
        for k,v in validated_subform_data.items() :
            self.fields[k].create(validated_data=v, ingredient=instance)
        return instance

    def update(self, instance, validated_data):
        # remind: parent list serializer will set atomic transaction, no need to set it at here
        validated_subform_data = {k: validated_data.pop(k, []) for k in self.Meta.nested_fields}
        instance = super().update(instance=instance, validated_data=validated_data)
        for k in self.Meta.nested_fields:
            subform_qset = getattr(instance, k).all()
            self.fields[k].update(instance=subform_qset, validated_data=validated_subform_data[k],
                    ingredient=instance, allow_insert=True, allow_delete=True)
        #raise  IntegrityError
        return instance


