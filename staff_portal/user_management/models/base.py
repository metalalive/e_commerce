import secrets
import hashlib
import logging
from functools import partial

from datetime import datetime, timezone, timedelta

from django.db     import  models, IntegrityError, transaction
from django.db.models.manager   import Manager
from django.db.models.constants import LOOKUP_SEP
from django.core.validators import RegexValidator
from django.core.exceptions import EmptyResultSet, ObjectDoesNotExist, MultipleObjectsReturned
from django.contrib.contenttypes.models  import ContentType
from django.contrib.contenttypes.fields  import GenericForeignKey, GenericRelation

from softdelete.models import SoftDeleteObjectMixin

from common.util.python          import merge_partial_dup_listitem
from common.models.constants     import ROLE_ID_SUPERUSER, ROLE_ID_STAFF
from common.models.mixins        import MinimumInfoMixin, SerializableMixin
from common.models.closure_table import ClosureTableModelMixin, get_paths_through_processing_node, filter_closure_nodes_recovery
from common.models.fields   import CompoundPrimaryKeyField


from .common import _atomicity_fn, UsermgtChangeSet, UsermgtSoftDeleteRecord, AppCodeOptions
from django.contrib import auth


_logger = logging.getLogger(__name__)



class AbstractUserRelation(models.Model):
    class Meta:
        abstract = True
    allowed_models = models.Q(app_label='user_management', model='GenericUserProfile') | \
                     models.Q(app_label='user_management', model='GenericUserGroup')
    user_type = models.ForeignKey(to=ContentType, on_delete=models.CASCADE, null=False,
                                  db_column='user_type',  limit_choices_to=allowed_models)
    user_id   = models.PositiveIntegerField(db_column='user_id')
    user_ref  = GenericForeignKey(ct_field='user_type', fk_field='user_id')


# one email address belongs to a single user, or a single user group
class EmailAddress(AbstractUserRelation, SoftDeleteObjectMixin):
    """
    use cases of mailing :
        -- for individual user (not user group)
        * create, or delete entire user profile (including login account)
        * user activates, or deactivates their login account
        * user reset username / password of their login account
        -- for both of individual user and user group
        * employee makes a purchase order to their supplier(s)
        * customer makes, discards, or partially return a sale order
        * customer makes payment
        * customer gets full / partial refund
        * customer books an item (e.g. table, room)
        * customer cancels their booking
        * customer in waitlist can book items released by someone else who gives up
    """
    SOFTDELETE_CHANGESET_MODEL = UsermgtChangeSet
    SOFTDELETE_RECORD_MODEL = UsermgtSoftDeleteRecord
    class Meta:
        db_table = 'email_address'
    # each user may have more than one email addresses or phone numbers (or none)
    addr = models.EmailField(max_length=160, default="notprovide@localhost", blank=False, null=False, unique=False)



class PhoneNumber(AbstractUserRelation):
    class Meta:
        db_table = 'phone_number'
    ccode_validator   = RegexValidator(regex=r"^\d{1,3}$", message="non-digit character detected, or length of digits doesn't meet requirement. It must contain only digit e.g. '91', '886' , from 1 digit up to 3 digits")
    linenum_validator = RegexValidator(regex=r"^\+?1?\d{7,15}$", message="non-digit character detected, or length of digits doesn't meet requirement. It must contain only digits e.g. '9990099', from 7 digits up to 15 digits")
    country_code = models.CharField(max_length=3,  validators=[ccode_validator],   unique=False)
    line_number  = models.CharField(max_length=15, validators=[linenum_validator], unique=False)



# it is possible that one user has multiple address locations to record
class GeoLocation(AbstractUserRelation):
    """
    record locations for generic business operations,
    e.g.
    your businuss may need to record :
     * one or more outlets, for selling goods to end customers
     * warehouse, you either rent a space from others, or build your own
     * factory, if your finished goods is manufactured by your own company
     * farm, in case your company contracts farmers who grow produce (raw
       materials) for product manufacture.
     * shipping addresses of customers and suppliers
    """
    class Meta:
        db_table = 'geo_location'

    class CountryCode(models.TextChoices):
        AU = 'AU',
        AT = 'AT',
        CZ = 'CZ',
        DE = 'DE',
        HK = 'HK',
        IN = 'IN',
        ID = 'ID',
        IL = 'IL',
        MY = 'MY',
        NZ = 'NZ',
        PT = 'PT',
        SG = 'SG',
        TH = 'TH',
        TW = 'TW',
        US = 'US',

    id = models.AutoField(primary_key=True,)

    country = models.CharField(name='country', max_length=2, choices=CountryCode.choices, default=CountryCode.TW,)
    province    = models.CharField(name='province', max_length=50,) # name of the province
    locality    = models.CharField(name='locality', max_length=50,) # name of the city or town
    street      = models.CharField(name='street',   max_length=50,) # name of the road, street, or lane
    # extra detail of the location, e.g. the name of the building, which floor, etc.
    # Note each record in this database table has to be mapped to a building of real world
    detail      = models.CharField(name='detail', max_length=100,)
    # if
    # floor =  0, that's basement B1 floor
    # floor = -1, that's basement B2 floor
    # floor = -2, that's basement B3 floor ... etc
    floor = models.SmallIntegerField(default=1, blank=True, null=True)
    # simple words to describe what you do in the location for your business
    description = models.CharField(name='description', blank=True, max_length=100,)

    def __str__(self):
        out = ["Nation:", None, ", city/town:", None,]
        out[1] = self.country
        out[3] = self.locality
        return "".join(out)
## end of class GeoLocation


class QuotaMaterial(models.Model):
    """
    In quota arrangement, material simply represents source of supply,
    e.g.
    number of resources like database table rows, memory space, bus interconnect
    other pieces of hardware ... etc. which can be used by individual user or group.

    * Quota arrangement is about restricting users' access to resources,  it makes sense
      to maintain the material types and the arrangement of each user in this user
      management application.
    * For any other application implemented with different backend framework or different
      language, each application can add / edit / delete a set of new material types (used
      under the scope of the application) to this model, by invoking internal RPC function
      during schema migration.
    * This model is NOT for frontend client that attempts to modify the content of this model
    """
    class Meta:
        db_table = 'quota_material'

    class _MatCodeOptions(models.IntegerChoices):
        MAX_NUM_EMAILS = 1
        MAX_NUM_PHONE_NUMBERS = 2
        MAX_NUM_GEO_LOCATIONS = 3

    app_code = models.PositiveSmallIntegerField(null=False, blank=False, choices=AppCodeOptions.choices)
    mat_code = models.PositiveSmallIntegerField(null=False, blank=False)

    @classmethod
    def get_for_apps(cls, app_labels):
        app_labels = app_labels or []
        label_to_code_fn = lambda app_label: getattr(AppCodeOptions, app_label).value
        _appcodes = tuple(map(label_to_code_fn , app_labels))
        return cls.objects.filter(app_code__in=_appcodes)


class UserQuotaRelation(AbstractUserRelation, SoftDeleteObjectMixin):
    """ where the system stores quota arrangements for each user (or user group) """
    class Meta:
        db_table = 'user_quota_relation'
    SOFTDELETE_CHANGESET_MODEL = UsermgtChangeSet
    SOFTDELETE_RECORD_MODEL = UsermgtSoftDeleteRecord

    material = models.ForeignKey(to=QuotaMaterial, on_delete=models.CASCADE, null=False,
                      blank=False, db_column='material', related_name='usr_relations')
    maxnum   = models.PositiveSmallIntegerField(default=1)
    expiry   = models.DateTimeField(blank=True, null=True)
    id = CompoundPrimaryKeyField(inc_fields=['user_type', 'user_id', 'material'])


class GenericUserGroup(SoftDeleteObjectMixin, MinimumInfoMixin):
    SOFTDELETE_CHANGESET_MODEL = UsermgtChangeSet
    SOFTDELETE_RECORD_MODEL = UsermgtSoftDeleteRecord
    min_info_field_names = ['id','name']

    class Meta(SoftDeleteObjectMixin.Meta):
        db_table = 'generic_user_group'

    name  = models.CharField(max_length=50,  unique=False)
    # foreign key referencing to the same table
    #### parent = models.ForeignKey('self', db_column='parent', on_delete=models.CASCADE, null=True, blank=True)
    #roles = models.ManyToManyField(auth.models.Group, blank=True, db_table='generic_group_auth_role', related_name='user_groups')
    roles = GenericRelation('GenericUserAppliedRole', object_id_field='user_id', content_type_field='user_type')
    quota     = GenericRelation(UserQuotaRelation, object_id_field='user_id', content_type_field='user_type')
    emails    = GenericRelation(EmailAddress, object_id_field='user_id', content_type_field='user_type')
    phones    = GenericRelation(PhoneNumber,  object_id_field='user_id', content_type_field='user_type')
    locations = GenericRelation(GeoLocation,  object_id_field='user_id', content_type_field='user_type')

    @_atomicity_fn()
    def delete(self, *args, **kwargs):
        del_grp_id = self.pk
        hard_delete = kwargs.get('hard', False)
        if not hard_delete:
            if kwargs.get('changeset', None) is None:
                profile_id = kwargs.get('profile_id')
                kwargs['changeset'] = self.determine_change_set(profile_id=profile_id)
        self._decrease_subtree_pathlen(*args, **kwargs)
        # delete this node
        SoftDeleteObjectMixin.delete(self, *args, **kwargs)
        if not hard_delete: # logs the soft-deleted instance
            # all GenericRelation instances e.g. roles, quotas, will NOT be soft-deleted automatically,
            # instead developers have to soft-delete them explicitly by calling
            # Model.delete() or QuerySet.delete()
            self.roles.all().delete(*args, **kwargs)
            self.quota.all().delete(*args, **kwargs)
            self.emails.all().delete(*args, **kwargs)
            changeset = kwargs.pop('changeset', None)
            kwargs.pop('profile_id', None)
            self.phones.all().delete(*args, **kwargs)
            self.locations.all().delete(*args, **kwargs)
            if _logger.level <= logging.DEBUG:
                del_set_exist = type(self).objects.get_deleted_set().filter(pk=del_grp_id).exists()
                cond = models.Q(ancestor=del_grp_id) | models.Q(descendant=del_grp_id)
                del_paths_qset = GenericUserGroupClosure.objects.get_deleted_set().filter(cond)
                del_paths_qset = del_paths_qset.values('pk', 'ancestor__pk', 'descendant__pk', 'depth')
                cset_records = changeset.soft_delete_records.all().values('pk', 'content_type__pk', 'object_id')
                log_args = ['changeset_id', changeset.pk, 'del_grp_id', del_grp_id, 'del_set_exist', del_set_exist,
                        'del_paths_qset', del_paths_qset, 'cset_records', cset_records]
                _logger.debug(None, *log_args)
        #raise IntegrityError


    @_atomicity_fn()
    def undelete(self, *args, **kwargs):
        if kwargs.get('changeset', None) is None:
            profile_id = kwargs.get('profile_id',None)
            kwargs['changeset'] = self.determine_change_set(profile_id=profile_id, create=False)
        changeset_id = kwargs['changeset'].pk
        # recover this node first
        status = super().undelete(*args, **kwargs)
        # then recover all its deleted relations,
        if status is SoftDeleteObjectMixin.DONE_FULL_RECOVERY:
            self._increase_subtree_pathlen(*args, **kwargs)
        log_args = ['changeset_id', changeset_id, 'undel_grp_id', self.pk,]
        _logger.debug(None, *log_args)
        kwargs.pop('changeset', None)
        kwargs.pop('profile_id', None)
        #raise IntegrityError

    @get_paths_through_processing_node(with_deleted=False)
    def _decrease_subtree_pathlen(self, affected_paths):
        affected_paths_log = affected_paths.values('pk','ancestor__pk','descendant__pk','depth')
        log_args = ['affected_paths', affected_paths_log]
        for a in affected_paths:
            if a.depth > 1:
                a.depth -= 1
            else:
                _logger.error(None, *log_args)
                raise IntegrityError
        affected_paths.model.objects.bulk_update(affected_paths ,['depth'])
        _logger.debug(None, *log_args)

    @get_paths_through_processing_node()
    def _increase_subtree_pathlen(self, affected_paths):
        affected_paths_log = affected_paths.values('pk','ancestor__pk','descendant__pk','depth')
        log_args = ['affected_paths', affected_paths_log]
        for a in affected_paths:
            if a.depth < 1:
                _logger.error(None, *log_args)
                raise IntegrityError
            else:
                a.depth += 1
        affected_paths.model.objects.bulk_update(affected_paths ,['depth'])
        _logger.debug(None, *log_args)
        # TODO: check whether the undelete process will cause data corruption in application
        #### a.undelete(*args, **kwargs)

    def filter_before_recover(self, records_in):
        return filter_closure_nodes_recovery(records_in=records_in, app_label='user_management',
                model_name='GenericUserGroupClosure')



class GenericUserGroupClosure(ClosureTableModelMixin, SoftDeleteObjectMixin):
    """ closure table to describe tree structure of user group hierarchies """
    SOFTDELETE_CHANGESET_MODEL = UsermgtChangeSet
    SOFTDELETE_RECORD_MODEL = UsermgtSoftDeleteRecord
    class Meta(ClosureTableModelMixin.Meta):
        db_table = 'generic_user_group_closure'

    ancestor   = ClosureTableModelMixin.asc_field(ref_cls=GenericUserGroup)
    descendant = ClosureTableModelMixin.desc_field(ref_cls=GenericUserGroup)



class GenericUserProfile(SoftDeleteObjectMixin, SerializableMixin, MinimumInfoMixin):
    SOFTDELETE_CHANGESET_MODEL = UsermgtChangeSet
    SOFTDELETE_RECORD_MODEL = UsermgtSoftDeleteRecord
    NONE = 0
    SUPERUSER = ROLE_ID_SUPERUSER
    STAFF = ROLE_ID_STAFF
    min_info_field_names = ['id','first_name','last_name']

    class Meta(SoftDeleteObjectMixin.Meta):
        db_table = 'generic_user_profile'

    first_name = models.CharField(max_length=32, blank=False, unique=False)
    last_name  = models.CharField(max_length=32, blank=False, unique=False)
    # users may be active or not (e.g. newly registered users who haven't activate their account)
    active = models.BooleanField(default=False)
    time_created = models.DateTimeField(auto_now_add=True)
    # record last time this user used (or logined to) the system
    last_updated = models.DateTimeField(auto_now=True)
    # the group(s) the user belongs to
    ####groups = models.ManyToManyField(GenericUserGroup, blank=True, db_table='generic_user_group_relation', related_name='user_profiles')
    roles = GenericRelation('GenericUserAppliedRole', object_id_field='user_id', content_type_field='user_type')
    # reverse relations from related models e.g. emails / phone numbers / geographical locations
    quota     = GenericRelation(UserQuotaRelation, object_id_field='user_id', content_type_field='user_type')
    emails    = GenericRelation(EmailAddress, object_id_field='user_id', content_type_field='user_type')
    phones    = GenericRelation(PhoneNumber,  object_id_field='user_id', content_type_field='user_type')
    locations = GenericRelation(GeoLocation,  object_id_field='user_id', content_type_field='user_type')

    @_atomicity_fn()
    def delete(self, *args, **kwargs):
        del_prof_id = self.pk
        hard_delete = kwargs.get('hard', False)
        if not hard_delete: # let nested fields add in the same soft-deleted changeset
            if kwargs.get('changeset', None) is None:
                profile_id = kwargs.get('profile_id')
                kwargs['changeset'] = self.determine_change_set(profile_id=profile_id)
        self.deactivate(remove_account=False)
        super().delete(*args, **kwargs)
        if not hard_delete:
            # emails are soft-deleted for potential login account re-activation in the future
            self.emails.all().delete(*args, **kwargs)
            self.roles.all().delete(*args, **kwargs) # self.groups.all() soft-deleted automatically
            kwargs.pop('hard', False)
            kwargs.pop('profile_id', None)
            changeset = kwargs.pop('changeset', None)
            # Sensitive personal data like phones and geo-locations must be hard-deleted.
            self.phones.all().delete(*args, **kwargs)
            self.locations.all().delete(*args, **kwargs)
            if _logger.level <= logging.DEBUG:
                del_set_exist = type(self).objects.get_deleted_set().filter(pk=del_prof_id).exists()
                phones_exist = self.phones.all().exists()
                geoloc_exist = self.locations.all().exists()
                quota_count  = self.quota.count() # quota is not soft-delete model, should remain unchanged
                cset_records = changeset.soft_delete_records.all().values('pk', 'content_type__pk', 'object_id')
                log_args = ['del_prof_id', del_prof_id, 'del_set_exist', del_set_exist, 'cset_records', cset_records,
                        'changeset_id', changeset.pk, 'phones_exist', phones_exist, 'geoloc_exist', geoloc_exist,
                        'quota_count', quota_count]
                _logger.debug(None, *log_args)
        #raise IntegrityError("end of complex deletion ........")


    @_atomicity_fn()
    def undelete(self, *args, **kwargs):
        if kwargs.get('changeset', None) is None:
            profile_id = kwargs.get('profile_id',None)
            kwargs['changeset'] = SoftDeleteObjectMixin.determine_change_set(self, profile_id=profile_id, create=False)
        changeset_id = kwargs['changeset'].pk
        undel_prof_id = self.pk
        status = super().undelete(*args, **kwargs)
        kwargs.pop('changeset', None)
        kwargs.pop('profile_id', None)
        log_args = ['undel_prof_id', undel_prof_id, 'changeset_id', changeset_id, 'status', status]
        _logger.debug(None, *log_args)
        #raise IntegrityError("end of complex recover ........")


    @_atomicity_fn()
    def activate(self, new_account_data):
        account = None
        self.active = True
        self.save(update_fields=['active'])
        try:
            auth_rel = self.auth
            account  = auth_rel.login
            account.is_active = True
            account.save()
        except ObjectDoesNotExist: # for first time to activate the account
            new_account_data['profile'] = self
            account = auth.get_user_model().objects.create_user( **new_account_data )
            GenericUserProfile.update_account_privilege(profile=self, account=account)
        return account


    @_atomicity_fn()
    def deactivate(self, remove_account=False):
        log_args = []
        if not self.pk:
            err_msg = "cannot deactivate user instance that hasn't been created yet"
            log_args.extend(['msg', err_msg])
            _logger.error(None, *log_args)
            raise ValueError(err_msg)
        self.active = False
        self.save(update_fields=['active'])
        try:
            req = self.auth_rst_req
            req.delete()
        except ObjectDoesNotExist:
            log_args.extend(['msg', 'no auth-related request issued to the user'])
        try:
            auth_rel = self.auth
            account  = auth_rel.login
            auth_rel.check_admin_exist()
            if remove_account:
                auth_rel.delete()
            else:
                account.is_active = False
                account.save()
        except ObjectDoesNotExist:
            log_args.extend(['msg', 'no login account for the user'])
        if any(log_args):
            log_args.extend(['profile_id', self.pk])
            _logger.info(None, *log_args)

    @classmethod
    def estimate_inherit_quota(cls, groups):
        """
        the argument `groups` is iterable object of GenericUserGroup objects
        """
        #['ancestors__ancestor__id',
        # 'ancestors__ancestor__quota__material__app_code',
        # 'ancestors__ancestor__quota__material__mat_code'
        # 'ancestors__ancestor__quota__maxnum' ]
        quota_mat_field = ['ancestors', 'ancestor', 'quota', 'material'] # QuotaMaterial.id field
        quota_val_field = ['ancestors', 'ancestor', 'quota', 'maxnum']
        quota_mat_field = LOOKUP_SEP.join(quota_mat_field)
        quota_val_field = LOOKUP_SEP.join(quota_val_field)
        qset = groups.values(quota_mat_field).annotate(max_num_chosen=models.Max(quota_val_field)
            ).order_by(quota_mat_field).distinct().filter(max_num_chosen__gt=0)
        merged_inhehited = {q[quota_mat_field]: q['max_num_chosen'] for q in qset}
        log_msg = ['merged_inhehited', merged_inhehited]
        _logger.debug(None, *log_msg)
        return merged_inhehited

    @property
    def inherit_quota(self):
        # this only returns quota value added to the groups inherited by the user
        grp_ids = self.groups.values_list('group__pk', flat=True)
        grp_cls = self.groups.model.group.field.related_model
        groups  = grp_cls.objects.filter(pk__in=grp_ids)
        merged_inherited = type(self).estimate_inherit_quota(groups=groups)
        return merged_inherited

    @property
    def all_quota(self):
        _all_quota = self.inherit_quota
        qset = self.quota.values('material', 'maxnum')
        for item in qset:
            v1 = _all_quota.get(item['material'], -1)
            if v1 < item['maxnum']:
                _all_quota[item['material']] = item['maxnum']
        return _all_quota

    @property
    def inherit_roles(self):
        if not hasattr(self, '_inherit_roles'):
            #self._inherit_roles = [ra.role for grpa in self.groups.all()  for asc in grpa.group.ancestors.all()
            #    for ra in asc.ancestor.roles.all() ]
            grp_ct = ContentType.objects.get_for_model(GenericUserGroup)
            grp_ids = self.groups.values_list('group__pk', flat=True)
            asc_ids = GenericUserGroupClosure.objects.filter(descendant__pk__in=grp_ids).values_list('ancestor__id', flat=True)
            role_rel_cls = self.roles.model
            role_ids = role_rel_cls.objects.filter(user_type=grp_ct, user_id__in=asc_ids).values_list('role__id', flat=True)
            role_cls = role_rel_cls.role.field.related_model
            roles = role_cls.objects.filter(id__in=role_ids)
            self._inherit_roles = roles
        return self._inherit_roles

    @property
    def direct_roles(self):
        if not hasattr(self, '_direct_roles'):
            role_rel_cls = self.roles.model
            role_ids = self.roles.values_list('role__id', flat=True)
            role_cls = role_rel_cls.role.field.related_model
            roles = role_cls.objects.filter(id__in=role_ids)
            self._direct_roles = roles
        return self._direct_roles

    @property
    def all_roles(self):
        return {'direct':self.direct_roles, 'inherit':self.inherit_roles}


    @property
    def privilege_status(self):
        out = self.NONE
        role_ids = (ROLE_ID_SUPERUSER, ROLE_ID_STAFF)
        all_roles = self.all_roles
        for qset in all_roles.values():
            fetched_ids = qset.filter(id__in=role_ids).values_list('id', flat=True)
            if ROLE_ID_SUPERUSER in fetched_ids:
                out = self.SUPERUSER
                break
            elif ROLE_ID_STAFF in fetched_ids:
                out = self.STAFF
                break
        return out

    @classmethod
    def update_account_privilege(cls, profile, account):
        """
        private class method to update privilege (is_superuser, is_staff flags)
        in django.contrib.auth.User account, note this function is NOT thread-safe
        """
        if (profile is None) or (account is None):
            return
        privilege = profile.privilege_status
        if privilege == cls.SUPERUSER:
            account.is_superuser = True
            account.is_staff = True
        elif privilege == cls.STAFF:
            account.is_superuser = False
            account.is_staff = True
        else: # TODO, force such users logout if they were staff before the change
            account.is_superuser = False
            account.is_staff = False
        account.save(update_fields=['is_superuser','is_staff'])

        # update roles applied,
        new_roles = profile.all_roles
        old_role_relations = account.roles_applied.all()
        old_roles = old_role_relations.values_list('role__pk', flat=True)

        new_roles = set(new_roles)
        old_roles = set(old_roles)
        list_add = new_roles - old_roles
        list_del = old_roles - new_roles
        list_unchanged = old_roles & new_roles

        if list_add:
            role_model_cls =  profile.roles.model.role.field.related_model
            _list_add = role_model_cls.objects.filter(pk__in=list_add)
            #for role in _list_add:
            #    u = LoginAccountRoleRelation.objects.create(role=role, account=account)
        if list_del:
            old_role_relations.filter(role__pk__in=list_del).delete()
        # account.roles_applied.set(update_list) # completely useless if not m2m field
        log_args = ['profile_id', profile.pk, 'privilege', privilege, 'new_roles', new_roles,
                'old_roles', old_roles, 'list_add', list_add, 'list_del', list_del,
                'list_unchanged', list_unchanged]
        _logger.debug(None, *log_args)
#### end of GenericUserProfile




class GenericUserAppliedRole(AbstractUserRelation, SoftDeleteObjectMixin):
    SOFTDELETE_CHANGESET_MODEL = UsermgtChangeSet
    SOFTDELETE_RECORD_MODEL = UsermgtSoftDeleteRecord
    class Meta:
        db_table = 'generic_user_applied_role'

    # TODO:
    # * set unique constraint on each pair of (role, user/group)
    # * in case the staff who approved these role requests are deleted, the approved_by field should be
    #   modified to default superuser. So  a profile for default superuser will be necessary
    role = models.ForeignKey('Role', blank=False, db_column='role', related_name='users_applied',
                on_delete=models.CASCADE,)
    # the approvement should expire after the given time passed
    expiry = models.DateTimeField(blank=True, null=True)
    # record the user who approved the reqeust (that a role can be granted to the group or individual user
    approved_by  = models.ForeignKey(GenericUserProfile, blank=True, null=True, db_column='approved_by',
        related_name="approval_role",  on_delete=models.SET_NULL,)
    id = CompoundPrimaryKeyField(inc_fields=['user_type', 'user_id', 'role'])


class GenericUserGroupRelation(SoftDeleteObjectMixin):
    SOFTDELETE_CHANGESET_MODEL = UsermgtChangeSet
    SOFTDELETE_RECORD_MODEL = UsermgtSoftDeleteRecord
    class Meta:
        db_table = 'generic_user_group_relation'
    group   = models.ForeignKey(GenericUserGroup, blank=False, on_delete=models.CASCADE, db_column='group', related_name='profiles')
    profile = models.ForeignKey(GenericUserProfile, blank=False, on_delete=models.CASCADE, db_column='profile', related_name='groups')
    approved_by  = models.ForeignKey(GenericUserProfile, blank=True, null=True, db_column='approved_by',
        related_name="approval_group",  on_delete=models.SET_NULL,)
    id = CompoundPrimaryKeyField(inc_fields=['group','profile'])


