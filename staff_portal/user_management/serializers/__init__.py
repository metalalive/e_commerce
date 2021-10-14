import re
from copy     import copy, deepcopy
from datetime import datetime, timezone
#### from collections     import OrderedDict
import logging

from django.conf      import  settings as django_settings
from django.core      import  validators
from django.db.models.constants import LOOKUP_SEP
from django.utils.deconstruct   import deconstructible
from django.core.exceptions     import ValidationError, ObjectDoesNotExist, ImproperlyConfigured
from django.contrib.auth.models import Permission
from django.contrib.auth        import password_validation, get_user_model
from django.contrib.auth.hashers import check_password
from django.contrib.contenttypes.models  import ContentType

from rest_framework             import status as RestStatus
from rest_framework.fields      import CharField, BooleanField, ModelField, empty
from rest_framework.serializers import BaseSerializer, Serializer, ModelSerializer, IntegerField, PrimaryKeyRelatedField
from rest_framework.validators  import UniqueTogetherValidator
from rest_framework.exceptions  import ValidationError as RestValidationError, ErrorDetail as RestErrorDetail
from rest_framework.settings    import api_settings

from common.serializers         import  BulkUpdateListSerializer, ExtendedModelSerializer, DjangoBaseClosureBulkSerializer
from common.serializers.mixins  import  BaseClosureNodeMixin
from common.util.python         import  get_fixture_pks
from common.util.python.async_tasks    import  sendmail as async_send_mail

from ..async_tasks import update_accounts_privilege
from ..models.common import AppCodeOptions
from ..models.base import GenericUserGroup, GenericUserGroupClosure, GenericUserProfile,  GenericUserGroupRelation, _atomicity_fn, QuotaMaterial
from ..models.auth import Role, AccountResetRequest

from .common import ConnectedGroupField, ConnectedProfileField, UserSubformSetupMixin
from .nested import EmailSerializer, PhoneNumberSerializer, GeoLocationSerializer
from .nested import UserQuotaRelationSerializer, GenericUserRoleAssigner, GenericUserGroupRelationAssigner

_logger = logging.getLogger(__name__)



class PermissionSerializer(ModelSerializer):
    class Meta:
        model = Permission
        fields = ['id', 'name',]
        read_only_fields = ['id','name',]

    def __init__(self, instance=None, data=empty, account=None, **kwargs):
        # read-only serializer, frontend users are not allowed to edit
        # permission model in this project
        if not instance:
            instance = type(self).get_default_queryset()
        super().__init__(instance=instance, data=data, **kwargs)

    @classmethod
    def get_default_queryset(cls):
        app_labels = ['contenttypes', 'auth', 'sessions']
        rel_field_name = ['content_type', 'app_label', 'in']
        rel_field_name = LOOKUP_SEP.join(rel_field_name)
        condition = {rel_field_name:app_labels}
        queryset = cls.Meta.model.objects.exclude(**condition)
        return queryset


class RoleSerializer(ExtendedModelSerializer):
    atomicity = _atomicity_fn
    class Meta(ExtendedModelSerializer.Meta):
        model  = Role
        fields = ['id', 'name', 'permissions',]

    def __init__(self, instance=None, data=empty, **kwargs):
        # To reduce bytes transmitting from API caller, POST/PUT data only contains 
        # list of permission IDs for a role, no need to use customized serializer field
        self.fields['permissions'].child_relation.queryset = PermissionSerializer.get_default_queryset()
        kwargs['pk_field_name'] = 'id'
        super().__init__(instance=instance, data=data, **kwargs)
#### end of RoleSerializer


class GenericUserGroupClosureListSerializer(BulkUpdateListSerializer):
    def to_representation(self, instance):
        condition = {LOOKUP_SEP.join(['depth','gt']): 0}
        instance = instance.filter(**condition)
        out =  super().to_representation(data=instance)
        return out


class GenericUserGroupClosureSerializer(ExtendedModelSerializer):
    atomicity = _atomicity_fn
    class Meta(ExtendedModelSerializer.Meta):
        model = GenericUserGroupClosure
        list_serializer_class = GenericUserGroupClosureListSerializer

class GroupAncestorSerializer(GenericUserGroupClosureSerializer):
    class Meta(GenericUserGroupClosureSerializer.Meta):
        fields = ['depth', 'ancestor',]
        read_only_fields = ['depth', 'ancestor']
    ancestor   = ConnectedGroupField(read_only=True)

class GroupDescendantSerializer(GenericUserGroupClosureSerializer):
    class Meta(GenericUserGroupClosureSerializer.Meta):
        fields = ['depth', 'descendant']
        read_only_fields = ['depth', 'descendant']
    descendant = ConnectedGroupField(read_only=True)



class AbstractGenericUserSerializer(ExtendedModelSerializer, UserSubformSetupMixin):
    # overwrite atomicity function
    atomicity = _atomicity_fn

    # TRICKY, set instance argument will set read_only = False at pk field of model serializer
    # so the pk value will come with other validated data without being dropped.
    def __init__(self, instance=None, data=empty, **kwargs):
        # the order of field name list affects order of fields to be validated,
        # this serializer requires specific fields order on validation described below :
        # (1) validate `groups` field first
        # (2) validate `roles` field, validate roles and groups together
        # (3) validate `quota` field
        # (4) estimate final quota arrangment for each user, using `groups` value and `quota` value
        # (5) take estimated value at previous step, to validate emails/phones/geo-locations fields
        self.fields['roles']  = GenericUserRoleAssigner(many=True, instance=instance, account=kwargs.get('account'))
        self.fields['quota']  = UserQuotaRelationSerializer(many=True, instance=instance,
                _validation_error_callback=self._validate_quota_error_callback)
        self.fields['emails'] = EmailSerializer(many=True, instance=instance, data=data)
        self.fields['phones'] = PhoneNumberSerializer(many=True, instance=instance, data=data)
        self.fields['locations'] = GeoLocationSerializer(many=True, instance=instance, data=data)
        super().__init__(instance=instance, data=data, **kwargs)

    def run_validation(self, data=empty):
        try:
            value = super().run_validation(data=data)
        except (RestValidationError,) as e:
            emails_err_info = e.detail.get('emails')
            phones_err_info = e.detail.get('phones')
            locations_err_info = e.detail.get('locations')
            if (emails_err_info or phones_err_info or locations_err_info) and hasattr(self, '_final_quota_list'):
                log_msg = ['emails_err_info', str(emails_err_info), 'phones_err_info', str(phones_err_info), \
                        'locations_err_info', str(locations_err_info), '_final_quota_list', str(self._final_quota_list)]
                _logger.info(None, *log_msg)
                delattr(self, '_final_quota_list')
            raise
        return value

    def validate(self, value):
        log_msg = ['validated_quota', value['quota']]
        _logger.debug(None, *log_msg)
        validated_value = super().validate(value=value, exception_cls=ValidationError, _logger=_logger)
        return validated_value

    def extra_setup_before_validation(self, instance, data):
        subform_keys = [
                ('roles',('user_type', 'user_id', 'role')),
                ('quota',('user_type', 'user_id', 'material')),
                ('emails','id'), ('phones','id'), ('locations','id')
            ]
        for s_name, pk_name in subform_keys:
            self._setup_subform_instance(name=s_name, instance=instance, data=data, pk_field_name=pk_name)
            self._append_user_field(name=s_name, instance=instance, data=data)
            # consider parent list serializer will call this function multiple
            # times for validation, reset the read-only state
            self.fields[s_name].read_only = False
        log_msg = ['roles_data', data.get('roles')]
        _logger.debug(None, *log_msg)


    def _instant_update_contact_quota(self, _final_quota):
        usermgt_materials_code = tuple(map(lambda opt:opt.value , QuotaMaterial._MatCodeOptions))
        orm_filter_kwargs = {'app_code':AppCodeOptions.user_management,
                LOOKUP_SEP.join(['mat_code', 'in']) : usermgt_materials_code  }
        mat_ids = QuotaMaterial.objects.filter(**orm_filter_kwargs).values('id','mat_code')
        mat_id_map = {v['mat_code']:v['id'] for v in mat_ids}
        subform_keys =  ['emails','phones','locations']
        for k in subform_keys:
            nested_field = self.fields[k]
            mat_code = nested_field.child.Meta.model.quota_material.value
            mat_id = mat_id_map[mat_code]
            quota_val = _final_quota.get(mat_id, 0)
            nested_field.applied_quota = quota_val
        if not hasattr(self, '_final_quota_list'):
            self._final_quota_list = []
        self._final_quota_list.append(_final_quota) # for debug purpose 


    def _validate_quota_error_callback(self, exception):
        # skip validation on subsequent subform fields (if exists),
        # which rely on validated value of the quota fields
        log_msg = ['excpt_msg', exception]
        if self.instance:
            log_msg += ['edit_profile_id', self.instance.pk]
        _logger.debug(None, *log_msg)
        subform_keys = ['emails','phones','locations']
        for k in subform_keys:
            self.fields[k].read_only = True

    def create(self, validated_data):
        subform_keys = ['roles', 'quota', 'emails','phones','locations']
        validated_subform_data = {k: validated_data.pop(k, None) for k in subform_keys}
        with self.atomicity():
            instance = super().create(validated_data=validated_data)
            for k in subform_keys:
                self.fields[k].create(validated_data=validated_subform_data[k], usr=instance)
        return instance

    def update(self, instance, validated_data):
        subform_keys = ['roles', 'quota','emails','phones','locations']
        validated_subform_data = {k: validated_data.pop(k, None) for k in subform_keys}
        instance = super().update(instance=instance, validated_data=validated_data)
        for k in subform_keys:
            field = self.fields[k]
            subform_qset = getattr(instance, k).all()
            field.update(instance=subform_qset, validated_data=validated_subform_data[k],
                    usr=instance, allow_insert=True, allow_delete=True)
        return instance
## end of class AbstractGenericUserSerializer



class GenericUserProfileSerializer(AbstractGenericUserSerializer):
    class Meta(AbstractGenericUserSerializer.Meta):
        model = GenericUserProfile
        fields = ['id', 'first_name', 'last_name', 'active', 'time_created', 'last_updated', 'auth']
    # This serializer doesn't (also shouldn't) fetch data from contrib.auth User model, instead it
    # simply shows whether each user as login account or not.
    auth = BooleanField(read_only=True)
    # internally check roles and groups that will be applied
    # default value: 1 = superuser role, 2 = staff role
    PRESERVED_ROLE_IDS = get_fixture_pks(filepath='fixtures.json', pkg_hierarchy='user_management.role')

    def __init__(self, instance=None, data=empty, **kwargs):
        #### TODO, load all applied groups regardless who approved it, only for read view
        self.fields['groups'] = GenericUserGroupRelationAssigner(many=True, instance=instance,
                data=data, account=kwargs.get('account'))
        super().__init__(instance=instance, data=data, **kwargs)
        # When non-superuser logged-in users edit their own profile, they cannot modify `group`, `quota`,
        # and `role` fields, the data of all these fields are already assigned by someone at upper layer group
        # (or superuser),  each non-admin logged-in users cannot add a new role / group by themselves.
        # In such case this serializer internally ignored the write data in `group`, `role`, `quota` field
        self._skip_edit_permission_data = []

    def extra_setup_before_validation(self, instance, data):
        skip_edit_permission_data = False
        if not self._account.is_superuser and self.instance:
            logged_in_profile = self._account.profile
            editing_profile = self.instance
            if editing_profile.pk == logged_in_profile.pk:
                skip_edit_permission_data = True
            log_msg = ['skip_edit_permission_data', skip_edit_permission_data, 'editing_profile.pk', editing_profile.pk]
            _logger.debug(None, *log_msg)
        self._skip_edit_permission_data.append(skip_edit_permission_data)
        if self._skip_edit_permission_data[-1]:
            data['roles']  = []
            data['groups'] = []
            #data['quota']  = [] # TODO, figure out why this can be ignored
        subform_keys = [('groups', 'id'),]
        for s_name, pk_name in subform_keys:
            self._setup_subform_instance(name=s_name, instance=instance, data=data, pk_field_name=pk_name)
            self._append_user_field(name=s_name, instance=instance, data=data)
            self.fields[s_name].read_only = False
        super().extra_setup_before_validation(instance=instance, data=data)

    def validate_groups(self, value):
        self._applied_groups = [v['group'] for v in value]
        # load only when non-admin authenticated user edit group/role field of other user profile(s)
        if not self._account.is_superuser and self.instance:
            others_approved = GenericUserGroupRelation.objects.filter(profile=self.instance).exclude(
                    approved_by=self._account.genericuserauthrelation.profile )
            others_approved     = others_approved.values_list("group", flat=True)
            others_approved     = GenericUserGroup.objects.filter(pk__in=others_approved)
            gid_approved_this_user = list(map(lambda grp: grp.pk, self._applied_groups))
            self._applied_groups = self._applied_groups + list(others_approved)
            gid_approved_others  = list(others_approved.values_list('id', flat=True))
            log_msg = ['gid_approved_this_user', gid_approved_this_user, 'gid_approved_others', gid_approved_others]
            _logger.debug(None, *log_msg)
        return value

    def validate_roles(self, value):
        if not any(self._applied_groups):
            superuser_role_id = self.PRESERVED_ROLE_IDS[0]
            superuser_role = tuple(filter(lambda v: v['role'].id == superuser_role_id, value))
            if not any(superuser_role):
                roles_id = list(map(lambda v: v['role'].id, value))
                log_msg = ['applied_group_exist', False, 'roles_id',  roles_id]
                _logger.debug(None, *log_msg)
                raise ValidationError("non-admin users must select at least one user group")
        return value

    def validate_quota(self, value):
        grp_ids = tuple(map(lambda obj:obj.id, self._applied_groups))
        groups_qset = GenericUserGroup.objects.filter(id__in=grp_ids)
        _final_quota = self._estimate_hierarchy_quota(override=value, groups=groups_qset)
        self._instant_update_contact_quota(_final_quota)
        return value

    def _estimate_hierarchy_quota(self, override, groups):
        override = {oq['material'].id : oq['maxnum'] for oq in override}
        merged_inhehited = self.Meta.model.estimate_inherit_quota(groups=groups)
        final_applied = merged_inhehited | override
        log_msg = ['override', override, 'final_applied', final_applied]
        _logger.debug(None, *log_msg)
        return final_applied

    def create(self, validated_data):
        subform_keys = ['groups',]
        validated_subform_data = {k: validated_data.pop(k, None) for k in subform_keys}
        with self.atomicity():
            instance = super().create(validated_data=validated_data)
            for k in subform_keys:
                self.fields[k].create(validated_data=validated_subform_data[k], usr=instance)
        return instance

    def update(self, instance, validated_data):
        # remind: parent list serializer will set atomic transaction, no need to set it at here
        subform_keys = ['groups', 'roles', 'quota','emails','phones','locations']
        validated_subform_data = {k: validated_data.pop(k, None) for k in subform_keys}
        instance = super().update(instance=instance, validated_data=validated_data)
        if self._skip_edit_permission_data[0]: # discard permission data e.g. `groups`, `roles`, `quota`
            subform_keys = ['emails','phones','locations']
        self._skip_edit_permission_data.remove(self._skip_edit_permission_data[0])
        for k in subform_keys:
            field = self.fields[k]
            subform_qset = getattr(instance, k).all()
            field.update(instance=subform_qset, validated_data=validated_subform_data[k],
                    usr=instance, allow_insert=True, allow_delete=True)
        return instance

#### end of  GenericUserProfileSerializer


class BulkGenericUserGroupSerializer(DjangoBaseClosureBulkSerializer):
    CLOSURE_MODEL_CLS     = GenericUserGroupClosureSerializer.Meta.model
    PK_FIELD_NAME         = GenericUserGroupClosureSerializer.Meta.model.id.field.name
    DEPTH_FIELD_NAME      = GenericUserGroupClosureSerializer.Meta.model.depth.field.name
    ANCESTOR_FIELD_NAME   = GenericUserGroupClosureSerializer.Meta.model.ancestor.field.name
    DESCENDANT_FIELD_NAME = GenericUserGroupClosureSerializer.Meta.model.descendant.field.name

    def update(self, instance, validated_data, **kwargs):
        instance = super().update(instance=instance, validated_data=validated_data , **kwargs)
        grp_ids = list(map(lambda obj:obj.id, instance))
        update_accounts_privilege.delay(affected_groups=grp_ids, deleted=False)
        return instance


class GenericUserGroupSerializer(BaseClosureNodeMixin, AbstractGenericUserSerializer):
    class Meta(BaseClosureNodeMixin.Meta, AbstractGenericUserSerializer.Meta):
        model = GenericUserGroup
        fields = ['id', 'name', 'ancestors', 'descendants', 'usr_cnt',]
        list_serializer_class = BulkGenericUserGroupSerializer

    ancestors   = GroupAncestorSerializer(many=True, read_only=True)
    descendants = GroupDescendantSerializer(many=True, read_only=True)
    usr_cnt = IntegerField(read_only=True)

    def validate_quota(self, value):
        # In order not to complicate the design, quota arrangements of a parent group will NOT be
        # inherited by all its children and descendant groups.
        # That is, if there's group A inherited by another group B, and a quota arrangement
        # `max-num-emails = 4` is applied to group A, then group B will NOT automatically have
        # the same quota arrangement.
        _final_quota = {v['material'] if isinstance(v['material'], int) else \
                v['material'].id: v['maxnum'] for v in value}
        self._instant_update_contact_quota(_final_quota)
        return value

    #### usr_cnt = SerializerMethodField() # don't use this, it cannot be reordered
    #### def get_usr_cnt(self, obj):
    ####     return obj.profiles.count()
## end of class GenericUserGroupSerializer



class BulkAuthUserRequestSerializer(BulkUpdateListSerializer):
    def update(self, instance, validated_data):
        instance = super().update(instance=instance, allow_insert=True,
                validated_data=validated_data)
        return instance


class AuthUserResetRequestSerializer(ExtendedModelSerializer, UserSubformSetupMixin):
    class Meta(ExtendedModelSerializer.Meta):
        model = AccountResetRequest
        fields = ['id', 'email', 'profile']
        list_serializer_class = BulkAuthUserRequestSerializer

    email = PrimaryKeyRelatedField(many=False, queryset=EmailSerializer.Meta.model.objects.none() )
    atomicity = _atomicity_fn

    def __init__(self, instance=None, data=empty, **kwargs):
        #### self.exc_rd_fields = kwargs.pop('exc_rd_fields', None)
        # Following variables will be used for mailing with user authentication link
        # the user auth link could be for (1) account activation (2) username reset
        # (3) password reset
        self._msg_template_path = kwargs.pop('msg_template_path', None)
        self._subject_template  = kwargs.pop('subject_template', None)
        self._url_host          = kwargs.pop('url_host', None)
        self._url_resource      = kwargs.pop('url_resource', None)
        if not data is empty:
            if isinstance(data, list):
                email_ids = [d['email'] for d in data if d.get('email',None)]
            else :
                email_ids = [data.get('email', '')]
            qset = EmailSerializer.Meta.model.objects.filter(id__in=email_ids)
            self.fields['email'].queryset = qset
        super().__init__(instance=instance, data=data, **kwargs)

    def to_representation(self, instance):
        data = super().to_representation(instance=instance)
        if hasattr(self, '_async_tasks_id'):
            # will work ONLY after saving validated data and issuing asynchronous mailing task
            # , the argument `instance` should be model instance
            task_id = self._async_tasks_id[instance.profile.pk]
            data['async_task'] = task_id
        return data

    def run_validation(self, data=empty):
        pk_condition = {'profile': data.get('profile', '')}
        return super().run_validation(data=data, pk_condition=pk_condition,
                set_null_if_obj_not_found=True)

    def extra_setup_before_validation(self, instance, data):
        self._mark_as_creation_on_update(pk_field_name='id', instance=instance, data=data)
        if instance:
            # don't use auth reuqest ID from clients even when they are provided, client
            # applications search auth-user reuqest instance using `user profile ID` instead.
            data['id'] = instance.pk

    def validate(self, value): # check if the given email ID is related to the user profile
        profile = value['profile']
        email   = value['email']
        cnt = profile.emails.filter(pk=email.pk).count()
        if cnt != 1:
            errmsg = "User doesn't have the chosen email"
            log_msg = ['errmsg', errmsg, 'email_id', email.pk, 'profile_id', profile.pk]
            _logger.warning(None, *log_msg)
            raise RestValidationError(errmsg)
        return value

    def create(self, validated_data):
        instance = super().create(validated_data=validated_data)
        self._mailing(validated_data=validated_data, req=instance)
        return instance

    def update(self, instance, validated_data):
        instance = super().update(instance=instance, validated_data=validated_data)
        self._mailing(validated_data=validated_data, req=instance)
        return instance

    def _mailing(self, validated_data, req):
        """
        get mail plaintext (instead of passing model instance as task argument),
        get mail template, and data to render, place all of them to task queue
        , the mailing process will be done asynchronously by another service program
        """
        profile = validated_data['profile']
        mail_ref = validated_data.get('email', None)
        if mail_ref is None: # skip if not provided
            return
        subject_data = {'first_name': profile.first_name}
        msg_data = {'first_name': profile.first_name, 'last_name': profile.last_name,
            'url_host': self._url_host, 'url_resource': self._url_resource, 'token':req.token,
            'expire_before': str(int(self.Meta.model.MAX_TOKEN_VALID_TIME / 60)),
        }
        to_addr = mail_ref.email.addr
        from_addr = django_settings.DEFAULT_FROM_EMAIL

        result = async_send_mail.delay(to_addrs=[to_addr], from_addr=from_addr,
                    subject_template=self._subject_template, subject_data=subject_data,
                    msg_template_path=self._msg_template_path,  msg_data=msg_data, )
        if not hasattr(self, '_async_tasks_id'):
            self._async_tasks_id = {}
        self._async_tasks_id[profile.pk] = result.task_id
        log_msg = ['_async_tasks_id', self._async_tasks_id]
        _logger.debug(None, *log_msg)

#### end of AuthUserResetRequestSerializer




# ----------------------------------------------------------------------
@deconstructible
class UsernameUniquenessValidator:
    """
    give model class and name of a field, check record uniqueness
    associated with giving value in __call__ function
    """
    def __init__(self, account):
        self._account = account or get_user_model()()

    def __call__(self, value):
        errmsg = None
        log_level = logging.INFO
        if self._account.pk and (self._account.username == value):
            errmsg = "your new username should be different from original one"
        else:
            backup = self._account.username
            self._account.username = value
            try: # invoke existing validator at model level
                self._account.validate_unique()
            except ValidationError as e:
                for item in e.error_dict.get(get_user_model().USERNAME_FIELD, None):
                    if item.message.find("exist") > 0:
                        errmsg = item.message
                        log_level = logging.WARNING
                        break
            self._account.username = backup
        if errmsg: #TODO: replace with RestValidationError
            log_msg = ['errmsg', errmsg, 'value', value, 'account_id', self._account.pk,
                    'account_username', self._account.username,]
            _logger.log(log_level, None, *log_msg)
            raise ValidationError(message=errmsg)


@deconstructible
class PasswordComplexityValidator:

    def __init__(self, account, password_confirm=None):
        self._account = account or get_user_model()()
        if not password_confirm is None:
            self._password_confirm = password_confirm

    def __call__(self, value):
        errs = []
        if hasattr(self, '_password_confirm'):
            if self._password_confirm != value:
                msg = "'password' field doesn't match 'confirm password' field."
                errs.append(ValidationError(message=msg))
        if re.search("[^\w]", value) is None:
            msg = "new password must contain at least one special symbol e.g. @, $, +, ...."
            errs.append(ValidationError(message=msg))
        try:
            password_validation.validate_password(value, self._account)
        except ValidationError as e:
            errs = errs + e.error_list
        if len(errs) > 0: #TODO: replace with RestValidationError
            log_msg = ['errs', errs]
            _logger.info(None, *log_msg)
            raise ValidationError(message=errs)


@deconstructible
class StringEqualValidator(validators.BaseValidator):
    def compare(self, a, b):
        return a != b

@deconstructible
class OldPasswdValidator(validators.BaseValidator):
    def compare(self, a, b):
        # check password without saving it
        return not check_password(password=a , encoded=b)


class  LoginAccountSerializer(Serializer):
    """
    There are case scenarios that will invoke this serializer :
        case #1: New users activate their own login account at the first time
        case #2: Unauthorized users forget their username, and request to reset
        case #3: Unauthorized users forget their password, and request to reset
        case #4: Authorized users change their username, within valid login session
        case #5: Authorized users change their password, within valid login session
        case #6: Login authentication
    """
    old_uname = CharField(required=True, max_length=128)
    old_passwd = CharField(required=True, max_length=128)
    username  = CharField(required=True, max_length=128, min_length=6, )
    password  = CharField(required=True, max_length=128, min_length=10,)
    password2 = CharField(required=True, max_length=128)

    # case #1: auth_req = non-null, account = null
    # case #2: auth_req = non-null, account = null, but can be derived from auth_req
    # case #3: auth_req = non-null, account = null, but can be derived from auth_req
    # case #4: auth_req = null, account = non-null
    # case #5: auth_req = null, account = non-null
    def __init__(self, data, account, auth_req, confirm_passwd=False, uname_required=False,
            old_uname_required=False,  old_passwd_required=False, passwd_required=False, **kwargs):
        self._auth_req = auth_req
        self._mail_kwargs = kwargs.pop('mail_kwargs',None)
        self.fields['username'].required = uname_required
        self.fields['password'].required = passwd_required
        self.fields['password2'].required = confirm_passwd
        self.fields['old_uname'].required = old_uname_required
        self.fields['old_passwd'].required = old_passwd_required
        log_msg = ['account', account]
        if account and isinstance(account, get_user_model()):
            old_uname_validator = StringEqualValidator(limit_value=account.username,
                     message="incorrect old username")
            old_passwd_validator = OldPasswdValidator(limit_value=account.password,
                     message="incorrect old password")
            self.fields['old_uname'].validators.append(old_uname_validator)
            self.fields['old_passwd'].validators.append(old_passwd_validator)
        elif auth_req:
            account = auth_req.profile.account
        else:
            errmsg = "caller must provide `account` or `auth_req`, both of them must NOT be null"
            log_msg.extend(['errmsg', errmsg])
            _logger.error(None, *log_msg)
            raise ImproperlyConfigured(errmsg)

        passwd2 = data.get('password2', '') if confirm_passwd  else None

        uname_validator  = UsernameUniquenessValidator(account=account)
        passwd_validator = PasswordComplexityValidator(account=account, password_confirm=passwd2)
        self.fields['username'].validators.append(uname_validator)
        self.fields['password'].validators.append(passwd_validator)
        _logger.debug(None, *log_msg)
        super().__init__(instance=account, data=data, **kwargs)


    def _clean_validate_only_fields(self, validated_data):
        for key in ['password2','old_uname','old_passwd']:
            validated_data.pop(key, None)
        log_msg = ['validated_data', validated_data]
        _logger.debug(None, *log_msg)
        return validated_data

    def create(self, validated_data):
        profile = self._auth_req.profile
        email   = self._auth_req.email
        validated_data = self._clean_validate_only_fields(validated_data)
        with _atomicity_fn():
            instance = profile.activate(new_account_data=validated_data)
            self._auth_req.delete()
        if self._mail_kwargs and email: # notify user again by email
            self._mailing(profile=profile, mail_ref=email, username=instance.username)
        return instance

    def update(self, instance, validated_data):
        profile = None
        email   = None
        validated_data = self._clean_validate_only_fields(validated_data)
        with _atomicity_fn():
            for attr, value in validated_data.items():
                if attr == "password":
                    instance.set_password(raw_password=value)
                else:
                    setattr(instance, attr, value)
            instance.save() # password will be hashed in AuthUser model before save
            if self._auth_req:
                profile = self._auth_req.profile
                email   = self._auth_req.email
                self._auth_req.delete()
            # check instance.username and instance.password if necessary
        if self._mail_kwargs and email:
            self._mailing(profile=profile, mail_ref=email, username=instance.username)
        return instance


    def _mailing(self, profile, mail_ref, username):
        event_time = datetime.now(timezone.utc)
        masked_username = username[:3]
        msg_data = {'first_name': profile.first_name, 'last_name': profile.last_name,
            'event_time': event_time, 'masked_username': masked_username,
        }
        to_addr = mail_ref.email.addr
        from_addr = django_settings.DEFAULT_FROM_EMAIL

        result = async_send_mail.delay(to_addrs=[to_addr], from_addr=from_addr,
                    subject_template=self._mail_kwargs['subject_template'],
                    msg_template_path=self._mail_kwargs['msg_template_path'],
                    msg_data=msg_data, )
        if not hasattr(self, '_async_tasks_id'):
            self._async_tasks_id = {}
        self._async_tasks_id[profile.pk] = result.task_id

#### end of LoginAccountSerializer


