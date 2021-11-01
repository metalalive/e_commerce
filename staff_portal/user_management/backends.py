import logging

from django.contrib.auth     import authenticate
from django.contrib.auth.models import Permission
from django.contrib.auth.backends import ModelBackend
from django.core.exceptions import ImproperlyConfigured, PermissionDenied

from common.models.db import db_conn_retry_wrapper

_logger = logging.getLogger(__name__)


def _get_role_permissions(user_obj):
    # django.contrib.auth.models.Group is used as roles in my application,
    # also user-to-group relation table (originally m2m field: groups) in  django.contrib.auth.models.User is
    # renamed to LoginAccountRoleRelation.
    related_names = ['group', 'accounts_applied', 'account']
    user_groups_query = '__'.join(related_names)
    # retrieve all low-level permissions granted to the given user
    # default query would be group__account_applied__account
    return Permission.objects.filter(**{user_groups_query: user_obj})


class ExtendedModelBackend(ModelBackend):
    """
    extend ModelBackend for recognizing whether the backend should restrict
    that only staff is allowed to login
    """
    def authenticate(self, request, username=None, password=None, is_staff_only=True, **kwargs):
        """ further check superuser or staff status of logged-in user, if specified """
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
        return user

    def _get_group_permissions(self, user_obj):
        return _get_role_permissions(user_obj)


