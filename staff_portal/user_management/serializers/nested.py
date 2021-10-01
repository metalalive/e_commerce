import logging

from django.db.models import  Manager as DjangoModelManager
from django.db.utils  import  IntegrityError
from django.contrib.contenttypes.models  import ContentType
from rest_framework.exceptions  import  ErrorDetail as DRFErrorDetail, ValidationError as RestValidationError
from rest_framework.fields      import  empty

from common.serializers         import  BulkUpdateListSerializer, ExtendedModelSerializer
from common.serializers.mixins  import  QuotaCheckerMixin
from common.serializers.mixins.internal import AugmentEditFieldsMixin
from common.validators          import  UniqueListItemsValidator

from ..models.base import EmailAddress, PhoneNumber, GeoLocation, _atomicity_fn, UserQuotaRelation, GenericUserAppliedRole, GenericUserGroupRelation
from  .common import ConnectedGroupField, ConnectedProfileField, UserSubformSetupMixin

_logger = logging.getLogger(__name__)



class AugmentUserRefMixin(AugmentEditFieldsMixin):
    _field_name_map = {'usr' :'_user_instance', }


class QuotaCheckerSerializer(QuotaCheckerMixin, AugmentUserRefMixin, BulkUpdateListSerializer):
    # ensure method resolution order meets application requirement : __mro__
    pass


class BulkUserQuotaRelationSerializer(AugmentUserRefMixin, BulkUpdateListSerializer):
    """
    * It should be safe to override default validator list, since this class
      shouldn't be overriden by any other class.
    * UniqueListItemsValidator is applied instead of UniqueTogetherValidator
      for checking uniqueness of usage type in each quota arrangement form,
      because UniqueListItemsValidator can do things quickly without database
      connection.
      (side note: it doesn't make sense to have duplicate usage type for a user,
      e.g. have both usage-type-email=2 and usage-type-email=9 in client request)
    """
    default_validators = [UniqueListItemsValidator(fields=['usage_type']),]


class CommonUserSubformSerializer(ExtendedModelSerializer, UserSubformSetupMixin):
    atomicity = _atomicity_fn

    def extra_setup_before_validation(self, instance, data):
        self._mark_as_creation_on_update(pk_field_name='id', instance=instance, data=data)

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

    def update(self, instance, validated_data, allow_insert=False, allow_delete=False):
        log_msg = ['srlz_cls', type(self).__qualname__, 'instance_id', instance.pk, 'validated_data', validated_data]
        _logger.debug(None, *log_msg)
        instance = super().update(instance=instance, validated_data=validated_data)
        return instance
#### end of  CommonUserSubformSerializer


class EmailSerializer(CommonUserSubformSerializer):
    class Meta(CommonUserSubformSerializer.Meta):
        model = EmailAddress
        fields = ['id', 'user_type', 'user_id', 'addr']
        read_only_fields = ['user_type', 'user_id']
        list_serializer_class = QuotaCheckerSerializer

class PhoneNumberSerializer(CommonUserSubformSerializer):
    class Meta(CommonUserSubformSerializer.Meta):
        model = PhoneNumber
        fields = ['id', 'user_type', 'user_id', 'country_code', 'line_number',]
        read_only_fields = ['user_type', 'user_id']
        list_serializer_class = QuotaCheckerSerializer

class GeoLocationSerializer(CommonUserSubformSerializer):
    class Meta(CommonUserSubformSerializer.Meta):
        model = GeoLocation
        fields = ['id', 'user_type', 'user_id', 'country', 'province', 'locality', 'street', 'detail', 'floor', 'description',]
        read_only_fields = ['user_type', 'user_id']
        list_serializer_class = QuotaCheckerSerializer


class UserQuotaRelationSerializer(CommonUserSubformSerializer):
    class Meta(CommonUserSubformSerializer.Meta):
        model = UserQuotaRelation
        fields = ['id', 'user_type', 'user_id', 'usage_type', 'maxnum']
        read_only_fields = ['user_type', 'user_id']
        list_serializer_class = BulkUserQuotaRelationSerializer

    def __init__(self, instance=None, data=empty, **kwargs):
        if data is empty:
            self.fields['usage_type'] = None
            raise NotImplementedError()
        super().__init__(instance=instance, data=data, **kwargs)




class _BulkUserPriviledgeAssigner(AugmentUserRefMixin, BulkUpdateListSerializer):
    def augment_write_data(self, target, data, account):
        return self.child.augment_write_data(target=target, data=data, account=account)

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

    def update(self, instance, validated_data, allow_insert=False, allow_delete=False, **kwargs):
        account = self.child._account
        if not account.is_superuser:
            profile = account.profile
            instance = instance.filter(approved_by=profile.pk)
        log_msg = ['srlz_cls', type(self.child).__qualname__, 'validated_data', validated_data]
        _logger.debug(None, *log_msg)
        instance = super().update(instance=instance, validated_data=validated_data, **kwargs,
                allow_insert=allow_insert, allow_delete=allow_delete)
        return instance


class _BaseUserPriviledgeAssigner(ExtendedModelSerializer, UserSubformSetupMixin):
    atomicity = _atomicity_fn
    class Meta(ExtendedModelSerializer.Meta):
        # subclasses must orverride these fields
        _apply_type = None
        model = None
        list_serializer_class = _BulkUserPriviledgeAssigner

    def to_representation(self, instance):
        # `_apply_type` is either `role` or `group` instance
        apply_type = getattr(instance, self.Meta._apply_type)
        out = {'id': apply_type.pk, 'name': apply_type.name}
        # instance.role.pk
        return out

    def augment_write_data(self, data, account, filter_kwargs):
        """
        input:  list of IDs for role(s) or group(s),
        output: list of dicts, each of which contains user_id/user_type
        """
        # TODO: backend should also receive expiry time of each role in request data, then
        # the structure of each input item would roughly be :
        # {'apply_type': some_id, 'valid_time_period': time_range}
        if data:
            out = {d: {'id': '', self.Meta._apply_type: d,} for d in data}
            # retrieve apply ID, by giving role ID (or group ID)
            if not account.is_superuser:
                profile = account.profile
                filter_kwargs['approved_by'] = profile.pk
            apply_ids = self.Meta.model.objects.filter(**filter_kwargs)
            for a in apply_ids:
                type_id = getattr(a, self.Meta._apply_type).pk
                out[type_id]['id'] = getattr(a, 'pk')
            data = list(out.values())
            log_msg = ['srlz_cls', type(self).__qualname__, 'filter_kwargs', filter_kwargs, 'data', data]
            _logger.debug(None, *log_msg)
        return data

    def run_validation(self, data=empty):
        return super().run_validation(data=data, set_null_if_obj_not_found=True)

    def extra_setup_before_validation(self, instance, data):
        self._mark_as_creation_on_update(pk_field_name='id', instance=instance, data=data)

    def update(self, instance, validated_data):
        log_msg = []
        try:
            d_id   = validated_data['id']
            d_type = validated_data[self.Meta._apply_type]
            log_msg += ['id', d_id, 'apply_type', d_type]
            assert getattr(instance, 'id') == d_id
            assert getattr(instance, self.Meta._apply_type) == d_type
            # content will be the same , no need to update
        except (ValueError, KeyError, AssertionError) as e:
            log_msg += ['excpt_msg', e]
            _logger.error(None, *log_msg)
        return instance



class GenericUserRoleAssigner(_BaseUserPriviledgeAssigner):
    class Meta(_BaseUserPriviledgeAssigner.Meta):
        _apply_type = 'role'
        model = GenericUserAppliedRole
        fields = ['id', _apply_type, 'user_type', 'user_id', 'last_updated']
        read_only_fields = ['user_type', 'user_id',]

    def to_representation(self, instance):
        out = super().to_representation(instance=instance)
        return out

    def augment_write_data(self, target, data, account):
        user_id   = getattr(target, 'pk') if target else 0
        user_type = ContentType.objects.get_for_model(target) if target else None
        filter_kwargs = {'role__pk__in':data, 'user_type':user_type, 'user_id':user_id,}
        return super().augment_write_data(data=data, account=account, filter_kwargs=filter_kwargs)

    def create(self, validated_data):
        target = validated_data.pop('_user_instance', None)
        validated_data['user_type'] = ContentType.objects.get_for_model(target)
        validated_data['user_id']   = target.pk
        validated_data['approved_by'] = self._account.profile
        log_msg = ['validated_data', validated_data]
        _logger.debug(None, *log_msg)
        instance = super().create(validated_data=validated_data)
        return instance


class GenericUserGroupRelationAssigner(_BaseUserPriviledgeAssigner):
    class Meta(_BaseUserPriviledgeAssigner.Meta):
        _apply_type = 'group'
        model = GenericUserGroupRelation
        fields = ['id', _apply_type, 'user_type', 'user_id', 'last_updated']
        read_only_fields = ['user_type', 'user_id',]

    def to_representation(self, instance):
        out = super().to_representation(instance=instance)
        return out

    def augment_write_data(self, target, data, account):
        profile_id = getattr(target, 'pk') if target else 0
        filter_kwargs = {'group__pk__in':data,  'profile__pk':profile_id,}
        return super().augment_write_data(data=data, account=account, filter_kwargs=filter_kwargs)

    def create(self, validated_data):
        target = validated_data.pop('_user_instance', None)
        validated_data['profile'] = target
        validated_data['approved_by'] = self._account.profile
        log_msg = ['validated_data', validated_data]
        _logger.debug(None, *log_msg)
        instance = super().create(validated_data=validated_data)
        return instance


