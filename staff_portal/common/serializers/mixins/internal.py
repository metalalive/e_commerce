import logging

from django.core.exceptions     import ValidationError
from rest_framework.exceptions  import ValidationError as RestValidationError
from rest_framework.serializers import BaseSerializer, ListSerializer
from rest_framework.fields      import empty


_logger = logging.getLogger(__name__)

class SerializerExcludeFieldsMixin:
    """
    exclude fields at instance level
    Input:
        One query parameter, for read operation through URI request :
            * `fields` , list of field names allowed to fetch.
        extra object parameters may be given to exclude field names
            * `exc_rd_fields`, finely control list of field names to exclude when
               API caller requests data to read.
            * `exc_wr_fields`, similar to `exc_rd_fields` but for write operations.
    """
    DELIMITOR = '__'

    # TODO: figure out how to exclude fields before they're instantiated
    def _common_exclude_fields(self, allowed_fields, exc_fd_name):
        if hasattr(self,exc_fd_name) and getattr(self, exc_fd_name):
            log_msg = ['srlz_cls', type(self).__qualname__]
            for fd in getattr(self, exc_fd_name):
                hier_names = fd.split(self.DELIMITOR)
                if len(hier_names) == 1:
                    allowed_fields = list( set(allowed_fields) - set([fd]) )
                    log_msg += ['fd', fd]
                else:
                    _field = self.fields.get(hier_names[0], None)
                    if _field is None or not isinstance(_field, BaseSerializer):
                        continue
                    _field = _field.child if isinstance(_field, ListSerializer) else _field
                    if not hasattr(_field, exc_fd_name) or not getattr(_field, exc_fd_name):
                        setattr(_field, exc_fd_name, [])
                    getattr(_field, exc_fd_name).append( self.DELIMITOR.join(hier_names[1:]) )
            _logger.debug(None, *log_msg)
        existing = set(self.fields.keys())
        allowed  = set(allowed_fields)
        for field_name in (existing - allowed):
            self.fields.pop(field_name)

    @property
    def presentable_fields_name(self):
        if not hasattr(self, '_presentable_fields_name'):
            req = self.context.get('request', None)
            if req:
                allowed_fields = req.query_params.get('fields', None)
                if allowed_fields:
                    allowed_fields = allowed_fields.split(',')
                    allowed_fields = map(str.strip , allowed_fields) # trim whitespace
                    allowed_fields = list(set(allowed_fields))
                else:
                    allowed_fields = []
            else:
                allowed_fields = []
            self._presentable_fields_name = allowed_fields
        return self._presentable_fields_name

    def exclude_read_fields(self):
        # note these functions needs to be executed only once during instance life cycle
        if hasattr(self, '_exclude_read_fields_done'):
            return # to prevent unessesary recursive calls
        allowed_fields = self.presentable_fields_name
        if not allowed_fields:
            return
        self._common_exclude_fields(allowed_fields=allowed_fields, exc_fd_name='exc_rd_fields')
        setattr(self, '_exclude_read_fields_done', True)


    def exclude_write_fields(self):
        if hasattr(self, '_exclude_write_fields_done'):
            return # to prevent unessesary recursive calls
        setattr(self, '_exclude_write_fields_done', True)
        allowed_fields = list(self.fields.keys())
        self._common_exclude_fields(allowed_fields=allowed_fields, exc_fd_name='exc_wr_fields')

#### end of SerializerExcludeFieldsMixin


class ValidationErrorCallbackMixin:
    def run_validation(self, data=empty):
        try:
            value = super().run_validation(data=data)
        except (ValidationError, RestValidationError) as exc:
            target = self.child if isinstance(self, ListSerializer) else self
            if hasattr(target,'_validation_error_callback') and target._validation_error_callback:
                target._validation_error_callback(exception=exc)
            raise RestValidationError(detail=exc.detail)
        return value

class AugmentEditFieldsMixin:
    """
    Internal mixin class for augmenting extra fields required in the model before writing
    validated data  to backend database.
    This mixins is used to override create() or update() methods of a list serializer
    """
    _field_name_map = {}

    def create(self, validated_data, **kwargs):
        for item in validated_data: # process list of data that will be written
            for k,v in self._field_name_map.items():
                if kwargs.get(k):
                    item[v] = kwargs.get(k)
        return super().create(validated_data=validated_data)

    #### def update(self, instance, validated_data, usr, allow_insert=False, allow_delete=False):
    def update(self, instance, validated_data, allow_insert=False, allow_delete=False, **kwargs):
        for item in validated_data: # process list of data that will be written
            if item.get(self.child.pk_field_name, None) is None: # primary key may not be named `id`
                for k,v in self._field_name_map.items():
                    if kwargs.get(k):
                        item[v] = kwargs.get(k)
                #### item['_user_instance'] = usr
        return super().update(instance=instance, validated_data=validated_data,
                allow_insert=allow_insert, allow_delete=allow_delete)


