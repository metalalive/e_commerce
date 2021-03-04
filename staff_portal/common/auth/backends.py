import logging

from django.contrib.auth     import authenticate
from django.contrib.auth.backends import ModelBackend, RemoteUserBackend
from django.core.exceptions import ImproperlyConfigured, PermissionDenied

from rest_framework.authentication import BaseAuthentication, SessionAuthentication
from rest_framework.permissions import BasePermission

from common.models.db import db_conn_retry_wrapper
from common.util.python import get_request_meta_key, serial_kvpairs_to_dict

FORWARDED_HEADER = 'forwarded'
_logger = logging.getLogger(__name__)

def _get_role_permissions(user_obj):
    from django.contrib.auth.models import Permission
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
        return user

    def _get_group_permissions(self, user_obj):
        return _get_role_permissions(user_obj)


class ForwardClientBackend(RemoteUserBackend):
    """
    this auth backend has to be used with `ForwardClientAuthentication`
    """
    create_unknown_user = False

    def _get_group_permissions(self, user_obj):
        return _get_role_permissions(user_obj)


class ForwardClientAuthentication(BaseAuthentication):
    """
    In this project, my Django API gateway acts as a reverse proxy and handles authentication
    when receiving client requests, then proxies the requests to destination application server
    which provides specific service. So the destination application server has to be limited
    to receive & process the reqeusts which are ONLY from the API gateway.

    In other words, in order to ensure each reqeust comes from the API gateway, this authentication
    class SHOULD be used in destination application servers, with IP/domain filter or whitelisting
    mechanism.

    Many of web servers (e.g. Apache or Nginx) or network software tools (e.g. Linux iptable, firewall)
    provide the functionality to restrict accesses by filtering IP address of each reqeust from
    untrusted network. if you are allowed to set up DMZ, NAT ... in the network infrastructure of
    your backend system, you can instead set up the network tools (as mentioned above) in front of
    the destination application server.
    """
    max_retry_db_conn = 2
    wait_intvl_sec = 0.05

    def authenticate(self, request):
        fwd_key = get_request_meta_key(FORWARDED_HEADER)
        fwd = request.META.get(fwd_key, '')
        fwd_dict = serial_kvpairs_to_dict(serial_kv=fwd, delimiter_pair=';', delimiter_kv='=')
        ##print('fwd_dict = %s' % fwd_dict)
        if fwd_dict:
            forward_usernames = fwd_dict.get('for', [])
            if any(forward_usernames):
                user = self._authenticate(remote_user=forward_usernames[0])
                if user and user.is_active:
                    return (user, None)

    @db_conn_retry_wrapper
    def _authenticate(self, **kwargs):
        user = authenticate(**kwargs)
        return user


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


