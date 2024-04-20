import json
import copy
import logging

from django.core.exceptions   import ValidationError as DjangoValidationError
from django.contrib.contenttypes.models  import ContentType
from rest_framework.fields    import empty as DRFEmptyData, FloatField
from rest_framework.exceptions  import ValidationError as RestValidationError, ErrorDetail as RestErrorDetail
from rest_framework.settings import DEFAULTS as drf_default_settings

from ecommerce_common.validators   import  NumberBoundaryValidator, UniqueListItemsValidator
from ecommerce_common.serializers  import  ExtendedModelSerializer, BulkUpdateListSerializer
from ecommerce_common.serializers.mixins  import NestedFieldSetupMixin
from ecommerce_common.serializers.mixins.internal import AugmentEditFieldsMixin
from ecommerce_common.views.error import DRFRequestDataConflictError

from ..models.base import ProductAttributeType, ProductAttributeValueStr, ProductAttributeValuePosInt, ProductAttributeValueInt, ProductAttributeValueFloat, _ProductAttrValueDataType
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
        _label = src_field_label
        fn_gather_type_id = lambda d:d.get(_label['type'])
        valid_attrs = filter(fn_gather_type_id, src)
        attrtype_ids = list(map(fn_gather_type_id, valid_attrs))
        qset = self.child.fields['attr_type'].queryset
        try:
            # NOTE: the result set should NOT be cached since there would be
            # several attribute pairs in bulk ingredients received in one flight
            # if you cache the result here, that will cause logic bug
            qset = qset.filter(pk__in=attrtype_ids)
            return qset
        except ValueError as ve:
            model_cls = qset.model
            id_name = model_cls._meta.pk.name
            value_error_msg_pattern = "Field '%s' expected a number but got %r."
            fn_find_rootcause = lambda d: value_error_msg_pattern % (id_name, d.get('type')) == ve.args[0]
            rootcause = tuple(filter(fn_find_rootcause, src))
            if any(rootcause):
                err_detail = {'data': rootcause, 'code': 'query_invalid_data_type',
                        'model':model_cls.__name__ , 'field': id_name}
                errmsg = {_label['_list']: [json.dumps(err_detail),]}
                raise DjangoValidationError(errmsg)
            else:
                raise

    def extract(self, data, dst_field_name):
        _label = src_field_label
        src = data.get(_label['_list'], [])
        valid_types = self._get_valid_types(src=src)
        valid_types = valid_types.filter(dtype=self.child.Meta.model.DATATYPE.value[0][0])
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

    def validate(self, value, _logger=None, exception_cls=Exception):
        id_required = self.child.fields['id'].required
        if id_required:
            err_msg_pattern = ['{"message":"duplicate ID found in ',
                    self.child.Meta.model.DATATYPE.label.lower(),' attribute","value":%s}']
            err_msg_pattern = ''.join(err_msg_pattern)
            unique_id_checker = UniqueListItemsValidator( fields=['id'],
                    error_cls=DRFRequestDataConflictError, err_msg_pattern=err_msg_pattern)
            unique_id_checker(value=value, caller=self)
        return value
## end of class AttrValueListSerializer


class AbstractAttrValueSerializer(ExtendedModelSerializer, NestedFieldSetupMixin):
    atomicity = _atomicity_fn
    class Meta(ExtendedModelSerializer.Meta):
        fields = ['id', 'ingredient_type', 'ingredient_id', 'attr_type', 'value']
        read_only_fields = ['ingredient_type', 'ingredient_id']
        list_serializer_class = AttrValueListSerializer

    def __init__(self, instance=None, data=DRFEmptyData, **kwargs):
        self.fields['extra_amount'] = FloatField(min_value=0.0)
        self.fields['extra_amount'].validators.append(NumberBoundaryValidator(limit=0.0, larger_than=True, include=False))
        super().__init__(instance=instance, data=data, **kwargs)

    @property
    def presentable_fields_name(self):
        return ['id', 'attr_type', 'value', 'extra_amount']

    def to_representation(self, instance):
        out = super().to_representation(instance=instance)
        stored_attrtype = out.pop('attr_type' , None)
        if not stored_attrtype:
            log_msg = ['srlz_cls', type(self).__qualname__,
                    'msg', 'product attribute type must not be empty in type-value pair',
                    'id', out.get('id',None) , 'value',out.get('value',None) ]
            _logger.error(None, *log_msg)
        out[src_field_label['type']] = stored_attrtype
        if out['extra_amount'] is None:
            out.pop('extra_amount', None)
        return out

    def extra_setup_before_validation(self, instance, data):
        self.fields['extra_amount'].required = data.get('extra_amount', None) is not None
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


class BaseIngredientListSerializer(BulkUpdateListSerializer):
    def validate(self, value, _logger=None, exception_cls=Exception):
        id_required = self.child.fields['id'].required
        if id_required:
            unique_id_checker = UniqueListItemsValidator(fields=['id'], error_cls=DRFRequestDataConflictError)
            unique_id_checker(value=value, caller=self)
        return value


class BaseIngredientSerializer(ExtendedModelSerializer, NestedFieldSetupMixin):
    atomicity = _atomicity_fn

    class Meta(ExtendedModelSerializer.Meta):
        fields = []
        nested_fields = tuple((opt[0][1] for  opt in _ProductAttrValueDataType))
        list_serializer_class = BaseIngredientListSerializer

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
        if (not presentable_fields_name) or (src_field_label['_list'] in  presentable_fields_name):
            gather_all_attrs = []
            for s_name in self.Meta.nested_fields:
                attrs = out.pop(s_name, [])
                gather_all_attrs.extend(attrs)
            out[src_field_label['_list']] = gather_all_attrs
        return out

    def extra_setup_before_validation(self, instance, data):
        for s_name in self.Meta.nested_fields:
            try:
                self.fields[s_name].extract(data=data, dst_field_name=s_name)
            except DjangoValidationError  as e:
                serialized_rootcause_attrs = e.args[0]['attributes'][0]
                rootcause = json.loads(serialized_rootcause_attrs)
                if rootcause['code'] == 'query_invalid_data_type' and rootcause['model'] == \
                        ProductAttributeType.__name__ and rootcause['field'] == 'id':
                    rootcuase_attrs = rootcause['data']
                else:
                    rootcuase_attrs = data.get(src_field_label['_list'], [])
                self._raise_unknown_attr_type_error(rootcuase_attrs)
            self._setup_subform_instance(name=s_name, instance=instance, data=data, pk_field_name='id')
            self.fields[s_name].append_ingredient(name=s_name, instance=instance, data=data)
        unclassified_attributes = data.get(src_field_label['_list'], [])
        if any(unclassified_attributes):
            self._raise_unknown_attr_type_error(unclassified_attributes)


    def _raise_unknown_attr_type_error(self, unclassified_attributes):
        details = []
        for idx in range(self._num_attr_vals):
            item = tuple(filter(lambda x:x.get('_seq_num', -1) == idx, unclassified_attributes))
            detail = {}
            if any(item):
                err_detail = RestErrorDetail('unclassified attribute type `%s`' \
                        % (item[0]['type']), code='unclassified_attributes')
                detail['type'] = [err_detail]
            details.append(detail)
        raise RestValidationError(detail={src_field_label['_list']: details})


    def _err_detail_invalid_attr_value(self, error, data):
        non_field_err_key = drf_default_settings['NON_FIELD_ERRORS_KEY']
        err_fields = []
        err_non_field = []
        for s_name in self.Meta.nested_fields:
            err_attr = error.detail.pop(s_name, [])
            if isinstance(err_attr, dict):
                err_non_field.extend(err_attr[non_field_err_key])
            else: # list
                for idx in range(len(err_attr)):
                    if err_attr[idx]:
                        _info = {'origin':data[s_name][idx], 'errobj':err_attr[idx]}
                        err_fields.append(_info)
        if any(err_fields):
            details = []
            for idx in range(self._num_attr_vals):
                items = tuple(filter(lambda x:x['origin'].get('_seq_num', -1) == idx, err_fields))
                detail = items[0]['errobj'] if any(items) else  {}
                details.append(detail)
            error.detail[src_field_label['_list']] = details
        elif any(err_non_field):
            error.detail[src_field_label['_list']] = {non_field_err_key: err_non_field}


    def _augment_seq_num_attrs(self, data):
        _seq_num = 0
        for d in data.get(src_field_label['_list'], []):
            d['_seq_num'] = _seq_num
            _seq_num += 1
        self._num_attr_vals = _seq_num

    def _remove_seq_num_attrs(self, data):
        attr_field_names = self.Meta.nested_fields + (src_field_label['_list'],)
        for name in attr_field_names:
            for d in data.get(name, []):
                d.pop('_seq_num', None)
        delattr(self, '_num_attr_vals')

    def run_validation(self, data=DRFEmptyData):
        try:
            self._augment_seq_num_attrs(data)
            validated_data = super().run_validation(data=data)
        except RestValidationError as e:
            self._err_detail_invalid_attr_value(error=e, data=data)
            raise
        finally:
            self._remove_seq_num_attrs(data)
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

## end of class BaseIngredientSerializer

