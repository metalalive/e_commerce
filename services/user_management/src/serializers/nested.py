import logging

from django.core.validators import MaxValueValidator, MinValueValidator
from django.db.models.constants import LOOKUP_SEP
from django.contrib.contenttypes.models import ContentType
from django.utils import timezone as django_timezone
from rest_framework.exceptions import ValidationError as RestValidationError
from rest_framework.fields import empty

from ecommerce_common.serializers import (
    BulkUpdateListSerializer,
    ExtendedModelSerializer,
)
from ecommerce_common.serializers.mixins.internal import AugmentEditFieldsMixin
from ecommerce_common.serializers.validators import UniqueListItemsValidator

from ..models.base import (
    EmailAddress,
    PhoneNumber,
    GeoLocation,
    _atomicity_fn,
    UserQuotaRelation,
    GenericUserAppliedRole,
    GenericUserGroupRelation,
)
from .common import ConnectedProfileField, UserSubformSetupMixin

_logger = logging.getLogger(__name__)
# logstash will duplicate the same log message, figure out how that happenes.


class BaseQuotaCheckerMixin:
    def __init__(self, quota_validator, **kwargs):
        super().__init__(**kwargs)
        self._quota_validator = quota_validator
        self._applied_quota = 0

    @property
    def applied_quota(self):
        return self._applied_quota

    @applied_quota.setter
    def applied_quota(self, value):
        log_msg = [
            "srlz_cls",
            type(self.child).__qualname__,
        ]
        if not isinstance(value, (int, float)):
            err_msg = "Quota value is %s, which is neither integer or float number" % value
            log_msg.extend(["err_msg", err_msg])
            _logger.info(None, *log_msg)
            raise ValueError(err_msg)
        self._applied_quota = value
        self.edit_quota_threshold(quota_validator=self._quota_validator, value=value)
        log_msg.extend(["_applied_quota", self._applied_quota])
        _logger.debug(None, *log_msg)

    def edit_quota_threshold(self, quota_validator, value):
        raise NotImplementedError()


class QuotaValidator(MaxValueValidator):
    def __call__(self, value):
        item_len = len(value)
        log_msg = ["item_len", item_len, "limit_value", self.limit_value]
        _logger.debug(None, *log_msg)
        super().__call__(item_len)


class AugmentUserRefMixin(AugmentEditFieldsMixin):
    _field_name_map = {
        "usr": "_user_instance",
    }


class QuotaCheckerSerializer(AugmentUserRefMixin, BaseQuotaCheckerMixin, BulkUpdateListSerializer):
    # ensure method resolution order meets application requirement : __mro__
    def __init__(self, *args, **kwargs):
        errmsg = "you haven't configured quota for the item"
        quota_validator = QuotaValidator(limit_value=0, message=errmsg)
        super().__init__(*args, quota_validator=quota_validator, **kwargs)
        self.validators.append(quota_validator)

    def edit_quota_threshold(self, quota_validator, value):
        errmsg = "number of items provided exceeds the limit: {q0}"
        errmsg = errmsg.format(q0=str(value))
        self._quota_validator.message = errmsg
        self._quota_validator.limit_value = value


class BulkUserQuotaRelationSerializer(AugmentUserRefMixin, BulkUpdateListSerializer):
    """
    * It should be safe to override default validator list, since this class
      shouldn't be overriden by any other class.
    * UniqueListItemsValidator is applied instead of UniqueTogetherValidator
      for checking uniqueness of usage type in each quota arrangement form,
      because UniqueListItemsValidator can do things quickly without database
      connection.
      (side note: it doesn't make sense to have duplicate usage type for a user,
      e.g. have both usage-type-email=2 and usage-type-email=9 in one client request)
    """

    default_validators = [
        UniqueListItemsValidator(fields=["material"]),
    ]

    def _retrieve_material_ids(self, data):
        def _fn(d):
            return d[self.pk_field_name]

        ids = map(_fn, filter(_fn, data))
        conditions = {LOOKUP_SEP.join([self.pk_field_name, "in"]): ids}
        qset = self._current_quota_applied.filter(**conditions)
        qset = qset.values_list(self.pk_field_name, flat=True)
        return qset

    def _update_data_map(self, data):
        mat_ids = self._retrieve_material_ids(data)
        out = {
            item[self.pk_field_name]: item
            for item in data
            if item[self.pk_field_name].id in mat_ids
        }
        return out

    def _insert_data_map(self, data):
        mat_ids = self._retrieve_material_ids(data)
        return [item for item in data if item[self.pk_field_name].id not in mat_ids]

    def update(self, instance, validated_data, allow_insert=False, allow_delete=False, **kwargs):
        self._current_quota_applied = instance
        instance = super().update(
            instance=self._current_quota_applied,
            validated_data=validated_data,
            allow_insert=allow_insert,
            allow_delete=allow_delete,
            **kwargs,
        )
        return instance


class CommonUserSubformSerializer(ExtendedModelSerializer, UserSubformSetupMixin):
    atomicity = _atomicity_fn

    @property
    def presentable_fields_name(self):
        return self.Meta.fields

    def run_validation(self, data=empty):
        value = super().run_validation(data=data, set_null_if_obj_not_found=True)
        return value

    def create(self, validated_data):
        # In this application, this may be invoked by GenericUserGroupSerializer
        # or GenericUserProfileSerializer
        usr = validated_data.pop("_user_instance", None)
        validated_data["user_type"] = ContentType.objects.get_for_model(usr)
        validated_data["user_id"] = usr.pk
        log_msg = [
            "srlz_cls",
            type(self).__qualname__,
            "validated_data",
            validated_data,
        ]
        _logger.debug(None, *log_msg)
        instance = super().create(validated_data=validated_data)
        return instance


#### end of  CommonUserSubformSerializer


class EmailSerializer(CommonUserSubformSerializer):
    class Meta(CommonUserSubformSerializer.Meta):
        model = EmailAddress
        fields = ["id", "addr"]
        list_serializer_class = QuotaCheckerSerializer

    def extra_setup_before_validation(self, instance, data):
        self._mark_as_creation_on_update(pk_field_name="id", instance=instance, data=data)


class PhoneNumberSerializer(CommonUserSubformSerializer):
    class Meta(CommonUserSubformSerializer.Meta):
        model = PhoneNumber
        # fmt: off
        fields = ["id", "country_code", "line_number"]
        # fmt: on
        list_serializer_class = QuotaCheckerSerializer

    def extra_setup_before_validation(self, instance, data):
        self._mark_as_creation_on_update(pk_field_name="id", instance=instance, data=data)


class GeoLocationSerializer(CommonUserSubformSerializer):
    class Meta(CommonUserSubformSerializer.Meta):
        model = GeoLocation
        # fmt: off
        fields = ["id", "country", "province", "locality", "street", "detail", "floor", "description"]
        # fmt: on
        list_serializer_class = QuotaCheckerSerializer

    def extra_setup_before_validation(self, instance, data):
        self._mark_as_creation_on_update(pk_field_name="id", instance=instance, data=data)


class UserQuotaRelationSerializer(CommonUserSubformSerializer):
    class Meta(CommonUserSubformSerializer.Meta):
        model = UserQuotaRelation
        fields = ["material", "maxnum", "expiry"]
        read_only_fields = []
        list_serializer_class = BulkUserQuotaRelationSerializer

    def __init__(self, *args, data=empty, **kwargs):
        # DO NOT pass client form data on initialization, to avoid pk field from converting
        # to IntegerField at ExtendedModelSerializer , also avoid the list serializer
        # (BulkUpdateListSerializer) from adding EditFormObjIdValidator
        super().__init__(*args, pk_field_name="material", **kwargs)
        exp_validator = MinValueValidator(limit_value=django_timezone.now())
        self.fields["expiry"].validators.append(exp_validator)

    def update(self, instance, validated_data):
        # fmt: off
        log_msg = [
            "srlz_cls", type(self).__qualname__, "instance_id", instance.pk,
            "new_material", validated_data["material"], "new_maxnum", validated_data["maxnum"],
            "old_maxnum", instance.maxnum,
        ]
        # fmt: on
        old_expiry = instance.expiry
        new_expiry = validated_data["expiry"]
        if old_expiry:
            old_expiry = old_expiry.isoformat()
        if new_expiry:
            new_expiry = new_expiry.isoformat()
        log_msg.extend(["old_expiry", old_expiry, "new_expiry", new_expiry])
        try:
            assert instance.material == validated_data["material"], "material does not match"
            instance = super().update(instance=instance, validated_data=validated_data)
        except AssertionError:
            _logger.error(None, *log_msg)
            raise
        else:
            _logger.debug(None, *log_msg)
        return instance


## end of class UserQuotaRelationSerializer


class _BulkUserPriviledgeAssigner(AugmentUserRefMixin, BulkUpdateListSerializer):

    def _retrieve_priv_ids(self, data):
        def _fn(d):
            return d[self.pk_field_name]

        ids = map(_fn, filter(_fn, data))
        conditions = {LOOKUP_SEP.join([self.pk_field_name, "in"]): ids}
        qset = self._current_priv_set_applied.filter(**conditions)
        qset = qset.values_list(self.pk_field_name, flat=True)
        return qset

    def _update_data_map(self, data):
        # the model of this serializer has compound key which consists 2 referential fields that always exists.
        # the same function at parent class determines whether an input data item goes to insertion map
        # or update map by checking whether a pk field is null. Therefore I cnanot reply on the same function
        # at parent class and instead overwrite the same function at here
        priv_ids = self._retrieve_priv_ids(data)
        out = {
            item[self.pk_field_name]: item
            for item in data
            if item[self.pk_field_name].id in priv_ids
        }
        return out

    def _insert_data_map(self, data):
        priv_ids = self._retrieve_priv_ids(data)
        return [item for item in data if item[self.pk_field_name].id not in priv_ids]

    def update(self, instance, validated_data, allow_insert=False, allow_delete=False, **kwargs):
        self._current_priv_set_applied = instance
        log_msg = [
            "srlz_cls",
            type(self.child).__qualname__,
            "instance",
            instance,
            "validated_data",
            validated_data,
        ]
        _logger.debug(None, *log_msg)
        instance = super().update(
            instance=self._current_priv_set_applied,
            validated_data=validated_data,
            allow_insert=allow_insert,
            allow_delete=allow_delete,
            **kwargs,
        )
        return instance


class _BaseUserPriviledgeAssigner(ExtendedModelSerializer, UserSubformSetupMixin):
    atomicity = _atomicity_fn

    class Meta(ExtendedModelSerializer.Meta):
        # subclasses must orverride these fields
        _apply_type = None
        model = None
        list_serializer_class = _BulkUserPriviledgeAssigner

    def __init__(self, *args, data=empty, **kwargs):
        # DO NOT pass client form data on initialization, to avoid pk field from
        # converting to IntegerField at ExtendedModelSerializer
        super().__init__(*args, pk_field_name=self.Meta._apply_type, **kwargs)

    def run_validation(self, data=empty):
        return super().run_validation(data=data, set_null_if_obj_not_found=True)

    @property
    def presentable_fields_name(self):
        return self.Meta.fields

    def to_representation(self, instance):
        if self.fields.get("approved_by"):
            if not isinstance(self.fields["approved_by"], ConnectedProfileField):
                self.fields["approved_by"] = ConnectedProfileField(many=False)
            self.fields["approved_by"].instance = instance.approved_by
        out = super().to_representation(instance=instance)
        return out


class GenericUserRoleBulkAssigner(_BulkUserPriviledgeAssigner):
    default_validators = [
        UniqueListItemsValidator(fields=["role"]),
    ]


class GenericUserGroupRelBulkAssigner(_BulkUserPriviledgeAssigner):
    default_validators = [
        UniqueListItemsValidator(fields=["group"]),
    ]


class RoleAssignValidator:
    requires_context = True
    err_msg_pattern = "Role is NOT assigned to current login user: %s"

    @property
    def profile(self):
        return getattr(self, "_profile", None)

    @profile.setter
    def profile(self, value):
        setattr(self, "_profile", value)

    def __call__(self, value, caller):
        if self.profile.privilege_status == type(self.profile).SUPERUSER:
            return
        roles_available = self.profile.all_roles
        role_id = getattr(value, "id")
        role_exist_direct = roles_available["direct"].filter(id=role_id).exists()
        role_exist_inherit = roles_available["inherit"].filter(id=role_id).exists()
        if not (role_exist_direct or role_exist_inherit):
            err_msg = self.err_msg_pattern % (value.id)
            raise RestValidationError(err_msg)


class GroupAssignValidator:
    requires_context = True
    err_msg_pattern = "Current login user does NOT belong to this group : %s"

    @property
    def profile(self):
        return getattr(self, "_profile", None)

    @profile.setter
    def profile(self, value):
        setattr(self, "_profile", value)

    def __call__(self, value, caller):
        if self.profile.privilege_status == type(self.profile).SUPERUSER:
            return
        grp_id = value.id
        field_name = LOOKUP_SEP.join(["group", "descendants", "descendant", "id"])
        qset = self.profile.groups.filter(**{field_name: grp_id})
        # qset = qset.values_list(field_name, flat=True)
        grp_exist = qset.exists()
        if not grp_exist:
            err_msg = self.err_msg_pattern % (value.id)
            raise RestValidationError(err_msg)


class GenericUserRoleAssigner(_BaseUserPriviledgeAssigner):
    class Meta(_BaseUserPriviledgeAssigner.Meta):
        _apply_type = "role"
        model = GenericUserAppliedRole
        fields = [_apply_type, "expiry", "approved_by"]
        read_only_fields = ["approved_by"]
        list_serializer_class = GenericUserRoleBulkAssigner

    def __init__(self, *args, **kwargs):
        super().__init__(*args, **kwargs)
        role_id_validator = RoleAssignValidator()
        exp_validator = MinValueValidator(limit_value=django_timezone.now())
        self.fields["role"].validators.append(role_id_validator)
        self.fields["expiry"].validators.append(exp_validator)
        self._role_id_validator = role_id_validator

    def run_validation(self, data=empty):
        self._role_id_validator.profile = self._account.profile
        return super().run_validation(data=data)

    def create(self, validated_data):
        target = validated_data.pop("_user_instance", None)
        validated_data["user_type"] = ContentType.objects.get_for_model(target)
        validated_data["user_id"] = target.pk
        validated_data["approved_by"] = self._account.profile
        log_msg = ["validated_data", validated_data]
        _logger.debug(None, *log_msg)
        instance = super().create(validated_data=validated_data)
        return instance

    def update(self, instance, validated_data):
        log_msg = []
        # fmt: off
        if instance.approved_by != self._account.profile:
            log_msg.extend(
                ["profile_before_edit", instance.approved_by.id, "profile_after_edit", self._account.profile.id]
            )
        # fmt: on
        old_role = getattr(instance, self.Meta._apply_type)
        new_role = validated_data[self.Meta._apply_type]
        log_msg.extend(["old_role", old_role, "new_role", new_role])
        if old_role == new_role:
            if validated_data["expiry"] != instance.expiry:
                validated_data["approved_by"] = self._account.profile
                instance = super().update(instance=instance, validated_data=validated_data)
            _logger.debug(None, *log_msg)
        else:
            _logger.error(None, *log_msg)
        return instance


class GenericUserGroupRelationAssigner(_BaseUserPriviledgeAssigner):
    class Meta(_BaseUserPriviledgeAssigner.Meta):
        _apply_type = "group"
        model = GenericUserGroupRelation
        fields = [_apply_type, "approved_by"]
        read_only_fields = ["approved_by"]
        list_serializer_class = GenericUserGroupRelBulkAssigner

    def __init__(self, *args, **kwargs):
        super().__init__(*args, **kwargs)
        grp_id_validator = GroupAssignValidator()
        self.fields["group"].validators.append(grp_id_validator)
        self._grp_id_validator = grp_id_validator

    def run_validation(self, data=empty):
        self._grp_id_validator.profile = self._account.profile
        return super().run_validation(data=data)

    def create(self, validated_data):
        target = validated_data.pop("_user_instance", None)
        validated_data["profile"] = target
        validated_data["approved_by"] = self._account.profile
        log_msg = ["validated_data", validated_data]
        _logger.debug(None, *log_msg)
        instance = super().create(validated_data=validated_data)
        return instance

    def update(self, instance, validated_data):
        log_msg = []
        try:
            d_type = validated_data[self.Meta._apply_type]
            log_msg.extend(["apply_type", d_type])
            assert getattr(instance, self.Meta._apply_type) == d_type
            # content will be the same , no need to update
        except (ValueError, KeyError, AssertionError) as e:
            log_msg.extend(["excpt_msg", e])
            _logger.error(None, *log_msg)
        return instance
