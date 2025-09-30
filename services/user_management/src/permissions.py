import logging

from django.db.models import Q
from django.db.models.constants import LOOKUP_SEP

from rest_framework.settings import api_settings
from rest_framework.permissions import (
    BasePermission as DRFBasePermission,
    DjangoModelPermissions,
)
from rest_framework.filters import BaseFilterBackend

from ecommerce_common.auth.jwt import JWTclaimPermissionMixin
from .models.base import GenericUserProfile, GenericUserGroupRelation

_logger = logging.getLogger(__name__)
"""
permissions for views in staff-only backend site
"""


class ModelLvlPermsPermissions(DRFBasePermission, JWTclaimPermissionMixin):
    perms_map = {
        "GET": ["view_role"],
        "OPTIONS": [],
    }

    def has_permission(self, request, view):
        return self._has_permission(tok_payld=request.auth, method=request.method)


class RolePermissions(DjangoModelPermissions, BaseFilterBackend, JWTclaimPermissionMixin):
    message = {
        api_settings.NON_FIELD_ERRORS_KEY: ["you do not have permission to perform the operation"]
    }
    perms_map = {
        "GET": [
            "view_role",
        ],
        "OPTIONS": [],
        "HEAD": [],
        "POST": [
            "view_role",
            "add_role",
        ],
        "PUT": [
            "view_role",
            "change_role",
        ],
        "PATCH": [
            "view_role",
            "change_role",
        ],
        "DELETE": [
            "view_role",
            "delete_role",
        ],
    }

    def filter_queryset(self, request, queryset, view):
        """
        only retrieve all roles granted to the current authenticated user,
        except it's superuser
        """
        account = request.user
        read_all = getattr(view, "_can_read_all_roles", False)
        if not account.is_superuser and not read_all:
            valid_roles = account.profile.all_roles
            direct_role_ids = valid_roles["direct"].values_list("id", flat=True)
            inherit_role_ids = valid_roles["inherit"].values_list("id", flat=True)
            condition = Q(id__in=direct_role_ids) | Q(id__in=inherit_role_ids)
            queryset = queryset.filter(condition)
        return queryset

    def has_permission(self, request, view):
        result = self._has_permission(tok_payld=request.auth, method=request.method)
        # still return true for safe method like GET, because unauthorized users still
        # can only view the roles granted to themselves, But NOT allowed to modify
        view._can_read_all_roles = result
        if result is False and request.method == "GET":
            result = True
        return result

    def has_object_permission(self, request, view, obj):
        account = request.user
        read_all = getattr(view, "_can_read_all_roles", False)
        if account.is_superuser or read_all:
            result = True
        else:
            valid_roles = account.profile.all_roles
            result_d = valid_roles["direct"].filter(id=obj.pk).exists()
            result_i = valid_roles["inherit"].filter(id=obj.pk).exists()
            result = result_d or result_i
        return result


## end of class RolePermissions


def _get_valid_groups(account):
    field_name = LOOKUP_SEP.join(["group", "descendants", "descendant", "id"])
    return account.profile.groups.values_list(field_name, flat=True).distinct()


def _get_valid_profs(account):
    valid_grp_ids = _get_valid_groups(account=account)
    field_name = LOOKUP_SEP.join(["group", "id", "in"])
    applied_grp_set = GenericUserGroupRelation.objects.filter(**{field_name: valid_grp_ids})
    valid_prof_ids = applied_grp_set.values_list("profile__id", flat=True).distinct()
    return valid_prof_ids


class UserGroupsPermissions(DRFBasePermission, BaseFilterBackend, JWTclaimPermissionMixin):
    message = {
        api_settings.NON_FIELD_ERRORS_KEY: ["not allowed to perform this action on the group(s)"]
    }
    perms_map = {
        "GET": ["view_genericusergroup"],
        "OPTIONS": [],
        "HEAD": [],
        "POST": [
            "view_genericusergroup",
            "add_genericusergroup",
        ],
        "PUT": [
            "view_genericusergroup",
            "change_genericusergroup",
        ],
        "PATCH": [
            "view_genericusergroup",
            "change_genericusergroup",
        ],
        "DELETE": [
            "view_genericusergroup",
            "delete_genericusergroup",
        ],
    }

    def has_hierarchy_permission(self, request):
        account = request.user
        valid_grp_ids = _get_valid_groups(account=account)
        valid_grp_ids = set(valid_grp_ids)
        req_grp_ids = filter(lambda d: d.get("id"), request.data)
        req_grp_ids = set(map(lambda d: d["id"], req_grp_ids))
        uncovered_grp_ids = req_grp_ids - valid_grp_ids
        result = not any(uncovered_grp_ids)
        log_args = ["valid_grp_ids", valid_grp_ids, "req_grp_ids", req_grp_ids]
        log_args.extend(["result", result])
        loglevel = logging.DEBUG if result else logging.WARNING
        _logger.log(loglevel, None, *log_args, request=request)
        return result

    def has_permission(self, request, view):
        result = self._has_permission(tok_payld=request.auth, method=request.method)
        # logged-in users that do not have read permission can only view the groups assigned
        # to themselves, then set result to True for GET request
        request._can_view_all_groups = result
        if result is True:
            account = request.user
            if not account.is_superuser and request.method.upper() in ("PUT", "DELETE"):
                result = self.has_hierarchy_permission(request=request)
        else:
            if request.method.upper() == "GET":
                result = True
        return result

    # In Django default implementation, APIView.check_permissions() is automatically called
    # prior to method handling function (e.g. GET, POST ... etc) ,
    # while APIView.check_object_permissions() is called only when invoking View.get_object()
    # , for performance reason, generic view will NOT automatically call check_object_permissions()
    # to check permission on each object in a queryset, instead one could filter the queryset
    # appropriately before checking permission
    def has_object_permission(self, request, view, obj):
        result = False
        can_view_all_groups = getattr(request, "_can_view_all_groups", False)
        if can_view_all_groups:
            result = True
        else:
            account = request.user
            valid_grps = _get_valid_groups(account=account)
            result = obj.id in valid_grps
        return result

    # only for handling queryset permissions
    def filter_queryset(self, request, queryset, view):
        can_view_all_groups = getattr(request, "_can_view_all_groups", False)
        if not can_view_all_groups:
            account = request.user
            valid_grps = _get_valid_groups(account=account)
            queryset = queryset.filter(pk__in=valid_grps)
        return queryset


#### end of UserGroupsPermissions


def _profile_has_hierarchy_permission(request, pk_field_name):
    account = request.user
    valid_prof_ids = _get_valid_profs(account=account)
    valid_prof_ids = set(valid_prof_ids)
    req_ids = filter(lambda d: d.get(pk_field_name), request.data)
    req_ids = set(map(lambda d: d[pk_field_name], req_ids))
    uncovered_ids = req_ids - valid_prof_ids
    result = not any(uncovered_ids)
    log_args = ["valid_prof_ids", valid_prof_ids, "req_ids", req_ids]
    log_args.extend(["result", result])
    loglevel = logging.DEBUG if result else logging.WARNING
    _logger.log(loglevel, None, *log_args, request=request)
    return result


class UserProfilesPermissions(DRFBasePermission, JWTclaimPermissionMixin):
    message = {
        api_settings.NON_FIELD_ERRORS_KEY: ["not allowed to perform this action on the profile(s)"]
    }
    perms_map = {
        "GET": [],  # TODO: implement field-level visibility
        "OPTIONS": [],
        "HEAD": [],
        "POST": [
            "view_genericuserprofile",
            "add_genericuserprofile",
        ],
        "PUT": [
            "view_genericuserprofile",
            "change_genericuserprofile",
        ],
        "PATCH": [
            "view_genericuserprofile",
            "change_genericuserprofile",
        ],
        "DELETE": [
            "view_genericuserprofile",
            "delete_genericuserprofile",
        ],
    }

    def has_permission(self, request, view):
        result = self._has_permission(tok_payld=request.auth, method=request.method)
        account = request.user
        # a user at any group can edit their own profile, except applied groups and roles
        # which can only be edited by anyone granted `profile manager role`
        # at ancestor group.
        if result:
            if not account.is_superuser and request.method.upper() in ("PUT", "DELETE"):
                result = _profile_has_hierarchy_permission(request=request, pk_field_name="id")
        else:
            # logged-in users that do not have access permission can only read/write his/her own profile
            account_prof_id = str(account.profile.id)
            log_args = []
            if request.method.upper() == "PUT" and len(request.data) == 1:
                data = request.data[0]
                req_prof_id = str(data.get("id", ""))
                if req_prof_id == account_prof_id:
                    result = True
                log_args.extend(["req_prof_id", req_prof_id])
            elif request.method.upper() == "DELETE":
                req_ids = filter(lambda d: d.get("id"), request.data)
                IDs = list(map(lambda d: d["id"], req_ids))
                log_args.extend(["IDs", IDs])
                # TODO, how to recover if a logged-in user deleted its own account ? recovered by superuser ?
                if len(IDs) == 1 and str(IDs[0]) == account_prof_id:
                    result = True
            log_args.extend(["result", result, "account_prof_id", account_prof_id])
            loglevel = logging.DEBUG if result else logging.WARNING
            _logger.log(loglevel, None, *log_args, request=request)
        return result

    def has_object_permission(self, request, view, obj):
        return request.method.upper() == "GET"


#### end of UserProfilesPermissions


class AccountDeactivationPermission(DRFBasePermission, JWTclaimPermissionMixin):
    perms_map = {
        "GET": ["ALWAYS_INVALID"],
        "OPTIONS": ["ALWAYS_INVALID"],
        "HEAD": ["ALWAYS_INVALID"],
        "POST": [
            "delete_unauthresetaccountrequest",
            "change_loginaccount",
            "delete_loginaccount",
        ],
        "PUT": ["ALWAYS_INVALID"],
        "PATCH": ["ALWAYS_INVALID"],
        "DELETE": ["ALWAYS_INVALID"],
    }

    def has_permission(self, request, view):
        result = self._has_permission(tok_payld=request.auth, method=request.method)
        account = request.user
        # each authorized user can only deactivate his/her own account,
        # while superuser can deactivate several accounts (of other users) in one API call.
        if result and not account.is_superuser:
            result = _profile_has_hierarchy_permission(request=request, pk_field_name="profile")
        return result


class AccountActivationPermission(AccountDeactivationPermission):
    perms_map = {
        "GET": ["ALWAYS_INVALID"],
        "OPTIONS": ["ALWAYS_INVALID"],
        "HEAD": ["ALWAYS_INVALID"],
        "POST": ["add_unauthresetaccountrequest", "change_loginaccount"],
        "PUT": ["ALWAYS_INVALID"],
        "PATCH": ["ALWAYS_INVALID"],
        "DELETE": ["ALWAYS_INVALID"],
    }

    def has_permission(self, request, view):
        result = super().has_permission(request=request, view=view)
        req_items = list(filter(lambda d: not d.get("profile"), request.data))
        if result and any(req_items):
            email_ids = set(map(lambda d: d.get("email"), req_items))
            valid_prof_ids = _get_valid_profs(account=request.user)
            filter_kwargs = {
                LOOKUP_SEP.join(["id", "in"]): valid_prof_ids,
                LOOKUP_SEP.join(["emails", "id", "in"]): email_ids,
            }
            try:
                qset = GenericUserProfile.objects.filter(**filter_kwargs).distinct()
                existing_email_ids = qset.values_list(LOOKUP_SEP.join(["emails", "id"]), flat=True)
                result = email_ids == set(existing_email_ids)
            except ValueError:
                result = False
        return result
