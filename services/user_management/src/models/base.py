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
from django.utils import timezone as django_timezone

from softdelete.models import SoftDeleteObjectMixin

from ecommerce_common.util   import merge_partial_dup_listitem
from ecommerce_common.models.constants     import ROLE_ID_SUPERUSER, ROLE_ID_STAFF
from ecommerce_common.models.enums.django  import JsonFileChoicesMeta
from ecommerce_common.models.mixins        import MinimumInfoMixin
from ecommerce_common.models.closure_table import ClosureTableModelMixin, get_paths_through_processing_node, filter_closure_nodes_recovery
from ecommerce_common.models.fields   import CompoundPrimaryKeyField


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


class _ExpiryFieldMixin(models.Model):
    class Meta:
        abstract = True
    # the approvement should expire after the given time passed
    expiry = models.DateTimeField(blank=True, null=True)

    @classmethod
    def expiry_condition(self, field_name:list=None):
        field_name = field_name or ['expiry']
        field_gte    = field_name.copy()
        field_isnull = field_name.copy()
        field_gte.append('gte')
        field_isnull.append('isnull')
        field_gte    = LOOKUP_SEP.join(field_gte   )
        field_isnull = LOOKUP_SEP.join(field_isnull)
        now_time = django_timezone.now()
        return (models.Q(**{field_gte:now_time}) | models.Q(**{field_isnull:True}))


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

    def __str__(self):
        return 'Quota material ID %s' % self.id


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
    quota_material = QuotaMaterial._MatCodeOptions.MAX_NUM_EMAILS
    class Meta:
        db_table = 'email_address'
    # each user may have more than one email addresses or phone numbers (or none)
    addr = models.EmailField(max_length=160, default="notprovide@localhost", blank=False, null=False, unique=False)



class PhoneNumber(AbstractUserRelation):
    class Meta:
        db_table = 'phone_number'
    quota_material = QuotaMaterial._MatCodeOptions.MAX_NUM_PHONE_NUMBERS
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
    quota_material = QuotaMaterial._MatCodeOptions.MAX_NUM_GEO_LOCATIONS

    class CountryCode(models.TextChoices, metaclass=JsonFileChoicesMeta):
        filepath = './common/data/nationality_code.json'

    id = models.AutoField(primary_key=True,)

    country = models.CharField(name='country', max_length=2, choices=CountryCode.choices, default=CountryCode.TW,)
    province = models.CharField(name='province', max_length=50,) # name of the province
    locality = models.CharField(name='locality', max_length=50,) # name of the city or town
    street   = models.CharField(name='street',   max_length=50,) # name of the road, street, or lane
    # extra detail of the location, e.g. the name of the building, which floor, etc.
    # Note each record in this database table has to be mapped to a building of real world
    detail   = models.CharField(name='detail', max_length=100,)
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


class UserQuotaRelation(AbstractUserRelation, SoftDeleteObjectMixin, _ExpiryFieldMixin):
    """ where the system stores quota arrangements for each user (or user group) """
    class Meta:
        db_table = 'user_quota_relation'
    SOFTDELETE_CHANGESET_MODEL = UsermgtChangeSet
    SOFTDELETE_RECORD_MODEL = UsermgtSoftDeleteRecord

    material = models.ForeignKey(to=QuotaMaterial, on_delete=models.CASCADE, null=False,
                      blank=False, db_column='material', related_name='usr_relations')
    maxnum   = models.PositiveSmallIntegerField(default=1)
    id = CompoundPrimaryKeyField(inc_fields=['user_type', 'user_id', 'material'])


class GenericUserCommonFieldsMixin(SoftDeleteObjectMixin):
    SOFTDELETE_CHANGESET_MODEL = UsermgtChangeSet
    SOFTDELETE_RECORD_MODEL = UsermgtSoftDeleteRecord
    class Meta:
        abstract = True
    roles = GenericRelation('GenericUserAppliedRole', object_id_field='user_id', content_type_field='user_type')
    # reverse relations from related models e.g. emails / phone numbers / geographical locations
    quota     = GenericRelation(UserQuotaRelation, object_id_field='user_id', content_type_field='user_type')
    emails    = GenericRelation(EmailAddress, object_id_field='user_id', content_type_field='user_type')
    phones    = GenericRelation(PhoneNumber,  object_id_field='user_id', content_type_field='user_type')
    locations = GenericRelation(GeoLocation,  object_id_field='user_id', content_type_field='user_type')

    @_atomicity_fn()
    def delete(self, *args, **kwargs):
        hard_delete = kwargs.get('hard', False)
        if not hard_delete:
            if kwargs.get('changeset', None) is None:
                profile_id = kwargs.get('profile_id')
                kwargs['changeset'] = self.determine_change_set(profile_id=profile_id)
        # delete this node
        super().delete(*args, **kwargs)
        if not hard_delete: # logs the soft-deleted instance
            # all GenericRelation instances e.g. roles, quotas, will NOT be soft-deleted automatically,
            # instead developers have to soft-delete them explicitly by calling
            # Model.delete() or QuerySet.delete()
            self.roles.all().delete(*args, **kwargs)
            self.quota.all().delete(*args, **kwargs)
            self.emails.all().delete(*args, **kwargs)
            changeset = kwargs.pop('changeset', None)
            kwargs.pop('profile_id', None)
            # Sensitive personal data like phones and geo-locations must be hard-deleted.
            self.phones.all().delete(*args, **kwargs)
            self.locations.all().delete(*args, **kwargs)
            return changeset

    @_atomicity_fn()
    def undelete(self, *args, **kwargs):
        if kwargs.get('changeset', None) is None:
            profile_id = kwargs.get('profile_id',None)
            kwargs['changeset'] = self.determine_change_set(profile_id=profile_id, create=False)
        changeset_id = kwargs['changeset'].pk
        # recover this node first
        status = super().undelete(*args, **kwargs)
        kwargs.pop('changeset', None)
        kwargs.pop('profile_id', None)
        log_args = ['changeset_id', changeset_id, 'status', status, 'obj_id', self.pk,
                'obj_type', str(type(self))]
        _logger.debug(None, *log_args)
        return status



class GenericUserGroup(GenericUserCommonFieldsMixin, MinimumInfoMixin):
    min_info_field_names = ['id','name']

    class Meta(SoftDeleteObjectMixin.Meta):
        db_table = 'generic_user_group'

    name  = models.CharField(max_length=50,  unique=False)
    # foreign key referencing to the same table
    #### parent = models.ForeignKey('self', db_column='parent', on_delete=models.CASCADE, null=True, blank=True)

    @_atomicity_fn()
    def delete(self, *args, **kwargs):
        del_grp_id = self.pk
        self._decrease_subtree_pathlen(*args, **kwargs)
        profile_id  = kwargs.get('profile_id', None)
        hard_delete = kwargs.get('hard', False)
        changeset = super().delete(*args, **kwargs)
        if not hard_delete: # logs the soft-deleted instance
            if 'profile_id' not in kwargs.keys():
                kwargs['profile_id'] = profile_id
            self.profiles.all().delete(*args, changeset=changeset, **kwargs)
            if _logger.level <= logging.DEBUG:
                del_set_exist = type(self).objects.get_deleted_set().filter(pk=del_grp_id).exists()
                cond = models.Q(ancestor=del_grp_id) | models.Q(descendant=del_grp_id)
                del_paths_qset = GenericUserGroupClosure.objects.get_deleted_set().filter(cond)
                del_paths_qset = del_paths_qset.values('pk', 'ancestor__pk', 'descendant__pk', 'depth')
                cset_records = changeset.soft_delete_records.all().values('pk', 'content_type__pk', 'object_id')
                log_args = ['changeset_id', changeset.pk, 'del_grp_id', del_grp_id, 'del_set_exist', del_set_exist,
                        'del_paths_qset', del_paths_qset, 'cset_records', cset_records]
                _logger.debug(None, *log_args)


    @_atomicity_fn()
    def undelete(self, *args, **kwargs):
        status = super().undelete(*args, **kwargs)
        # then recover all its deleted relations,
        if status is SoftDeleteObjectMixin.DONE_FULL_RECOVERY:
            self._increase_subtree_pathlen(*args, **kwargs)

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

    @classmethod
    def get_profiles_under_groups(cls, grp_ids, deleted):
        affected_groups_origin = grp_ids
        if deleted:
            qset = GenericUserGroupClosure.objects.get_deleted_set()
        else:
            qset = GenericUserGroupClosure.objects.all()
        qset = qset.filter(ancestor__pk__in=grp_ids)
        affected_groups = qset.values_list('descendant__pk', flat=True)
        # always update privilege in all user accounts including deactivated accounts
        kwargs_prof = {'group__pk__in': affected_groups, 'with_deleted':True}
        qset = GenericUserGroupRelation.objects.filter(**kwargs_prof)
        profile_ids = qset.values_list('profile__pk', flat=True)
        log_args = ['affected_groups', affected_groups, 'profile_ids', profile_ids,
                'affected_groups_origin', affected_groups_origin]
        _logger.info(None, *log_args)
        # load user profiles that already activated their accounts
        profiles = GenericUserProfile.objects.filter(pk__in=profile_ids, account__isnull=False)
        return profiles
## end of class GenericUserGroup


class GenericUserGroupClosure(ClosureTableModelMixin, SoftDeleteObjectMixin):
    """ closure table to describe tree structure of user group hierarchies """
    SOFTDELETE_CHANGESET_MODEL = UsermgtChangeSet
    SOFTDELETE_RECORD_MODEL = UsermgtSoftDeleteRecord
    class Meta(ClosureTableModelMixin.Meta):
        db_table = 'generic_user_group_closure'

    ancestor   = ClosureTableModelMixin.asc_field(ref_cls=GenericUserGroup)
    descendant = ClosureTableModelMixin.desc_field(ref_cls=GenericUserGroup)


class GenericUserProfile(GenericUserCommonFieldsMixin, MinimumInfoMixin):
    NONE = 0
    SUPERUSER = ROLE_ID_SUPERUSER
    STAFF = ROLE_ID_STAFF
    min_info_field_names = ['id','first_name','last_name']

    class Meta(SoftDeleteObjectMixin.Meta):
        db_table = 'generic_user_profile'

    first_name = models.CharField(max_length=32, blank=False, unique=False)
    last_name  = models.CharField(max_length=32, blank=False, unique=False)
    time_created = models.DateTimeField(auto_now_add=True)
    # record last time this user used (or logined to) the system
    last_updated = models.DateTimeField(auto_now=True)
    # the group(s) the user belongs to
    ####groups = models.ManyToManyField(GenericUserGroup, blank=True, db_table='generic_user_group_relation', related_name='user_profiles')
    @classmethod
    def update_accounts_privilege(cls, profiles):
        with _atomicity_fn():
            for profile in profiles:
                profile.update_account_privilege()

    @_atomicity_fn()
    def delete(self, *args, **kwargs):
        del_prof_id = self.pk
        profile_id  = kwargs.get('profile_id', None)
        hard_delete = kwargs.get('hard', False)
        self.clean_reset_account_requests()
        changeset = super().delete(*args, **kwargs) # login account will be automatically deleted
        if not hard_delete:
            if 'profile_id' not in kwargs.keys():
                kwargs['profile_id'] = profile_id
            self.groups.all().delete(*args, changeset=changeset, **kwargs)
            if _logger.level <= logging.DEBUG:
                del_set_exist = type(self).objects.get_deleted_set().filter(pk=del_prof_id).exists()
                phones_exist = self.phones.all().exists()
                geoloc_exist = self.locations.all().exists()
                cset_records = changeset.soft_delete_records.all().values('pk', 'content_type__pk', 'object_id')
                log_args = ['del_prof_id', del_prof_id, 'del_set_exist', del_set_exist, 'cset_records', cset_records,
                        'changeset_id', changeset.pk, 'phones_exist', phones_exist, 'geoloc_exist', geoloc_exist ]
                _logger.debug(None, *log_args)

    @_atomicity_fn()
    def undelete(self, *args, **kwargs):
        hard_delete = kwargs.get('hard', False)
        ops_prof_id  = kwargs.get('profile_id', None)
        try:
            status = super().undelete(*args, **kwargs)
        except ObjectDoesNotExist as e:
            err_msg = e.args[0]
            if hard_delete or (not ops_prof_id) or err_msg != self._changeset_not_found_err_msg:
                raise
            prof_cls_ct = ContentType.objects.get_for_model(self)
            qset = self.SOFTDELETE_CHANGESET_MODEL.objects.filter(content_type=prof_cls_ct, object_id=self.pk)
            if not qset.exists():
                raise
            changeset = qset.first()
            if int(changeset.done_by) != self.pk:
                raise # TODO, logging
            ops_prof = type(self).objects.get(id=ops_prof_id)
            ops_prof_can_undelete = ops_prof.privilege_status == ops_prof.SUPERUSER
            if not ops_prof_can_undelete:
                related_field_name = LOOKUP_SEP.join(['group', 'descendants', 'descendant', 'id'])
                valid_grp_ids = ops_prof.groups.values_list(related_field_name, flat=True)
                related_field_name = LOOKUP_SEP.join(['group', 'id', 'in'])
                applied_grp_set = GenericUserGroupRelation.objects.all(with_deleted=True)
                qset = applied_grp_set.filter(**{related_field_name: valid_grp_ids, 'profile':self})
                ops_prof_can_undelete = qset.exists()
            if ops_prof_can_undelete:
                try:
                    kwargs['profile_id'] = self.pk
                    status = super().undelete(*args, **kwargs)
                finally:
                    kwargs['profile_id'] = ops_prof_id
            else:
                raise


    @_atomicity_fn()
    def activate(self, new_account_data):
        account = None
        try:
            account  = self.account
            account.is_active = True
            account.save(update_fields=['is_active'])
        except ObjectDoesNotExist: # for first time to activate the account
            new_account_data.update({'profile': self, 'is_active': True})
            self.account = auth.get_user_model().objects.create_user( **new_account_data )
            self.update_account_privilege()
        return self.account


    @_atomicity_fn()
    def deactivate(self, remove_account=False):
        log_args = []
        if not self.pk:
            err_msg = "cannot deactivate user instance that hasn't been created yet"
            log_args.extend(['msg', err_msg])
            _logger.error(None, *log_args)
            raise ValueError(err_msg)
        self.clean_reset_account_requests()
        try:
            account  = self.account
            if remove_account:
                account.delete()
            else:
                account.check_admin_exist()
                account.is_active = False
                account.save(update_fields=['is_active'])
        except ObjectDoesNotExist:
            log_args.extend(['msg', 'no login account for the user'])
        if any(log_args):
            log_args.extend(['profile_id', self.pk])
            _logger.info(None, *log_args)

    def clean_reset_account_requests(self):
        for email in self.emails.all():
            email.rst_account_reqs.all().delete()

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
        quota_exp_field = ['ancestors', 'ancestor', 'quota', 'expiry']
        quota_mat_field = LOOKUP_SEP.join(quota_mat_field)
        quota_val_field = LOOKUP_SEP.join(quota_val_field)
        cond_before_groupby = UserQuotaRelation.expiry_condition(field_name=quota_exp_field)
        cond_after_groupby  = models.Q(max_num_chosen__gt=0)
        qset = groups.filter( cond_before_groupby ).values(quota_mat_field).annotate(
                max_num_chosen=models.Max(quota_val_field)
            ).order_by(quota_mat_field).distinct().filter( cond_after_groupby )
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
        grp_ct = ContentType.objects.get_for_model(GenericUserGroup)
        grp_ids = self.groups.values_list('group__pk', flat=True)
        asc_ids = GenericUserGroupClosure.objects.filter(descendant__pk__in=grp_ids).values_list('ancestor__id', flat=True)
        role_rel_cls = self.roles.model
        valid_role_rel_cond = models.Q(user_type=grp_ct) & models.Q(user_id__in=asc_ids) & role_rel_cls.expiry_condition()
        role_ids = role_rel_cls.objects.filter(valid_role_rel_cond).values_list('role__id', flat=True)
        role_cls = role_rel_cls.role.field.related_model
        roles = role_cls.objects.filter(id__in=role_ids)
        return roles

    @property
    def direct_roles(self):
        role_rel_cls = self.roles.model
        valid_role_rel_cond = role_rel_cls.expiry_condition()
        role_ids = self.roles.filter(valid_role_rel_cond).values_list('role__id', flat=True)
        role_cls = role_rel_cls.role.field.related_model
        roles = role_cls.objects.filter(id__in=role_ids)
        return roles

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

    def update_account_privilege(self, auto_save=True):
        """
        update privilege (is_superuser, is_staff flags)  in LoginAccount,
        the method should run after this instance of the class is already saved
        """
        log_args = ['auto_save', auto_save, 'profile_id', self.pk,]
        try:
            self.account # test whether the account exists
            privilege = self.privilege_status
            if privilege == self.SUPERUSER:
                self.account.is_superuser = True
                self.account.is_staff = True
            elif privilege == self.STAFF:
                self.account.is_superuser = False
                self.account.is_staff = True
            else: # TODO, force such users logout if they were staff before the change
                self.account.is_superuser = False
                self.account.is_staff = False
            if auto_save:
                self.account.save(update_fields=['is_superuser','is_staff'])
            log_args.extend(['privilege', privilege])
        except ObjectDoesNotExist as e: # for first time to activate the account
            log_args.extend(['errmsg', ' '.join(e.args)])
        _logger.debug(None, *log_args)
#### end of GenericUserProfile




class GenericUserAppliedRole(AbstractUserRelation, SoftDeleteObjectMixin, _ExpiryFieldMixin):
    SOFTDELETE_CHANGESET_MODEL = UsermgtChangeSet
    SOFTDELETE_RECORD_MODEL = UsermgtSoftDeleteRecord
    class Meta:
        db_table = 'generic_user_applied_role'
    # * in case the staff who approved these role requests are deleted, the approved_by field should be
    #   modified to default superuser. So  a profile for default superuser will be necessary
    role = models.ForeignKey('Role', blank=False, db_column='role', related_name='users_applied',
                on_delete=models.CASCADE,)
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


