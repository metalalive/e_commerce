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
from location.models   import Location

from ..models import UserEmailAddress, EmailAddress, UserPhoneNumber, PhoneNumber, UserLocation, _atomicity_fn
from ..models import UserQuotaRelation, QuotaUsageType,  GenericUserAppliedRole, GenericUserGroupRelation
from  .common import ConnectedGroupField, ConnectedProfileField, UserSubformSetupMixin


_logger = logging.getLogger(__name__)



class QuotaUsageTypeListSerializer(BulkUpdateListSerializer):
    def create(self, *args, **kwargs):
        # even `material` is one-to-one field at model level, DRF validator
        # still does not check uniquenes of material field, such unique
        #  constraint check will be passed down to database level
        try:
            return super().create(*args, **kwargs)
        except IntegrityError as e:
            self._handle_dup_material(e)

    def _handle_dup_material(self, error):
        err_code = error.args[0]
        err_msg = error.args[1].lower()
        if ('duplicate entry' in err_msg) and ('material' in err_msg) and \
                (self.child.Meta.model._meta.db_table in err_msg):
            dup_id = err_msg.split()[2]
            dup_id = dup_id.strip('\'')
            dup_id = int(dup_id)
            err_msg = '{"message":"duplicate entry","id":%s}' % dup_id
            err_detail = DRFErrorDetail(err_msg, code='conflict')
            occurences = list(filter(lambda d:d['material'] == dup_id, self.initial_data))
            idx = self.initial_data.index(occurences[1])
            err_details = [{} for _ in range(len(self.initial_data))]
            err_details[idx]['material'] = [err_detail]
            validation_error = RestValidationError(detail=err_details)
            raise validation_error
        else:
            raise

class QuotaUsageTypeSerializer(ExtendedModelSerializer):
    atomicity = _atomicity_fn
    class Meta(ExtendedModelSerializer.Meta):
        model  = QuotaUsageType
        fields = ['id', 'label', 'material']
        list_serializer_class = QuotaUsageTypeListSerializer

    def to_representation(self, instance):
        out = super().to_representation(instance=instance)
        mat_id = out.get('material', None)
        if mat_id: # TODO, do this at model level ?
            obj = ContentType.objects.values('app_label').get(pk=mat_id)
            out['appname'] = obj['app_label']
        return out


class EmailSerializer(ExtendedModelSerializer, UserSubformSetupMixin):
    atomicity = _atomicity_fn
    class Meta(ExtendedModelSerializer.Meta):
        model = EmailAddress
        fields = ['id', 'addr',]
    def extra_setup_before_validation(self, instance, data):
        self._mark_as_creation_on_update(pk_field_name='id', instance=instance, data=data)

class PhoneNumberSerializer(ExtendedModelSerializer, UserSubformSetupMixin):
    atomicity = _atomicity_fn
    class Meta(ExtendedModelSerializer.Meta):
        model = PhoneNumber
        fields = ['id', 'country_code', 'line_number',]
    def extra_setup_before_validation(self, instance, data):
        self._mark_as_creation_on_update(pk_field_name='id', instance=instance, data=data)

class GeoLocationSerializer(ExtendedModelSerializer, UserSubformSetupMixin):
    atomicity = _atomicity_fn
    class Meta(ExtendedModelSerializer.Meta):
        model = Location
        fields = ['id', 'country', 'province', 'locality', 'street', 'detail', 'floor', 'description',]
    def extra_setup_before_validation(self, instance, data):
        self._mark_as_creation_on_update(pk_field_name='id', instance=instance, data=data)


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
    # TODO, if nested_form_field is not listed in query parameter of client request
    # then self.fields[nested_form_field] will be thrown away which cause server
    # error in following functions, fix this issue

    def __init__(self, instance=None, data=empty, **kwargs):
        if hasattr(self.Meta, 'nested_form_field') and self.Meta.nested_form_field:
            key = self.Meta.nested_form_field
            self.fields[key] = self.Meta.nested_form_cls(many=False, instance=instance, data=data)
        super().__init__(instance=instance, data=data, **kwargs)

    def _construct_nested_form(self, data):
        if hasattr(self.Meta, 'nested_form_field') and self.Meta.nested_form_field:
            key = self.Meta.nested_form_field
            field_obj = self.fields.get(key, None)
            if field_obj:
                _field_names = field_obj.Meta.fields
                _nest = {k:data.pop(k) for k in  _field_names if not data.get(k,None) is None}
                data[key] = _nest
                uid = data.pop('uid', None)
                if uid:
                    data['id'] = uid
                log_msg = ['srlz_cls', type(self).__qualname__, 'key', key, 'data', data]
                _logger.debug(None, *log_msg)

    def _destruct_nested_form(self, data):
        if hasattr(self.Meta, 'nested_form_field') and self.Meta.nested_form_field:
            key = self.Meta.nested_form_field
            field_obj = self.fields.get(key, None)
            if field_obj:
                _field_names = field_obj.Meta.fields
                _nest = data.pop(key, None)
                _data = {k:_nest.pop(k) for k in  _field_names if not _nest.get(k,None) is None}
                uid = data.pop('id', None)
                if uid:
                    data['uid'] = uid
                data = data | _data
        return data

    def to_representation(self, instance):
        out = super().to_representation(instance=instance)
        out = self._destruct_nested_form(data=out)
        return out

    def extra_setup_before_validation(self, instance, data):
        if hasattr(self.Meta, 'nested_form_field') and self.Meta.nested_form_field:
            self._setup_subform_instance(name=self.Meta.nested_form_field,
                    instance=instance, data=data)
        self._mark_as_creation_on_update(pk_field_name='id', instance=instance, data=data)

    def run_validation(self, data=empty):
        try:
            self._construct_nested_form(data=data)
            value = super().run_validation(data=data, set_null_if_obj_not_found=True)
        except RestValidationError as e:
            log_msg = ['srlz_cls', type(self).__qualname__, 'excpt_msg', e.detail]
            _logger.info(None, *log_msg)
            _detail = self._destruct_nested_form(data=e.detail)
            raise RestValidationError(detail=_detail)
        return value

    def create(self, validated_data):
        # In this application, this may be invoked by GenericUserGroupSerializer
        # or GenericUserProfileSerializer
        usr = validated_data.pop('_user_instance', None)
        validated_data['user_type'] = ContentType.objects.get_for_model(usr)
        validated_data['user_id'] = usr.pk
        if hasattr(self.Meta, 'nested_form_field') and self.Meta.nested_form_field:
            key = self.Meta.nested_form_field
            nested_data = validated_data.get(key, None)
            ####nested_data['id'] = ""
            nest_obj = self.fields[key].create(validated_data=nested_data)
            validated_data[key] = nest_obj
        log_msg = ['srlz_cls', type(self).__qualname__, 'validated_data', validated_data]
        _logger.debug(None, *log_msg)
        instance = super().create(validated_data=validated_data)
        return instance

    def update(self, instance, validated_data, allow_insert=False, allow_delete=False):
        if hasattr(self.Meta, 'nested_form_field') and self.Meta.nested_form_field:
            key = self.Meta.nested_form_field
            nested_data = validated_data.get(key, None)
            nest_instance = getattr(instance, key)
            # ignore nested ID sent by frontend request, instead this application loads the associated nested
            # email/phone/location ID before the update ....
            nested_data['id'] = getattr(nest_instance, 'id')
            nest_obj = self.fields[key].update(instance=nest_instance, validated_data=nested_data)
            validated_data[key] = nest_obj
        log_msg = ['srlz_cls', type(self).__qualname__, 'instance_id', instance.pk, 'validated_data', validated_data]
        _logger.debug(None, *log_msg)
        instance = super().update(instance=instance, validated_data=validated_data)
        return instance

#### end of  CommonUserSubformSerializer


class UserEmailRelationSerializer(CommonUserSubformSerializer):
    class Meta(CommonUserSubformSerializer.Meta):
        model = UserEmailAddress
        fields = ['id', 'email', 'user_type', 'user_id']
        nested_form_field = 'email'
        nested_form_cls = EmailSerializer
        list_serializer_class = QuotaCheckerSerializer

class UserPhoneRelationSerializer(CommonUserSubformSerializer):
    class Meta(CommonUserSubformSerializer.Meta):
        model = UserPhoneNumber
        fields = ['id', 'phone', 'user_type', 'user_id']
        nested_form_field = 'phone'
        nested_form_cls = PhoneNumberSerializer
        list_serializer_class = QuotaCheckerSerializer

class UserLocationRelationSerializer(CommonUserSubformSerializer):
    class Meta(CommonUserSubformSerializer.Meta):
        model = UserLocation
        fields = ['id', 'address', 'user_type', 'user_id']
        nested_form_field = 'address'
        nested_form_cls = GeoLocationSerializer
        list_serializer_class = QuotaCheckerSerializer


class UserQuotaRelationSerializer(CommonUserSubformSerializer):
    class Meta(CommonUserSubformSerializer.Meta):
        model = UserQuotaRelation
        fields = ['id', 'usage_type', 'maxnum', 'user_type', 'user_id']
        list_serializer_class = BulkUserQuotaRelationSerializer

    def __init__(self, instance=None, data=empty, **kwargs):
        if data is empty:
            self.fields['usage_type'] = QuotaUsageTypeSerializer(many=False, read_only=True)
        #### if (not data is empty) and instance:
        ####     self.fields['id'] = ModelField(model_field=self.Meta.model._meta.get_field('id'), required=False)
        super().__init__(instance=instance, data=data, **kwargs)




class BulkGenericUserAppliedRoleSerializer(AugmentUserRefMixin, BulkUpdateListSerializer):

    def augment_write_data(self, target, data, account):
        return self.child.augment_write_data(target=target, data=data, account=account)

    def to_representation(self, data):
        if isinstance(data, DjangoModelManager):
            account = self.child._account
            log_msg = []
            if account:
                profile = account.genericuserauthrelation.profile
                if account.is_superuser:
                    pass # only superusers can check all role(s)/group(s) applied to a  single user group / individual user
                elif self.child._from_read_view:
                    pass # read-only view is allowed to grab all role(s)/group(s) applied to a  single user group / individual user
                else: # For non-superuser logged-in accounts, fetch the role(s) / group(s) approved by themselves.
                    data = data.filter(approved_by=profile.pk)
                log_msg += ['superuser', account.is_superuser, '_from_read_view', self.child._from_read_view,
                        'approved_by', profile.pk]
            else:
                data = data.none()
            _logger.debug(None, *log_msg)
        out = super().to_representation(data=data)
        #out.append(deepcopy(out[0]))
        return out

    def update(self, instance, validated_data, allow_insert=False, allow_delete=False, **kwargs):
        account = self.child._account
        if not account.is_superuser:
            profile = account.genericuserauthrelation.profile
            instance = instance.filter(approved_by=profile.pk)
        log_msg = ['srlz_cls', type(self.child).__qualname__, 'validated_data', validated_data]
        _logger.debug(None, *log_msg)
        instance = super().update(instance=instance, validated_data=validated_data, **kwargs,
                allow_insert=allow_insert, allow_delete=allow_delete)
        return instance


class BaseRoleAppliedSerializer(ExtendedModelSerializer, UserSubformSetupMixin):
    atomicity = _atomicity_fn
    class Meta(ExtendedModelSerializer.Meta):
        # subclasses must orverride these fields
        _apply_type = None
        model = None
        list_serializer_class = BulkGenericUserAppliedRoleSerializer

    def __init__(self, instance=None, data=empty, from_edit_view=False, from_read_view=False,  **kwargs):
        self._from_edit_view = from_edit_view
        self._from_read_view = from_read_view
        super().__init__(instance=instance, data=data, **kwargs)

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
                profile = account.genericuserauthrelation.profile
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



class GenericUserAppliedRoleSerializer(BaseRoleAppliedSerializer):
    class Meta(BaseRoleAppliedSerializer.Meta):
        _apply_type = 'role'
        model = GenericUserAppliedRole
        fields = ['id', _apply_type, 'profile', 'group']
    # if not being invoked by edit view, the 2 fields `user_type` and `user_id` will be
    # converted and expanded to either `group` and `profile` field
    group   = ConnectedGroupField(many=False, read_only=True)
    profile = ConnectedProfileField(many=False, read_only=True)

    def to_representation(self, instance):
        if self._from_edit_view or self._from_read_view:
            out = super().to_representation(instance=instance)
        else:
            model_cls = instance.user_type.model_class()
            instance  = model_cls.objects.get(pk=instance.user_id)
            out = {}
            field_names = ['profile', 'group']
            for fname in field_names:
                _field = self.fields[fname]
                if model_cls is _field.Meta.model:
                    out = _field.to_representation(instance)
                    break
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
        validated_data['approved_by'] = self._account.genericuserauthrelation.profile
        log_msg = ['validated_data', validated_data]
        _logger.debug(None, *log_msg)
        instance = super().create(validated_data=validated_data)
        return instance


class GenericUserGroupRelationSerializer(BaseRoleAppliedSerializer):
    class Meta(BaseRoleAppliedSerializer.Meta):
        _apply_type = 'group'
        model = GenericUserGroupRelation
        fields = ['id', _apply_type, 'profile']

    profile = ConnectedProfileField(many=False, read_only=True)

    def to_representation(self, instance):
        if self._from_edit_view or self._from_read_view:
            out = super().to_representation(instance=instance)
        else:
            out = self.fields['profile'].to_representation(instance.profile)
        return out

    def augment_write_data(self, target, data, account):
        profile_id = getattr(target, 'pk') if target else 0
        filter_kwargs = {'group__pk__in':data,  'profile__pk':profile_id,}
        return super().augment_write_data(data=data, account=account, filter_kwargs=filter_kwargs)

    def create(self, validated_data):
        target = validated_data.pop('_user_instance', None)
        validated_data['profile'] = target
        validated_data['approved_by'] = self._account.genericuserauthrelation.profile
        log_msg = ['validated_data', validated_data]
        _logger.debug(None, *log_msg)
        instance = super().create(validated_data=validated_data)
        return instance


