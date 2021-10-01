
from django.contrib import auth
from django.contrib.auth.models import GroupManager, _user_get_permissions, _user_has_perm, _user_has_module_perms
from django.contrib.auth.base_user import AbstractBaseUser, BaseUserManager
from django.contrib.auth.validators import UnicodeUsernameValidator
from django.db import models
from django.db.models.constants import LOOKUP_SEP
from django.utils import timezone
from django.utils.translation import gettext_lazy as _

from rest_framework.settings    import api_settings
from rest_framework.exceptions  import PermissionDenied

# from project codebase
from common.models.mixins import MinimumInfoMixin
from common.models.constants  import  ROLE_ID_SUPERUSER, ROLE_ID_STAFF
from .common import _atomicity_fn

# note: many of models here are copied from django.contribs.auth , but I remove some fields which are no longer used

class RoleQuerySet(models.QuerySet):
    def get_permissions(self, app_labels):
        ''' retrieve low-level permission instances from role queryset '''
        always_fetch_roles = (ROLE_ID_SUPERUSER, ROLE_ID_STAFF,)
        always_have_role_ids = models.Q(id__in=always_fetch_roles)
        rel_field = ['permissions', 'content_type', 'app_label', 'in']
        optional_role_ids = models.Q(**{LOOKUP_SEP.join(rel_field):app_labels})
        final_condition = always_have_role_ids | optional_role_ids
        role_qset = self.filter(final_condition).distinct()
        perm_cls = self.model.permissions.field.related_model
        perm_ids = role_qset.annotate(num_perms=models.Count('permissions')).filter(
                num_perms__gt=0).values_list('permissions', flat=True)
        perm_qset = perm_cls.objects.filter(id__in=perm_ids)
        return perm_qset

class RoleManager(GroupManager.from_queryset(RoleQuerySet)):
    pass

class Role(models.Model, MinimumInfoMixin):
    # Role-Based Access Control is applied to this project, the name field here are
    # referenced when the other application receives client request and check its
    # permission . There should be methods which maintain consistency on this name field
    # between authentication server (this user_management app) and all other resource servers
    # (all other apps implemented in different tech stack)
    name = models.CharField(_('name'), max_length=100, unique=True)
    # all other resource servers (might be implemented in different tech stack) can
    # add custom low-level permissions to Django's auth.Permission on schema migration,
    # so this authentication server has authorization information about all other apps
    # in this project.
    permissions = models.ManyToManyField('auth.Permission', verbose_name=_('permissions'), blank=False,)

    objects = RoleManager()
    min_info_field_names = ['id','name']

    class Meta:
        db_table = 'usermgt_role'
        verbose_name = _('role')
        verbose_name_plural = _('roles')

    def __str__(self):
        return self.name

    def natural_key(self):
        return (self.name,)
# end of class Role


class UserManager(BaseUserManager):
    use_in_migrations = True

    def _create_user(self, username, password, **extra_fields):
        """
        Create and save a user with the given username, and password.
        """
        if not username:
            raise ValueError('The given username must be set')
        username = self.model.normalize_username(username)
        user = self.model(username=username,  **extra_fields)
        user.set_password(password)
        user.save(using=self._db)
        return user

    def create_user(self, username, password=None, **extra_fields):
        extra_fields.setdefault('is_staff', False)
        extra_fields.setdefault('is_superuser', False)
        return self._create_user(username, password, **extra_fields)

    def create_superuser(self, username, password=None, **extra_fields):
        extra_fields.setdefault('is_staff', True)
        extra_fields.setdefault('is_superuser', True)

        if extra_fields.get('is_staff') is not True:
            raise ValueError('Superuser must have is_staff=True.')
        if extra_fields.get('is_superuser') is not True:
            raise ValueError('Superuser must have is_superuser=True.')

        return self._create_user(username, password, **extra_fields)

    def with_perm(self, perm, is_active=True, include_superusers=True, backend=None, obj=None):
        if backend is None:
            backends = auth._get_backends(return_tuples=True)
            if len(backends) == 1:
                backend, _ = backends[0]
            else:
                raise ValueError(
                    'You have multiple authentication backends configured and '
                    'therefore must provide the `backend` argument.'
                )
        elif not isinstance(backend, str):
            raise TypeError(
                'backend must be a dotted import path string (got %r).'
                % backend
            )
        else:
            backend = auth.load_backend(backend)
        if hasattr(backend, 'with_perm'):
            return backend.with_perm(
                perm,
                is_active=is_active,
                include_superusers=include_superusers,
                obj=obj,
            )
        return self.none()
## end of class UserManager


class PermissionsMixin(models.Model):
    """
    Add the fields and methods necessary to support the Role and low-level Permission
    models using the ModelBackend.
    """
    is_superuser = models.BooleanField(
        _('superuser status'),
        default=False,
        help_text=_(
            'Designates that this user has all permissions without '
            'explicitly assigning them.'
        ),
    )
    # in this mixin class I remove `groups` and `user_permissions` m2m fields because
    # the authentication service (user_management application) applies Role-based
    # access control (RBAC) so individual user shouldn't directly have low-level
    # permissions from `auth.Permission`

    class Meta:
        abstract = True

    def get_user_permissions(self, obj=None):
        """
        Return a list of permission strings that this user has directly.
        Query all available auth backends. If an object is passed in,
        return only permissions matching this object.
        """
        return _user_get_permissions(self, obj, 'user')

    def get_group_permissions(self, obj=None):
        """
        Return a list of permission strings that this user has through their
        groups. Query all available auth backends. If an object is passed in,
        return only permissions matching this object.
        """
        return _user_get_permissions(self, obj, 'group')

    def get_all_permissions(self, obj=None):
        return _user_get_permissions(self, obj, 'all')

    def has_perm(self, perm, obj=None):
        """
        Return True if the user has the specified permission. Query all
        available auth backends, but return immediately if any backend returns
        True. Thus, a user who has permission from a single auth backend is
        assumed to have permission in general. If an object is provided, check
        permissions for that object.
        """
        # Active superusers have all permissions.
        if self.is_active and self.is_superuser:
            return True
        # Otherwise we need to check the backends.
        return _user_has_perm(self, perm, obj)

    def has_perms(self, perm_list, obj=None):
        """
        Return True if the user has each of the specified permissions. If
        object is passed, check if the user has all required perms for it.
        """
        return all(self.has_perm(perm, obj) for perm in perm_list)

    def has_module_perms(self, app_label):
        """
        Return True if the user has any permissions in the given app label.
        Use similar logic as has_perm(), above.
        """
        # Active superusers have all permissions.
        if self.is_active and self.is_superuser:
            return True
        return _user_has_module_perms(self, app_label)
## end of class PermissionsMixin


class AbstractUser(AbstractBaseUser, PermissionsMixin, MinimumInfoMixin):
    """
    An abstract base class implementing a fully featured User model with
    admin-compliant permissions.

    Username and password are required. Other fields are optional.
    """
    username_validator = UnicodeUsernameValidator()

    username = models.CharField(
        _('username'),
        max_length=32,
        unique=True,
        help_text=_('Required. 64 characters or fewer. Letters, digits and @/./+/-/_ only.'),
        validators=[username_validator],
        error_messages={
            'unique': _("A user with that username already exists."),
        },
    )
    # email, first_name, and last_name fields at here are moved
    # to GenericUserProfile and UserEMailAddress model, since users could have
    # more than one emails
    is_staff = models.BooleanField(
        _('staff status'),
        default=False,
        help_text=_('Designates whether the user can log into this admin site.'),
    )
    is_active = models.BooleanField(
        _('active'),
        default=True,
        help_text=_(
            'Designates whether this user should be treated as active. '
            'Unselect this instead of deleting accounts.'
        ),
    )
    date_joined = models.DateTimeField(_('date joined'), default=timezone.now)
    password_last_updated = models.DateTimeField(_('password last updated'), blank=False, null=False)

    objects = UserManager()
    min_info_field_names = ['id','username']

    USERNAME_FIELD = 'username'

    class Meta:
        verbose_name = _('user')
        verbose_name_plural = _('users')
        abstract = True

    def clean(self):
        super().clean()

    def get_full_name(self):
        """
        Return the first_name plus the last_name, with a space in between.
        """
        raise NotImplementedError()

    def get_short_name(self):
        """Return the short name for the user."""
        raise NotImplementedError()

    def email_user(self, subject, message, from_email=None, **kwargs):
        """Send an email to this user."""
        raise NotImplementedError()


class LoginAccount(AbstractUser):
    class Meta(AbstractUser.Meta):
        db_table = 'login_account'
        swappable = 'AUTH_USER_MODEL'
    # not all registered users in GenericUserProfile can login in to system,
    # e.g. ex-employees, offline customers who never use computer,
    profile = models.OneToOneField('user_management.GenericUserProfile', db_column='profile',
                on_delete=models.CASCADE, primary_key=True, related_name='account')

    def delete(self, *args, **kwargs):
        self.check_admin_exist()
        super().delete(*args, **kwargs)

    def check_admin_exist(self):
        account = self
        # report error if frontend attempts to delete the only admin user in the backend site,
        # if other words, there must be at least one admin user (superuser = True) ready for the backend site,
        # (this seems difficult to be achieved by CheckConstraint)
        if account.is_superuser:
            num_superusers = type(account).objects.filter(is_superuser=True, is_active=True).count()
            log_args = ['account_id', account.pk, 'profile_id', self.profile.pk, 'num_superusers', num_superusers]
            if num_superusers <= 1:
                errmsg = "Forbidden to delete/deactivate this account"
                log_args.extend(['errmsg', errmsg])
                _logger.warning(None, *log_args)
                detail = {api_settings.NON_FIELD_ERRORS_KEY: [errmsg],}
                raise PermissionDenied(detail=detail) ##  SuspiciousOperation
            else:
                _logger.info(None, *log_args)
## end of class LoginAccount



class AccountResetRequest(models.Model, MinimumInfoMixin):
    """
    store token request for account reset operation, auto-incremented primary key is still required.
    entire token string will NOT stored in database table, instead it stores hashed token for more
    secure approach
    """
    class Meta:
        db_table = 'account_reset_request'

    TOKEN_DELIMITER = '-'
    MAX_TOKEN_VALID_TIME = 600
    min_info_field_names = ['id']

    profile  = models.OneToOneField('user_management.GenericUserProfile', blank=True, db_column='profile',
                on_delete=models.CASCADE, related_name="auth_rst_req")
    # TODO, build validator to check if the chosen email address is ONLY for one user,
    # not shared by several people (would that happen in real cases ?)
    email    = models.ForeignKey('user_management.EmailAddress', db_column='email', on_delete=models.SET_NULL, null=True, blank=True)
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

    @_atomicity_fn()
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

#### end of  AccountResetRequest




