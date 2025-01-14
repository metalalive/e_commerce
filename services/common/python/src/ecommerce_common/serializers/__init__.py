import logging
from collections.abc import Iterable

from django.db.models import Model as DjangoModel, QuerySet
from django.core.exceptions import ObjectDoesNotExist

from rest_framework.serializers import ModelSerializer, ListSerializer, IntegerField
from rest_framework.exceptions import (
    ValidationError as RestValidationError,
    ErrorDetail as RestErrorDetail,
)
from rest_framework.fields import empty
from rest_framework.settings import api_settings

from .mixins import (
    ValidationErrorCallbackMixin,
    SerializerExcludeFieldsMixin,
    ClosureTableMixin,
)
from .validators import EditFormObjIdValidator
from softdelete.models import SoftDeleteObjectMixin

__all__ = ["BulkUpdateListSerializer", "ExtendedModelSerializer"]

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
        if (
            hasattr(self, "initial_data") and self.instance
        ):  # validate only in bulk-update case
            self.validators.append(EditFormObjIdValidator())
        self.pk_field_name = self.child.pk_field_name

    @property
    def instance_ids(self):
        ids = self.instance.values_list(self.pk_field_name, flat=True)
        return list(ids)

    def extract_form_ids(self, formdata, include_null=True):
        ids = [
            item[self.pk_field_name]
            for item in formdata
            if include_null or item.get(self.pk_field_name, None)
        ]
        return ids

    def _update_instance_map(self, instance_set):
        instance_map = {getattr(m, self.pk_field_name): m for m in instance_set}
        return instance_map

    def _update_data_map(self, data):
        # construct data map for updates in bulk update operation, by doing shallow copy of
        # input form item whose ID exists
        return {
            item[self.pk_field_name]: item
            for item in data
            if item.get(self.pk_field_name, None) is not None
        }

    def _insert_data_map(self, data):
        # construct data map for insertions in bulk update operation, by doing shallow copy of
        # input form item whose ID doesn't exist
        return [item for item in data if item.get(self.pk_field_name, None) is None]

    def update(self, instance, validated_data, allow_insert=False, allow_delete=False):
        assert isinstance(
            instance, Iterable
        ), "the `instance` argument in BulkUpdateListSerializer \
                update() has to be Iterable e.g. list of model instances, QuerySet ...etc, \
                instead of single model instance"
        instance_map = self._update_instance_map(instance)
        data_map = self._update_data_map(data=validated_data)
        log_msg = []
        if any(instance_map):
            log_msg += [
                "instance_map_keys",
                list(instance_map.keys()),
            ]
        if any(data_map):
            log_msg += [
                "data_map_keys",
                list(data_map.keys()),
            ]
        srlz_cls_parent = "%s.%s" % (type(self).__module__, type(self).__qualname__)
        srlz_cls_child = "%s.%s" % (
            type(self.child).__module__,
            type(self.child).__qualname__,
        )
        ret = []
        try:
            with self.child.atomicity():
                for pk, d in data_map.items():
                    obj = instance_map.pop(pk, None)
                    ret.append(self.child.update(obj, d))
                if allow_delete and any(
                    instance_map
                ):  # check whether caller allows extra removal(s)
                    for m in instance_map.values():
                        del_kwargs = {}
                        if isinstance(m, SoftDeleteObjectMixin):
                            del_kwargs["hard"] = (
                                True  # apply hard delete if it is soft-delete model
                            )
                        m.delete(**del_kwargs)
                if allow_insert:  # check whether caller allows extra insertion(s)
                    pk_field_name = self.child.pk_field_name
                    data_map = self._insert_data_map(data=validated_data)
                    new_added_list = []
                    for d in data_map:
                        ret.append(self.child.create(d))
                        _pk = getattr(ret[-1], pk_field_name)
                        new_added_list.append(_pk)
                    if any(new_added_list):
                        log_msg += ["new_added_list", new_added_list]
            if any(log_msg):
                log_msg += [
                    "srlz_cls_parent",
                    srlz_cls_parent,
                    "srlz_cls_child",
                    srlz_cls_child,
                ]
                _logger.debug(None, *log_msg)
        except Exception as e:  # including django.db.IntegrityError
            log_msg += [
                "srlz_cls_parent",
                srlz_cls_parent,
                "srlz_cls_child",
                srlz_cls_child,
            ]
            log_msg += ["excpt_type", type(e).__qualname__, "excpt_msg", e]
            _logger.error(None, *log_msg)
            raise
        return ret

    def create(self, *args, **kwargs):
        log_msg = []
        srlz_cls_parent = "%s.%s" % (type(self).__module__, type(self).__qualname__)
        srlz_cls_child = "%s.%s" % (
            type(self.child).__module__,
            type(self.child).__qualname__,
        )
        try:
            with self.child.atomicity():
                ret = super().create(*args, **kwargs)
        except Exception as e:  # including django.db.IntegrityError
            log_msg += [
                "srlz_cls_parent",
                srlz_cls_parent,
                "srlz_cls_child",
                srlz_cls_child,
            ]
            log_msg += ["excpt_type", type(e).__qualname__, "excpt_msg", e]
            _logger.error(None, *log_msg)
            raise
        return ret


#### end of BulkUpdateListSerializer


class ExtendedModelSerializer(ModelSerializer, SerializerExcludeFieldsMixin):
    # callable object to guarantee atomicity of dataabse  transaction
    atomicity = None

    class Meta:
        list_serializer_class = BulkUpdateListSerializer
        validate_only_field_names = []

    def __init__(
        self, instance=None, data=empty, account=None, pk_field_name="id", **kwargs
    ):
        """
        the init() will do extra work :
            * specify pk field name, which could be `pk`, `id`, `ID`, determined by caller
            * pk field is required in validated_data after validation and before bulk update,
              however the field is dropped if it's AutoField or not editable at model level,
              to avoid this in bulk update, pk field will be editable temporarily by
              instantiating IntegerField
        """
        self.pk_field_name = pk_field_name
        if (
            (data is not empty)
            and instance
            and isinstance(instance, (DjangoModel, QuerySet))
        ):
            # TODO, change the class of primary key field until validation starts (post/put request received)
            self.fields[self.pk_field_name] = IntegerField()
        self._validate_only_fields = {}
        self._validation_error_callback = kwargs.pop("_validation_error_callback", None)
        self._account = account
        super().__init__(instance=instance, data=data, **kwargs)

    def run_validation(
        self, data=empty, pk_condition=None, set_null_if_obj_not_found=False
    ):
        """
        change self.instance with respect to the pk from validating data if this serializer has parent
        (ListSerializer or its subclasses), the workaround comes from issue #6130 in Django-Rest-Framework
        """
        if (
            hasattr(self, "parent")
            and self.parent
            and isinstance(self.parent, BulkUpdateListSerializer)
            and isinstance(self.parent.instance, QuerySet)
        ):
            if not pk_condition:
                pk_condition = {self.pk_field_name: data.get(self.pk_field_name, "")}
            try:
                self.instance = self.parent.instance.get(**pk_condition)
            except (ObjectDoesNotExist, ValueError) as e:
                srlz_cls_parent = "%s.%s" % (
                    type(self.parent).__module__,
                    type(self.parent).__qualname__,
                )
                srlz_cls_child = "%s.%s" % (
                    type(self).__module__,
                    type(self).__qualname__,
                )
                fully_qualified_cls_name = "%s.%s" % (
                    type(e).__module__,
                    type(e).__qualname__,
                )
                log_msg = [
                    "pk_condition",
                    pk_condition,
                    "excpt_msg",
                    e,
                    "excpt_type",
                    fully_qualified_cls_name,
                    "srlz_cls_parent",
                    srlz_cls_parent,
                    "srlz_cls_child",
                    srlz_cls_child,
                    "request_data",
                    data,
                ]
                if set_null_if_obj_not_found:
                    _logger.info(None, *log_msg)
                    self.instance = None
                else:
                    _logger.warning(None, *log_msg)
                    errmsg = "pk ({}) in request data cannot be mapped to existing instance, reason: {}"
                    errmsg = errmsg.format(str(pk_condition), str(e))
                    _detail = {
                        api_settings.NON_FIELD_ERRORS_KEY: RestErrorDetail(errmsg)
                    }
                    raise RestValidationError(detail=_detail)
        if hasattr(self.Meta, "validate_only_field_names"):
            _validate_only_fields = {
                k: data.get(k, "") for k in self.Meta.validate_only_field_names
            }
            self._validate_only_fields.update(_validate_only_fields)
        self.extra_setup_before_validation(instance=self.instance, data=data)
        value = super().run_validation(data=data)
        return value

    def extra_setup_before_validation(self, instance, data):
        pass

    @property
    def _readable_fields(self):  # override same method in parent Serializer
        self.exclude_read_fields()
        out = super()._readable_fields
        return out

    @property
    def _writable_fields(self):
        self.exclude_write_fields()
        out = super()._writable_fields
        return out


#### end of ExtendedModelSerializer


class DjangoBaseClosureBulkSerializer(BulkUpdateListSerializer, ClosureTableMixin):
    def __init__(self, instance=None, data=empty, **kwargs):
        super().__init__(instance=instance, data=data, **kwargs)
        if hasattr(self, "initial_data"):
            v = self.prepare_cycle_detection_validators(forms=self.initial_data)
            self.validators.append(v)

    @property
    def is_create(self):
        return self.instance is None

    def _get_field_data(self, form, key, default=None, remove_after_read=False):
        if remove_after_read:
            out = form.pop(key, default)
        else:
            out = form.get(key, default)
        return out

    def _set_field_data(self, form, key, val):
        form[key] = val

    def get_node_ID(self, node):
        return node.pk

    def create(self, validated_data):
        validated_data = self.get_sorted_insertion_forms(forms=validated_data)
        with self.child.atomicity():
            instances = super().create(validated_data=validated_data)
            # end of generic group bulk create
        return instances

    def update(self, instance, validated_data):
        validated_data = self.get_sorted_update_forms(
            instances=instance, forms=validated_data
        )
        ret = None
        with self.child.atomicity():
            self.clean_dup_update_paths()
            ret = super().update(instance=instance, validated_data=validated_data)
            # end of group bulk update
        return ret


#### end of  DjangoBaseClosureBulkSerializer
