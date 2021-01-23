import logging

from django.contrib.auth.models import Permission
from django.contrib.auth.backends import ModelBackend
from django.core.exceptions import ImproperlyConfigured, PermissionDenied
from django.conf   import  settings as django_settings

from rest_framework.authentication import SessionAuthentication
from rest_framework.permissions import BasePermission

from common.models.db import db_conn_retry_wrapper

_logger = logging.getLogger(__name__)


class ExtendedModelBackend(ModelBackend):
    """
    authentication backend for staff users
    """
    def authenticate(self, request, username=None, password=None, **kwargs):
        """ further check superuser or staff status of logged-in user, if specified """
        is_staff_only = kwargs.pop('is_staff_only', True)
        user = super().authenticate(request=request, username=username, password=password, **kwargs)
        log_args = ['account', user, 'is_staff_only', is_staff_only, 'username', username]
        if user and not user.is_active:
            log_args.extend(['is_active', user.is_active])
            _logger.warning(None, *log_args)
            raise PermissionDenied("inactive users are not allowed to log in to this site")
        if is_staff_only and user:
            if not (user.is_superuser or user.is_staff):
                log_args.extend(['is_superuser', user.is_superuser, 'is_staff', user.is_staff])
                _logger.warning(None, *log_args)
                # abort authentication process, for non-staff users
                raise PermissionDenied("non-staff users are not allowed to log in to staff-only site")
        if user:
            init_session_expiry_secs = self._set_session_expiry(request=request, user=user)
            log_args.extend(['account_id', user.pk, 'init_session_expiry_secs', init_session_expiry_secs])
            _logger.debug(None, *log_args)
        return user

    def _set_session_expiry(self, request, user):
        # TODO: find better way to determine session expiry time for staff
        if user.is_superuser:
            expiry_secs = django_settings.SESSION_COOKIE_AGE
        elif  user.is_staff:
            expiry_secs = django_settings.SESSION_COOKIE_AGE << 1
        else:
            expiry_secs = django_settings.SESSION_COOKIE_AGE << 2
        request.session.set_expiry(value=expiry_secs)
        return expiry_secs

    def _get_group_permissions(self, user_obj):
        # django.contrib.auth.models.Group is used as roles in my application,
        # also user-to-group relation table (originally m2m field: groups) in  django.contrib.auth.models.User is
        # renamed to LoginAccountRoleRelation.
        related_names = ['group', 'accounts_applied', 'account']
        user_groups_query = '__'.join(related_names)
        # retrieve all low-level permissions granted to the given user
        # default query would be group__account_applied__account
        return Permission.objects.filter(**{user_groups_query: user_obj})



class ExtendedSessionAuthentication(SessionAuthentication):
    max_retry_db_conn = 5
    wait_intvl_sec = 0.03

    @db_conn_retry_wrapper
    def authenticate(self, request):
        return super().authenticate(request=request)


class IsStaffUser(BasePermission):
    """
    Allows access only to staff or superusers.
    """
    def has_permission(self, request, view):
        return bool(request.user and (request.user.is_staff or request.user.is_superuser))


class IsSuperUser(BasePermission):
    """
    Allows access only to superusers.
    """
    def has_permission(self, request, view):
        return bool(request.user and request.user.is_superuser)


