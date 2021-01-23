import logging
from collections.abc import Iterable

from django.db        import  IntegrityError, transaction
from django.db.models import  QuerySet
from django.core.exceptions  import ObjectDoesNotExist

from rest_framework.serializers import ModelSerializer,  ListSerializer, IntegerField
from rest_framework.exceptions  import ValidationError as RestValidationError, ErrorDetail as RestErrorDetail
from rest_framework.fields      import empty
from rest_framework.settings    import api_settings

from  .mixins       import  ValidationErrorCallbackMixin, SerializerExcludeFieldsMixin
from ..validators   import  EditFormObjIdValidator
from softdelete.models import SoftDeleteObjectMixin

__all__ = ['BulkUpdateListSerializer', 'ExtendedModelSerializer']

_logger = logging.getLogger(__name__)


class BulkUpdateListSerializer(ValidationErrorCallbackMixin, ListSerializer):
    """
    * provide optional callback function on validation error
    * override update() that is not support in ListSerializer,
    """
    class Meta:
        pass

    def __init__(self, instance=None, data=empty, **kwargs):
        """
        Note that current version of ListSerializer does NOT fetch validators from
        its own metaclass (while Serializer class does so from self.Meta.validators),
        if your applications require validators running at ListSerializer level,
        you could explicitly insert validators to class variable `default_validators`
        , or do it in init function for more complicated validation case
        """
        super().__init__(instance=instance, data=data, **kwargs)
        if hasattr(self, 'initial_data') and self.instance: # validate only in bulk-update case
            self.validators.append(EditFormObjIdValidator())

    def update(self, instance, validated_data, allow_insert=False, allow_delete=False):
        assert isinstance(instance, Iterable) , ("the `instance` argument in BulkUpdateListSerializer \
                update() has to be Iterable e.g. list of model instances, QuerySet ...etc, \
                instead of single model instance")
        pk_field_name = self.child.pk_field_name
        instance_map = {m.pk: m for m in instance}
        data_map  = {item[pk_field_name] :item  for item in validated_data \
                if item.get(pk_field_name, None)}
        log_msg = []
        if any(instance_map):
            log_msg += ['instance_map_keys', list(instance_map.keys()),]
        if any(data_map):
            log_msg += ['data_map_keys', list(data_map.keys()),]
        srlz_cls_parent = '%s.%s' % (type(self).__module__, type(self).__qualname__)
        srlz_cls_child  = '%s.%s' % (type(self.child).__module__, type(self.child).__qualname__)
        ret = []
        try:
            with transaction.atomic():
                for pk, d in data_map.items():
                    obj = instance_map.pop(pk, None)
                    ret.append(self.child.update(obj, d))
                if allow_delete and any(instance_map): # check whether caller allows extra removal(s)
                    for m in instance_map.values():
                        del_kwargs = {}
                        if isinstance(m, SoftDeleteObjectMixin):
                            del_kwargs['hard'] = True  # apply hard delete if it is soft-delete model
                        m.delete(**del_kwargs)
                if allow_insert:  # check whether caller allows extra insertion(s)
                    data_map  = [item for item in validated_data if not item.get(pk_field_name, None)]
                    new_added_list = []
                    for d in data_map:
                        ret.append(self.child.create(d))
                        _pk = getattr(ret[-1], pk_field_name)
                        new_added_list.append( _pk )
                    if any(new_added_list):
                        log_msg += ['new_added_list', new_added_list]
            if any(log_msg):
                log_msg += ['srlz_cls_parent', srlz_cls_parent, 'srlz_cls_child', srlz_cls_child]
                _logger.debug(None, *log_msg)
        except Exception as e: # IntegrityError
            log_msg += ['srlz_cls_parent', srlz_cls_parent, 'srlz_cls_child', srlz_cls_child]
            log_msg += ['excpt_type', type(e).__qualname__, 'excpt_msg', e]
            _logger.error(None, *log_msg)
            raise
        return ret

#### end of BulkUpdateListSerializer


class ExtendedModelSerializer(ModelSerializer, SerializerExcludeFieldsMixin):
    class Meta:
        list_serializer_class = BulkUpdateListSerializer
        validate_only_field_names = []

    def __init__(self, instance=None, data=empty, account=None, **kwargs):
        """
        the init() will do extra work :
            * specify pk field name, which could be `pk`, `id`, `ID`, determined by caller
            * pk field is required in validated_data after validation and before bulk update,
              however the field is dropped if it's AutoField or not editable at model level,
              to avoid this in bulk update, pk field will be editable temporarily by
              instantiating IntegerField
        """
        self.pk_field_name = kwargs.pop('pk_field_name', 'id')
        if (not data is empty) and instance and isinstance(instance, QuerySet):
            # TODO, change field class type of primary key, until post/put request receipt
            self.fields[self.pk_field_name]  = IntegerField()
        self._validate_only_fields = {}
        self._validation_error_callback = kwargs.pop('_validation_error_callback', None)
        self._account = account
        super().__init__(instance=instance, data=data, **kwargs)

    def run_validation(self, data=empty, pk_condition=None, set_null_if_obj_not_found=False):
        """
        change self.instance with respect to the pk from validating data if this serializer has parent
        (ListSerializer or its subclasses), the workaround comes from issue #6130 in Django-Rest-Framework
        """
        if hasattr(self,'parent') and self.parent and isinstance(self.parent, BulkUpdateListSerializer) \
                and isinstance(self.parent.instance, QuerySet):
            if not pk_condition:
                pk_condition = {self.pk_field_name: data.get(self.pk_field_name, '')}
            try:
                self.instance = self.parent.instance.get(**pk_condition)
            except (ObjectDoesNotExist,ValueError) as e:
                srlz_cls_parent = '%s.%s' % (type(self.parent).__module__, type(self.parent).__qualname__)
                srlz_cls_child  = '%s.%s' % (type(self).__module__, type(self).__qualname__)
                fully_qualified_cls_name = '%s.%s' % (type(e).__module__, type(e).__qualname__)
                log_msg = ['pk_condition', pk_condition, 'excpt_msg', e, 'excpt_type', fully_qualified_cls_name,
                        'srlz_cls_parent', srlz_cls_parent, 'srlz_cls_child', srlz_cls_child, 'request_data', data, ]
                if set_null_if_obj_not_found:
                    _logger.info(None, *log_msg)
                    self.instance = None
                else:
                    _logger.warning(None, *log_msg)
                    errmsg = "pk ({}) in request data cannot be mapped to existing instance, reason: {}"
                    errmsg = errmsg.format(str(pk_condition), str(e))
                    _detail = {api_settings.NON_FIELD_ERRORS_KEY : RestErrorDetail(errmsg)}
                    raise RestValidationError( detail=_detail )
        if hasattr(self.Meta, 'validate_only_field_names'):
            _validate_only_fields = {k: data.get(k,'') for k in self.Meta.validate_only_field_names}
            self._validate_only_fields.update(_validate_only_fields)
        self.extra_setup_before_validation(instance=self.instance, data=data)
        value = super().run_validation(data=data)
        return value

    def extra_setup_before_validation(self, instance, data):
        pass

    @property
    def _readable_fields(self): # override same method in parent Serializer
        self.exclude_read_fields()
        out = super()._readable_fields
        return out

    @property
    def _writable_fields(self):
        self.exclude_write_fields()
        out = super()._writable_fields
        return out

#### end of ExtendedModelSerializer


