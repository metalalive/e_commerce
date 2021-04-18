import secrets
import hashlib
import logging

from datetime import datetime, timezone, timedelta

from django.db     import  models, IntegrityError, transaction
from django.db.models.manager   import Manager
from django.db.models.constants import LOOKUP_SEP
from django.core.validators import RegexValidator
from django.core.exceptions import EmptyResultSet, ObjectDoesNotExist, MultipleObjectsReturned
from django.contrib import auth
from django.contrib.contenttypes.models  import ContentType
from django.contrib.contenttypes.fields  import GenericForeignKey, GenericRelation
from django.contrib.auth.models import LoginAccountRoleRelation, Group as AuthRole

from rest_framework.settings    import api_settings
from rest_framework.exceptions  import PermissionDenied

from softdelete.models import SoftDeleteObjectMixin, ChangeSet, SoftDeleteRecord
from location.models   import Location

from common.util.python          import merge_partial_dup_listitem
from common.models.constants     import ROLE_ID_SUPERUSER, ROLE_ID_STAFF
from common.models.mixins        import MinimumInfoMixin, SerializableMixin
from common.models.closure_table import ClosureTableModelMixin, get_paths_through_processing_node, filter_closure_nodes_recovery

_logger = logging.getLogger(__name__)


class UsermgtChangeSet(ChangeSet):
    class Meta:
        db_table = 'usermgt_soft_delete_changeset'


class UsermgtSoftDeleteRecord(SoftDeleteRecord):
    class Meta:
        db_table = 'usermgt_soft_delete_record'
    changeset = UsermgtChangeSet.foreignkey_fieldtype()


class UserReferenceCheckMixin:
    def delete(self, *args, **kwargs):
        # TODO, will check whether there's any other user relation instance referenced to
        # of this deleting instance (e.g. email, phone, geo-location), if so, then the
        # instance will NOT be deleted.
        super().delete(*args, **kwargs)


# each user may have more than one email addresses or phone numbers (or none)
class EmailAddress(UserReferenceCheckMixin, SoftDeleteObjectMixin):
    SOFTDELETE_CHANGESET_MODEL = UsermgtChangeSet
    SOFTDELETE_RECORD_MODEL = UsermgtSoftDeleteRecord
    class Meta:
        db_table = 'email_address'
    addr = models.EmailField(max_length=160, default="notprovide@localhost", blank=False, null=False, unique=False)


class PhoneNumber(UserReferenceCheckMixin, models.Model):
    class Meta:
        db_table = 'phone_number'
    ccode_validator   = RegexValidator(regex=r"^\d{1,3}$", message="non-digit character detected, or length of digits doesn't meet requirement. It must contain only digit e.g. '91', '886' , from 1 digit up to 3 digits")
    linenum_validator = RegexValidator(regex=r"^\+?1?\d{7,15}$", message="non-digit character detected, or length of digits doesn't meet requirement. It must contain only digits e.g. '9990099', from 7 digits up to 15 digits")
    country_code = models.CharField(max_length=3,  validators=[ccode_validator],   unique=False)
    line_number  = models.CharField(max_length=15, validators=[linenum_validator], unique=False)


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
class UserEmailAddress(AbstractUserRelation, SoftDeleteObjectMixin):
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
        db_table = 'user_email_address'
    email = models.OneToOneField(EmailAddress, db_column='email', on_delete=models.CASCADE, related_name="useremail")

    def delete(self, *args, **kwargs):
        hard_delete = kwargs.get('hard', False)
        super().delete(*args, **kwargs)
        if hard_delete is False and kwargs.get('changeset', None) is None:
            # simply test whether changeset is given in soft-deleting case
            err_args = ['hard_delete', hard_delete, 'changeset', None]
            _logger.error(None, *err_args)
            raise KeyError
        self.email.delete(*args, **kwargs)


class UserPhoneNumber(AbstractUserRelation):
    class Meta:
        db_table = 'user_phone_number'
    phone = models.OneToOneField(PhoneNumber, db_column='phone', on_delete=models.CASCADE, related_name="userphone")

    def delete(self, *args, **kwargs):
        super().delete(*args, **kwargs)
        self.phone.delete(*args, **kwargs)


# it is possible that one user has multiple address locations to record
class UserLocation(AbstractUserRelation):
    class Meta:
        db_table = 'user_location'
    address  = models.OneToOneField(Location, db_column='address', on_delete=models.CASCADE, related_name="usergeoloc")

    def delete(self, *args, **kwargs):
        super().delete(*args, **kwargs)
        self.address.delete(*args, **kwargs)


class QuotaUsageType(models.Model, MinimumInfoMixin):
    min_info_field_names = ['id','label']
    class Meta:
        db_table = 'quota_usage_type'
    material = models.OneToOneField(to=ContentType, on_delete=models.CASCADE, db_column='material', )
    label    = models.CharField(max_length=50, unique=False)


class UserQuotaRelation(AbstractUserRelation):
    """ where the system stores quota arrangements for each user (or user group) """
    class Meta:
        db_table = 'user_quota_relation'
        constraints = [models.UniqueConstraint(fields=['user_id', 'user_type', 'usage_type'], name="unique_user_quota",)]
    usage_type = models.ForeignKey(to=QuotaUsageType, db_column='usage_type',  on_delete=models.CASCADE, related_name="user_quota_rules")
    maxnum = models.PositiveSmallIntegerField(default=1)


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
    quota = GenericRelation(UserQuotaRelation, object_id_field='user_id', content_type_field='user_type')
    #emails    = GenericRelation(UserEmailAddress, object_id_field='user_id', content_type_field='user_type')
    #phones    = GenericRelation(UserPhoneNumber,  object_id_field='user_id', content_type_field='user_type')
    #locations = GenericRelation(UserLocation,     object_id_field='user_id', content_type_field='user_type')

    @transaction.atomic
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
            if _logger.level <= logging.DEBUG:
                del_set_exist = type(self).objects.get_deleted_set().filter(pk=del_grp_id).exists()
                changeset = kwargs['changeset']
                cond = models.Q(ancestor=del_grp_id) | models.Q(descendant=del_grp_id)
                del_paths_qset = GenericUserGroupClosure.objects.get_deleted_set().filter(cond)
                del_paths_qset = del_paths_qset.values('pk', 'ancestor__pk', 'descendant__pk', 'depth')
                cset_records = changeset.soft_delete_records.all().values('pk', 'content_type__pk', 'object_id')
                log_args = ['changeset_id', changeset.pk, 'del_grp_id', del_grp_id, 'del_set_exist', del_set_exist,
                        'del_paths_qset', del_paths_qset, 'cset_records', cset_records]
                _logger.debug(None, *log_args)
            kwargs.pop('changeset', None)
            kwargs.pop('profile_id', None)
        #raise IntegrityError


    @transaction.atomic
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
        #### a.delete(*args, **kwargs)

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

    ##ancestor = models.ForeignKey(GenericUserGroup, db_column='ancestor', null=True,
    ##                on_delete=models.CASCADE, related_name='descendants')
    ##descendant = models.ForeignKey(GenericUserGroup, db_column='descendant', null=True,
    ##                on_delete=models.CASCADE, related_name='ancestors')
    ancestor   = ClosureTableModelMixin.asc_field(ref_cls=GenericUserGroup)
    descendant = ClosureTableModelMixin.desc_field(ref_cls=GenericUserGroup)



# TODO, should have default superuser account as fixture data
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
    # in case the user may have extra roles not specified by its groups
    ####roles = models.ManyToManyField(auth.models.Group, blank=True, db_table='generic_user_prof_auth_role', related_name='user_profiles')
    roles = GenericRelation('GenericUserAppliedRole', object_id_field='user_id', content_type_field='user_type')
    # reverse relations from related models e.g. emails / phone numbers / geographical locations
    quota     = GenericRelation(UserQuotaRelation, object_id_field='user_id', content_type_field='user_type')
    emails    = GenericRelation(UserEmailAddress, object_id_field='user_id', content_type_field='user_type')
    phones    = GenericRelation(UserPhoneNumber,  object_id_field='user_id', content_type_field='user_type')
    locations = GenericRelation(UserLocation,     object_id_field='user_id', content_type_field='user_type')

    @property
    def account(self):
        try:
            out = self.auth.login
        except ObjectDoesNotExist:
            out = None
        return out


    @transaction.atomic
    def delete(self, *args, **kwargs):
        del_prof_id = self.pk
        hard_delete = kwargs.get('hard', False)
        if not hard_delete:
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


    @transaction.atomic
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


    @transaction.atomic
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
            account = auth.models.User.objects.create_user( **new_account_data )
            GenericUserAuthRelation.objects.create(login=account, profile=self)
            GenericUserProfile.update_account_privilege(profile=self, account=account)
        return account


    @transaction.atomic
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


    @property
    def inherit_roles(self):
        if not hasattr(self, '_inherit_roles'):
            #self._inherit_roles = [ra.role for grpa in self.groups.all()  for asc in grpa.group.ancestors.all()
            #    for ra in asc.ancestor.roles.all() ]
            grp_ct = ContentType.objects.get_for_model(GenericUserGroup)
            grp_ids = self.groups.values_list('group__pk', flat=True)
            asc_ids = GenericUserGroupClosure.objects.filter(descendant__pk__in=grp_ids).values_list('ancestor__pk', flat=True)
            role_ids = GenericUserAppliedRole.objects.filter(user_type=grp_ct, user_id__in=asc_ids).values_list('role__pk', flat=True)
            self._inherit_roles = role_ids
        #### retrieve role object from GenericUserAppliedRole
        return self._inherit_roles

    @property
    def all_roles(self):
        if not hasattr(self, '_all_roles'):
            direct_roles  =  self.roles.values_list('role__pk', flat=True)
            #### print("direct_roles : "+ str(direct_roles))
            inherit_roles = self.inherit_roles
            self._all_roles = direct_roles | inherit_roles
        return self._all_roles


    @property
    def privilege_status(self):
        out = self.NONE
        all_roles = self.all_roles
        if ROLE_ID_SUPERUSER in all_roles:
            out = self.SUPERUSER
        elif ROLE_ID_STAFF  in all_roles:
            out = self.STAFF
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
            for role in _list_add:
                u = LoginAccountRoleRelation(role=role, account=account)
                u.save()
        if list_del:
            old_role_relations.filter(role__pk__in=list_del).delete()
        # account.roles_applied.set(update_list) # completely useless if not m2m field
        log_args = ['profile_id', profile.pk, 'privilege', privilege, 'new_roles', new_roles,
                'old_roles', old_roles, 'list_add', list_add, 'list_del', list_del,
                'list_unchanged', list_unchanged]
        _logger.debug(None, *log_args)


    def serializable(self, present):
        related_field_map = {
            'roles': (['id', 'name'], {
                'perm_code': models.F('permissions__codename'),
                'app_label': models.F('permissions__content_type__app_label'),
            }),
            'quota': (['maxnum'], { # TODO, fetch quota records from group hierarchy
                'material_id': models.F('usage_type__material'),
                'model_name' : models.F('usage_type__material__model'),
                'app_label'  : models.F('usage_type__material__app_label'),
            }),
            'emails': ([], {})
        }
        def query_fn(fd_value, field_name, out):
            qset = None
            sub_fd_names = related_field_map.get(field_name, None)
            # for roles, it is also necessary to fetch all extra roles
            # which are inherited indirectly from the user group hierarchy
            if field_name == 'roles':
                qset = AuthRole.objects.filter(pk__in=self.all_roles)
            elif isinstance(fd_value, Manager):
                qset = fd_value.all()
            if qset and sub_fd_names:
                qset = qset.values(*sub_fd_names[0] , **sub_fd_names[1] )
                out[field_name] =  list(qset)
        # end of query_fn
        out = super().serializable(present=present, query_fn=query_fn)
        if out.get('roles', None):
            for role in out['roles']:
                app_label = role.pop('app_label', None)
                perm_code = role.get('perm_code',None)
                if app_label and perm_code:
                    role['perm_code'] = '%s.%s' % (app_label, perm_code)
            # further reduce duplicate data
            merge_partial_dup_listitem( list_in=out['roles'], combo_key=('id', 'name',),\
                    merge_keys=('perm_code',))
        return out
#### end of GenericUserProfile



# not all user can login into system, e.g. ex-employees, customers who don't use computer,
class GenericUserAuthRelation(models.Model):
    class Meta:
        db_table = 'generic_user_auth_relation'
    profile = models.OneToOneField(GenericUserProfile, db_column='profile',  on_delete=models.CASCADE, related_name="auth")
    login = models.OneToOneField(auth.get_user_model(), db_column='login',  on_delete=models.CASCADE)

    def delete(self, *args, **kwargs):
        self.check_admin_exist()
        account = self.login
        super().delete(*args, **kwargs)
        account.delete()

    def check_admin_exist(self):
        account = self.login
        # report error if frontend attempts to delete the only admin user in the backend site,
        # if other words, there must be at least one admin user (superuser = True) ready for the backend site,
        # (this seems difficult to be achieved by CheckConstraint)
        if account.is_superuser:
            num_superusers = type(account).objects.filter(is_superuser=True, is_active=True).count()
            log_args = ['account_id', account.pk, 'profile_id', self.profile.pk, 'num_superusers', num_superusers]
            if num_superusers <= 1:
                errmsg = "Forbidden to delete/deactivate the account"
                log_args.extend(['errmsg', errmsg])
                _logger.warning(None, *log_args)
                detail = {api_settings.NON_FIELD_ERRORS_KEY: [errmsg],}
                raise PermissionDenied(detail=detail) ##  SuspiciousOperation
            else:
                _logger.info(None, *log_args)



class GenericUserAppliedRole(AbstractUserRelation, SoftDeleteObjectMixin):
    SOFTDELETE_CHANGESET_MODEL = UsermgtChangeSet
    SOFTDELETE_RECORD_MODEL = UsermgtSoftDeleteRecord
    class Meta:
        db_table = 'generic_user_applied_role'

    # TODO:
    # * set unique constraint on each pair of (role, user/group)
    # * in case the staff who approved these role requests are deleted, the approved_by field should be
    #   modified to default superuser. So  a profile for default superuser will be necessary
    role = models.ForeignKey(auth.models.Group, blank=False, db_column='role', related_name='users_applied',
                on_delete=models.CASCADE,)
    # the approvement should expire after amount of time passed
    last_updated = models.DateTimeField(auto_now=True)
    # record the user who approved the reqeust (that a role can be granted to the group or individual user
    approved_by  = models.ForeignKey(GenericUserProfile, blank=True, null=True, db_column='approved_by',
        related_name="approval_role",  on_delete=models.SET_NULL,)


class GenericUserGroupRelation(SoftDeleteObjectMixin):
    SOFTDELETE_CHANGESET_MODEL = UsermgtChangeSet
    SOFTDELETE_RECORD_MODEL = UsermgtSoftDeleteRecord
    class Meta:
        db_table = 'generic_user_group_relation'
    group   = models.ForeignKey(GenericUserGroup, blank=False, on_delete=models.CASCADE, db_column='group', related_name='profiles')
    profile = models.ForeignKey(GenericUserProfile, blank=False, on_delete=models.CASCADE, db_column='profile', related_name='groups')
    approved_by  = models.ForeignKey(GenericUserProfile, blank=True, null=True, db_column='approved_by',
        related_name="approval_group",  on_delete=models.SET_NULL,)











class AuthUserResetRequest(models.Model, MinimumInfoMixin):
    """
    store token request for account reset operation, auto-incremented primary key is still required.
    entire token string will NOT stored in database table, instead it stores hashed token for more
    secure approach
    """
    class Meta:
        db_table = 'auth_user_reset_request'

    TOKEN_DELIMITER = '-'
    MAX_TOKEN_VALID_TIME = 600
    min_info_field_names = ['id']

    profile  = models.OneToOneField(GenericUserProfile, blank=True, db_column='profile',
                on_delete=models.CASCADE, related_name="auth_rst_req")
    # TODO, build validator to check if the chosen email address is ONLY for one user,
    # not shared by several people (would that happen in real cases ?)
    email    = models.ForeignKey(UserEmailAddress, db_column='email', on_delete=models.SET_NULL, null=True, blank=True)
    hashed_token = models.BinaryField(max_length=32, blank=True) # note: MySQL will refuse to set unique = True
    time_created = models.DateTimeField(auto_now=True)

    def is_token_expired(self):
        result = True
        t0 = self.time_created
        if t0:
            t0 += timedelta(seconds=self.MAX_TOKEN_VALID_TIME)
            t1  = datetime.now(timezone.utc)
            if t0 > t1:
                result = False
        return result

    @classmethod
    def is_token_valid(cls, token_urlencoded):
        result = None
        parts = token_urlencoded.split(cls.TOKEN_DELIMITER)
        if len(parts) > 1:
            try:
                req_id = int(parts[0])
            except ValueError:
                req_id = -1
            if req_id > 0:
                hashed_token = cls._hash_token(token_urlencoded)
                try:
                    instance = cls.objects.get(pk=req_id , hashed_token=hashed_token)
                except cls.DoesNotExist as e:
                    instance = None
                if instance:
                    if instance.is_token_expired():
                        pass #### instance.delete()
                    else:
                        result = instance
                log_args = ['req_id', req_id, 'hashed_token', hashed_token, 'result', result]
                _logger.info(None, *log_args)
        return result

    @classmethod
    def _hash_token(cls, token):
        hashobj = hashlib.sha256()
        hashobj.update(token.encode('utf-8'))
        return hashobj.digest()


    def _new_token(self):
        # generate random number + request id as token that will be sent within email
        token = self.TOKEN_DELIMITER.join( [str(self.pk), secrets.token_urlsafe(32)] )
        # hash the token (using SHA256, as minimum security requirement),
        # then save the hash token to model instance.
        hashed_token = self._hash_token(token)
        log_args = ['new_token', token, 'new_hashed_token', hashed_token]
        _logger.debug(None, *log_args)
        return token, hashed_token

    @transaction.atomic
    def save(self, force_insert=False, force_update=False, using=None, update_fields=None, **kwargs):
        if not self.profile.active:
            self.profile.active = True
            self.profile.save(update_fields=['active'])
        # check if the user sent reset request before and the token is still valid,
        # request from the same user cannot be duplicate in the model.
        if self.pk is None:
            self.hashed_token = b''
            super().save(force_insert=force_insert, force_update=force_update, using=using, update_fields=update_fields, **kwargs)
        self._token, hashed_token  = self._new_token()
        self.hashed_token = hashed_token
        super().save(force_insert=False, force_update=force_update, using=using, update_fields=['hashed_token', 'time_created'], **kwargs)

    @property
    def token(self):
        if hasattr(self, '_token'):
            return self._token
        else:
            return None

    @property
    def minimum_info(self):
        out = super().minimum_info
        extra = {'profile': self.profile.minimum_info, 'email': self.email.email.addr}
        out.update(extra)
        return out

#### end of  AuthUserResetRequest


