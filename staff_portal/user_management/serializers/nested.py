import logging

from django.core.validators     import MaxValueValidator, MinValueValidator
from django.db.models import  Manager as DjangoModelManager
from django.db.models.constants import LOOKUP_SEP
from django.db.utils  import  IntegrityError
from django.contrib.contenttypes.models  import ContentType
from django.utils import timezone as django_timezone
from rest_framework.exceptions  import  ErrorDetail as DRFErrorDetail, ValidationError as RestValidationError
from rest_framework.fields      import  empty

from common.serializers         import  BulkUpdateListSerializer, ExtendedModelSerializer
from common.serializers.mixins  import  BaseQuotaCheckerMixin
from common.serializers.mixins.internal import AugmentEditFieldsMixin
from common.validators          import  UniqueListItemsValidator

from ..models.base import EmailAddress, PhoneNumber, GeoLocation, _atomicity_fn, UserQuotaRelation, GenericUserAppliedRole, GenericUserGroupRelation
from  .common import ConnectedProfileField, UserSubformSetupMixin

_logger = logging.getLogger(__name__)
# logstash will duplicate the same log message, figure out how that happenes.

class QuotaValidator(MaxValueValidator):
    def __call__(self, value):
        item_len = len(value)
        log_msg = ['item_len', item_len, 'limit_value', self.limit_value]
        _logger.debug(None, *log_msg)
        super().__call__(item_len)


class AugmentUserRefMixin(AugmentEditFieldsMixin):
    _field_name_map = {'usr' :'_user_instance', }

class QuotaCheckerSerializer(AugmentUserRefMixin, BaseQuotaCheckerMixin, BulkUpdateListSerializer):
    # ensure method resolution order meets application requirement : __mro__
    def __init__(self, *args, **kwargs):
        errmsg = 'you haven\'t configured quota for the item'
        quota_validator = QuotaValidator(limit_value=0, message=errmsg)
        super().__init__(*args, quota_validator=quota_validator, **kwargs)
        self.validators.append(quota_validator)

    def edit_quota_threshold(self, quota_validator, value):
        errmsg = 'number of items provided exceeds the limit: {q0}'
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
    default_validators = [UniqueListItemsValidator(fields=['material']),]

    def _retrieve_material_ids(self, data):
        _fn = lambda d: d[self.pk_field_name]
        ids = map(_fn, filter(_fn, data))
        conditions = {LOOKUP_SEP.join([self.pk_field_name,'in']) : ids}
        qset = self._current_quota_applied.filter(**conditions)
        qset = qset.values_list(self.pk_field_name, flat=True)
        return qset

    def _update_data_map(self, data):
        mat_ids = self._retrieve_material_ids(data)
        out = {item[self.pk_field_name]: item for item in data if item[self.pk_field_name].id in mat_ids}
        return out

    def _insert_data_map(self, data):
        mat_ids = self._retrieve_material_ids(data)
        return [item for item in data if item[self.pk_field_name].id not in mat_ids]

    def update(self, instance, validated_data, allow_insert=False, allow_delete=False, **kwargs):
        self._current_quota_applied = instance
        instance = super().update(instance=self._current_quota_applied, validated_data=validated_data,
                allow_insert=allow_insert, allow_delete=allow_delete, **kwargs,)
        return instance



class CommonUserSubformSerializer(ExtendedModelSerializer, UserSubformSetupMixin):
    atomicity = _atomicity_fn

    def run_validation(self, data=empty):
        value = super().run_validation(data=data, set_null_if_obj_not_found=True)
        return value

    def create(self, validated_data):
        # In this application, this may be invoked by GenericUserGroupSerializer
        # or GenericUserProfileSerializer
        usr = validated_data.pop('_user_instance', None)
        validated_data['user_type'] = ContentType.objects.get_for_model(usr)
        validated_data['user_id'] = usr.pk
        log_msg = ['srlz_cls', type(self).__qualname__, 'validated_data', validated_data]
        _logger.debug(None, *log_msg)
        instance = super().create(validated_data=validated_data)
        return instance
#### end of  CommonUserSubformSerializer


class EmailSerializer(CommonUserSubformSerializer):
    class Meta(CommonUserSubformSerializer.Meta):
        model = EmailAddress
        fields = ['id', 'user_type', 'user_id', 'addr']
        read_only_fields = ['user_type', 'user_id']
        list_serializer_class = QuotaCheckerSerializer

    def extra_setup_before_validation(self, instance, data):
        self._mark_as_creation_on_update(pk_field_name='id', instance=instance, data=data)


class PhoneNumberSerializer(CommonUserSubformSerializer):
    class Meta(CommonUserSubformSerializer.Meta):
        model = PhoneNumber
        fields = ['id', 'user_type', 'user_id', 'country_code', 'line_number',]
        read_only_fields = ['user_type', 'user_id']
        list_serializer_class = QuotaCheckerSerializer

    def extra_setup_before_validation(self, instance, data):
        self._mark_as_creation_on_update(pk_field_name='id', instance=instance, data=data)


class GeoLocationSerializer(CommonUserSubformSerializer):
    class Meta(CommonUserSubformSerializer.Meta):
        model = GeoLocation
        fields = ['id', 'user_type', 'user_id', 'country', 'province', 'locality', 'street', 'detail', 'floor', 'description',]
        read_only_fields = ['user_type', 'user_id']
        list_serializer_class = QuotaCheckerSerializer

    def extra_setup_before_validation(self, instance, data):
        self._mark_as_creation_on_update(pk_field_name='id', instance=instance, data=data)


class UserQuotaRelationSerializer(CommonUserSubformSerializer):
    class Meta(CommonUserSubformSerializer.Meta):
        model = UserQuotaRelation
        fields = ['user_type', 'user_id', 'material', 'maxnum', 'expiry']
        read_only_fields = ['user_type', 'user_id']
        list_serializer_class = BulkUserQuotaRelationSerializer

    def __init__(self, *args, data=empty, **kwargs):
        # DO NOT pass client form data on initialization, to avoid pk field from converting
        # to IntegerField at ExtendedModelSerializer , also avoid the list serializer
        # (BulkUpdateListSerializer) from adding EditFormObjIdValidator
        super().__init__(*args, pk_field_name='material', **kwargs)
        exp_validator = MinValueValidator(limit_value=django_timezone.now())
        self.fields['expiry'].validators.append(exp_validator)

    def update(self, instance, validated_data):
        log_msg = ['srlz_cls', type(self).__qualname__, 'instance_id', instance.pk, \
                'new_material', validated_data['material'], 'new_maxnum', validated_data['maxnum'], \
                'old_maxnum', instance.maxnum, ]
        old_expiry = instance.expiry
        new_expiry = validated_data['expiry']
        if old_expiry:
            old_expiry = old_expiry.isoformat()
        if new_expiry:
            new_expiry = new_expiry.isoformat()
        log_msg.extend(['old_expiry', old_expiry, 'new_expiry', new_expiry])
        try:
            assert instance.material == validated_data['material'], 'material does not match'
            instance = super().update(instance=instance, validated_data=validated_data)
        except AssertionError as e:
            _logger.error(None, *log_msg)
            raise
        else:
            _logger.debug(None, *log_msg)
        return instance




class _BulkUserPriviledgeAssigner(AugmentUserRefMixin, BulkUpdateListSerializer):
    def to_representation(self, data):
        if isinstance(data, DjangoModelManager):
            account = self.child._account
            log_msg = []
            if account:
                profile = account.profile
                if account.is_superuser:
                    pass # only superusers can check all role(s)/group(s) applied to a  single user group / individual user
                else: # For non-superuser logged-in accounts, fetch the role(s) / group(s) approved by themselves.
                    data = data.filter(approved_by=profile.pk)
                log_msg += ['superuser', account.is_superuser, 'approved_by', profile.pk]
            else:
                data = data.none()
            _logger.debug(None, *log_msg)
        out = super().to_representation(data=data)
        return out

    def _retrieve_priv_ids(self, data):
        _fn = lambda d: d[self.pk_field_name]
        ids = map(_fn, filter(_fn, data))
        conditions = {LOOKUP_SEP.join([self.pk_field_name,'in']) : ids}
        qset = self._current_priv_set_applied.filter(**conditions)
        qset = qset.values_list(self.pk_field_name, flat=True)
        return qset

    def _update_data_map(self, data):
        # the model of this serializer has compound key which consists 2 referential fields that always exists.
        # the same function at parent class determines whether an input data item goes to insertion map
        # or update map by checking whether a pk field is null. Therefore I cnanot reply on the same function
        # at parent class and instead overwrite the same function at here
        priv_ids = self._retrieve_priv_ids(data)
        out = {item[self.pk_field_name]: item for item in data if item[self.pk_field_name].id in priv_ids}
        return out

    def _insert_data_map(self, data):
        priv_ids = self._retrieve_priv_ids(data)
        return [item for item in data if item[self.pk_field_name].id not in priv_ids]

    def update(self, instance, validated_data, allow_insert=False, allow_delete=False, **kwargs):
        self._current_priv_set_applied = instance
        log_msg = ['srlz_cls', type(self.child).__qualname__, 'instance', instance,
                'validated_data', validated_data]
        _logger.debug(None, *log_msg)
        instance = super().update(instance=self._current_priv_set_applied, validated_data=validated_data,
                allow_insert=allow_insert, allow_delete=allow_delete, **kwargs,)
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

    def to_representation(self, instance):
        out = super().to_representation(instance=instance)
        return out

    def run_validation(self, data=empty):
        return super().run_validation(data=data, set_null_if_obj_not_found=True)


class GenericUserRoleBulkAssigner(_BulkUserPriviledgeAssigner):
    default_validators = [UniqueListItemsValidator(fields=['role']),]


class RoleAssignValidator:
    requires_context = True

    def __init__(self, profile):
        self._profile = profile

    def __call__(self, value, caller):
        if self._profile.privilege_status == type(self._profile).SUPERUSER:
            return
        roles_available = self._profile.all_roles
        role_id = getattr(value, 'id')
        role_exist_direct  = roles_available['direct' ].filter(id=role_id).exists()
        role_exist_inherit = roles_available['inherit'].filter(id=role_id).exists()
        if not (role_exist_direct or role_exist_inherit):
            err_msg = 'Role is NOT assigned to current login user: %s' \
                    % (value.id)
            raise RestValidationError(err_msg)


class GenericUserRoleAssigner(_BaseUserPriviledgeAssigner):
    class Meta(_BaseUserPriviledgeAssigner.Meta):
        _apply_type = 'role'
        model = GenericUserAppliedRole
        fields = [_apply_type, 'user_type', 'user_id', 'expiry']
        read_only_fields = ['user_type', 'user_id',]
        list_serializer_class = GenericUserRoleBulkAssigner

    def __init__(self, *args, **kwargs):
        super().__init__(*args, **kwargs)
        role_id_validator = RoleAssignValidator(profile=self._account.profile)
        exp_validator = MinValueValidator(limit_value=django_timezone.now())
        self.fields['role'].validators.append(role_id_validator)
        self.fields['expiry'].validators.append(exp_validator)

    def create(self, validated_data):
        target = validated_data.pop('_user_instance', None)
        validated_data['user_type'] = ContentType.objects.get_for_model(target)
        validated_data['user_id']   = target.pk
        validated_data['approved_by'] = self._account.profile
        log_msg = ['validated_data', validated_data]
        _logger.debug(None, *log_msg)
        instance = super().create(validated_data=validated_data)
        return instance

    def update(self, instance, validated_data):
        log_msg = []
        if instance.approved_by != self._account.profile:
            log_msg.extend(['profile_before_edit', instance.approved_by.id, 'profile_after_edit', self._account.profile.id ])
        old_role = getattr(instance, self.Meta._apply_type)
        new_role = validated_data[self.Meta._apply_type]
        log_msg.extend(['old_role', old_role, 'new_role', new_role])
        if old_role == new_role:
            if validated_data['expiry'] != instance.expiry:
                validated_data['approved_by'] = self._account.profile
                instance = super().update(instance=instance, validated_data=validated_data)
            _logger.debug(None, *log_msg)
        else:
            _logger.error(None, *log_msg)
        return instance


class GenericUserGroupRelationAssigner(_BaseUserPriviledgeAssigner):
    class Meta(_BaseUserPriviledgeAssigner.Meta):
        _apply_type = 'group'
        model = GenericUserGroupRelation
        fields = [_apply_type, 'profile',]
        read_only_fields = ['profile',]

    def create(self, validated_data):
        target = validated_data.pop('_user_instance', None)
        validated_data['profile'] = target
        validated_data['approved_by'] = self._account.profile
        log_msg = ['validated_data', validated_data]
        _logger.debug(None, *log_msg)
        instance = super().create(validated_data=validated_data)
        return instance

    def update(self, instance, validated_data):
        log_msg = []
        try:
            d_type = validated_data[self.Meta._apply_type]
            log_msg += ['apply_type', d_type]
            assert getattr(instance, self.Meta._apply_type) == d_type
            # content will be the same , no need to update
        except (ValueError, KeyError, AssertionError) as e:
            log_msg += ['excpt_msg', e]
            _logger.error(None, *log_msg)
        return instance


