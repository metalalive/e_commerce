import logging

from django.core.validators   import EMPTY_VALUES
from django.db.models         import Q
from django.db.models.constants import LOOKUP_SEP
from django.contrib.contenttypes.models  import ContentType

from rest_framework.settings   import api_settings
from rest_framework.exceptions import PermissionDenied
from rest_framework.permissions import BasePermission as DRFBasePermission, DjangoModelPermissions, DjangoObjectPermissions
from rest_framework.filters import BaseFilterBackend

from .models.base import GenericUserGroup,  GenericUserGroupClosure, GenericUserProfile, GenericUserGroupRelation

_logger = logging.getLogger(__name__)
"""
permissions for views in staff-only backend site
"""

class JWTclaimPermissionMixin:
    def _has_permission(self, tok_payld, method):
        from common.models.constants  import ROLE_ID_SUPERUSER, ROLE_ID_STAFF
        priv_status = tok_payld['priv_status']
        if priv_status == ROLE_ID_SUPERUSER:
            result = True
        elif priv_status == ROLE_ID_STAFF:
            perms_from_usr = list(map(lambda d:d['codename'] , tok_payld['perms']))
            perms_required = self.perms_map.get(method, [])
            covered = set(perms_required) - set(perms_from_usr)
            result = not any(covered)
        else:
            result = False
        return result


class ModelLvlPermsPermissions(DRFBasePermission, JWTclaimPermissionMixin):
    perms_map = {
        'GET': ['view_role'],
        'OPTIONS': [],
    }

    def has_permission(self, request, view):
        return self._has_permission(tok_payld=request.auth, method=request.method)


class RolePermissions(DjangoModelPermissions, BaseFilterBackend, JWTclaimPermissionMixin):
    message = {api_settings.NON_FIELD_ERRORS_KEY: ['you do not have permission to perform the operation']}
    perms_map = {
        'GET': ['view_role',],
        'OPTIONS': [],
        'HEAD': [],
        'POST':   ['view_role', 'add_role',   ],
        'PUT':    ['view_role', 'change_role',],
        'PATCH':  ['view_role', 'change_role',],
        'DELETE': ['view_role', 'delete_role',],
    }

    def filter_queryset(self, request, queryset, view):
        """
        only retrieve all roles granted to the current authenticated user,
        except it's superuser
        """
        account = request.user
        read_all = getattr(view, '_can_read_all_roles', False)
        if not account.is_superuser and not read_all:
            valid_roles = account.profile.all_roles
            direct_role_ids  = valid_roles['direct'].values_list('id', flat=True)
            inherit_role_ids = valid_roles['inherit'].values_list('id', flat=True)
            condition = Q(id__in=direct_role_ids) | Q(id__in=inherit_role_ids)
            queryset = queryset.filter(condition)
        return queryset

    def has_permission(self, request, view):
        result = self._has_permission(tok_payld=request.auth, method=request.method)
        # still return true for safe method like GET, because unauthorized users still
        # can only view the roles granted to themselves, But NOT allowed to modify
        view._can_read_all_roles = result
        if result is False and request.method == 'GET':
            result = True
        return result

    def has_object_permission(self, request, view, obj):
        account = request.user
        read_all = getattr(view, '_can_read_all_roles', False)
        if account.is_superuser or read_all:
            result = True
        else:
            valid_roles = account.profile.all_roles
            result_d = valid_roles['direct'].filter(id=obj.pk).exists()
            result_i = valid_roles['inherit'].filter(id=obj.pk).exists()
            result = result_d or result_i
        return result
## end of class RolePermissions


def _get_valid_groups(account):
    field_name = LOOKUP_SEP.join(['group', 'descendants', 'descendant', 'id'])
    return  account.profile.groups.values_list(field_name, flat=True)

def _get_valid_profs(self, account, view):
    if not hasattr(view, '_valid_profs_pk'):
        valid_grps = _get_valid_groups(account=account)
        applied_grp_set = GenericUserGroupRelation.objects.filter(group__pk__in=valid_grps['all'])
        valid_prof_set = GenericUserProfile.objects.filter(pk__in=applied_grp_set.values_list('profile__pk', flat=True))
        view._valid_profs_pk = valid_prof_set.values_list('pk', flat=True)
    return view._valid_profs_pk




class CommonUserPermissions(DjangoObjectPermissions, BaseFilterBackend):
    message = {api_settings.NON_FIELD_ERRORS_KEY: ['not allowed to perform this action on the profile(s) or group(s)']}

    # In Django default implementation, APIView.check_permissions() is automatically called
    # prior to method handling function (e.g. GET, POST ... etc) ,
    # while APIView.check_object_permissions() is called only when invoking View.get_object()
    # , for performance reason, generic view will NOT automatically call check_object_permissions()
    # to check permission on each object in a queryset, instead one could filter the queryset
    # appropriately before checking permission

    def has_object_permission(self, request, view, obj):
        raise NotImplementedError



class UserGroupsPermissions(DRFBasePermission, BaseFilterBackend, JWTclaimPermissionMixin):
    message = {api_settings.NON_FIELD_ERRORS_KEY: ['not allowed to perform this action on the profile(s) or group(s)']}
    perms_map = {
        'GET': ['view_genericusergroup'],
        'OPTIONS': [],
        'HEAD': [],
        'POST':   ['view_genericusergroup', 'add_genericusergroup',   ],
        'PUT':    ['view_genericusergroup', 'change_genericusergroup',],
        'PATCH':  ['view_genericusergroup', 'change_genericusergroup',],
        'DELETE': ['view_genericusergroup', 'delete_genericusergroup',],
    }

    def has_edit_permission(self, request, view):
        account = request.user
        valid_grp_ids = _get_valid_groups(account=account)
        valid_grp_ids = set(valid_grp_ids)
        req_grp_ids = filter(lambda d:d.get('id'), request.data)
        req_grp_ids = set(map(lambda d:d['id'], req_grp_ids))
        uncovered_grp_ids = req_grp_ids - valid_grp_ids
        result = not any(uncovered_grp_ids)
        log_args = ['valid_grp_ids', valid_grp_ids, 'req_grp_ids', req_grp_ids]
        log_args.extend(['result', result])
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
            if not account.is_superuser and request.method.upper() in ('PUT', 'DELETE'):
                result = self.has_edit_permission(request=request, view=view,)
        else:
            if request.method.upper() == 'GET':
                result = True
        return result

    def has_object_permission(self, request, view, obj):
        result = False
        can_view_all_groups = getattr(request, '_can_view_all_groups', False)
        if can_view_all_groups:
            result = True
        else:
            account = request.user
            valid_grps = _get_valid_groups(account=account)
            result = obj.id in valid_grps
        return result


    # only for handling queryset permissions
    def filter_queryset(self, request, queryset, view):
        can_view_all_groups = getattr(request, '_can_view_all_groups', False)
        if not can_view_all_groups:
            account = request.user
            valid_grps = _get_valid_groups(account=account)
            queryset = queryset.filter(pk__in=valid_grps)
        return queryset
#### end of UserGroupsPermissions



class UserProfilesPermissions(CommonUserPermissions):
    perms_map = {
        'GET': [
            '%(app_label)s.view_%(model_name)s',
            '%(app_label)s.view_genericusergroup',
            'auth.view_group',
            '%(app_label)s.view_userquotarelation',
            '%(app_label)s.view_useremailaddress',
            '%(app_label)s.view_userphonenumber',
            '%(app_label)s.view_userlocation',
            '%(app_label)s.view_quotausagetype',
            '%(app_label)s.view_emailaddress',
            '%(app_label)s.view_phonenumber',
            'location.view_location',
            ],
        'OPTIONS': [],
        'HEAD': [],
        'POST':   [
            '%(app_label)s.add_%(model_name)s',
            '%(app_label)s.view_genericusergroup',
            'auth.view_group',
            '%(app_label)s.add_genericuserappliedrole',
            '%(app_label)s.add_genericusergrouprelation',
            '%(app_label)s.add_userquotarelation',
            '%(app_label)s.add_useremailaddress',
            '%(app_label)s.add_userphonenumber',
            '%(app_label)s.add_userlocation',
            '%(app_label)s.view_quotausagetype',
            '%(app_label)s.add_emailaddress',
            '%(app_label)s.add_phonenumber',
            'location.add_location',
            ],
        'PUT': [
            '%(app_label)s.change_%(model_name)s',
            '%(app_label)s.view_genericusergroup',

            'auth.view_group',
            '%(app_label)s.add_genericuserappliedrole',
            '%(app_label)s.add_genericusergrouprelation',
            '%(app_label)s.change_genericuserappliedrole',
            '%(app_label)s.change_genericusergrouprelation',
            '%(app_label)s.delete_genericuserappliedrole',
            '%(app_label)s.delete_genericusergrouprelation',

            '%(app_label)s.add_userquotarelation',
            '%(app_label)s.add_useremailaddress',
            '%(app_label)s.add_userphonenumber',
            '%(app_label)s.add_userlocation',
            '%(app_label)s.change_userquotarelation',
            '%(app_label)s.change_useremailaddress',
            '%(app_label)s.change_userphonenumber',
            '%(app_label)s.change_userlocation',
            '%(app_label)s.delete_userquotarelation',
            '%(app_label)s.delete_useremailaddress',
            '%(app_label)s.delete_userphonenumber',
            '%(app_label)s.delete_userlocation',

            '%(app_label)s.view_quotausagetype',
            '%(app_label)s.add_emailaddress',
            '%(app_label)s.add_phonenumber',
            'location.add_location',
            '%(app_label)s.change_emailaddress',
            '%(app_label)s.change_phonenumber',
            'location.change_location',
            '%(app_label)s.delete_emailaddress',
            '%(app_label)s.delete_phonenumber',
            'location.delete_location',
            ],
        'PATCH':  [ # used as undelete / recovery API
            '%(app_label)s.change_%(model_name)s',
            '%(app_label)s.view_genericusergroup',
            'auth.view_group',
            '%(app_label)s.change_useremailaddress',
            '%(app_label)s.change_emailaddress',
            ],
        'DELETE': [
            '%(app_label)s.change_%(model_name)s', # consider soft-delete cases
            '%(app_label)s.delete_%(model_name)s',

            '%(app_label)s.change_useremailaddress',
            '%(app_label)s.delete_userquotarelation',
            '%(app_label)s.delete_useremailaddress',
            '%(app_label)s.delete_userphonenumber',
            '%(app_label)s.delete_userlocation',

            '%(app_label)s.change_emailaddress',
            '%(app_label)s.delete_emailaddress',
            '%(app_label)s.delete_phonenumber',
            'location.delete_location',
            ],
    }

    def has_edit_permission(self, request, view):
        result = True
        account = request.user
        req_payld = request.data
        valid_roles = account.profile.all_roles
        valid_grps  = _get_valid_groups(account=account, view=view)
        valid_profs = _get_valid_profs(account=account, view=view)
        err_msgs = []
        try:
            for data in req_payld:
                err_msg = {}
                pid   = data.get('id')
                grps  = data.get('groups', [])
                roles = data.get('roles', [])
                num_valid_grps  = valid_grps['all'].filter(descendant__pk__in=grps).count()
                # TODO be aware of overlapping roles (the same role in both of user and group)
                num_valid_roles = valid_roles['direct'].filter(id__in=roles).count() + \
                            valid_roles['inherit'].filter(id__in=roles).count()
                if pid and not valid_profs.filter(pk=int(pid)).exists():
                    err_msg[api_settings.NON_FIELD_ERRORS_KEY] = ["not allowed to edit the user profile (ID = {})".format(pid),]
                if len(grps) != num_valid_grps:
                    err_msg['groups'] = ["list of groups contains invalid ID {}".format(str(grps)) ]
                if len(roles) != num_valid_roles:
                    err_msg['roles'] = ["list of roles contains invalid ID {}".format(str(roles)) ]
                if any(err_msg):
                    result = False
                err_msgs.append(err_msg)
        except (ValueError, TypeError) as e:
            err_msgs = {api_settings.NON_FIELD_ERRORS_KEY: "caused by frontend input error"}
            result = False
        if not result:
            self.message = err_msgs
        return result


    def _get_delete_ids(self, request):
        IDs = request.query_params.get('ids', '')
        IDs = IDs.split(',')
        IDs = [int(i) for i in IDs if i.isdigit()]
        return IDs

    def has_delete_permission(self, request, view):
        result = True
        account = request.user
        err_msgs = {}
        log_args = []
        try:  # supposed to get list of IDs from URL
            IDs = self._get_delete_ids(request=request)
            valid_profs = self._get_valid_profs(account=account, view=view)
            num_valid_IDs = valid_profs.filter(pk__in=IDs).count()
            log_args.extend(['frontend_IDs', IDs, 'num_valid_IDs', num_valid_IDs])
            if len(IDs) != num_valid_IDs:
                err_msgs = {api_settings.NON_FIELD_ERRORS_KEY: "The list %s contains invalid IDs" % IDs}
                result = False
        except (ValueError, TypeError) as e:
            err_msgs = {api_settings.NON_FIELD_ERRORS_KEY: "unknown error from frontend input"}
            result = False
            log_args.extend(['excpt_msg', e])
        if not result:
            self.message = err_msgs
            log_args.extend(['err_msgs', err_msgs])
        log_args.extend(['result', result])
        loglevel = logging.DEBUG if result else logging.WARNING
        _logger.log(loglevel, None, *log_args, request=request)
        return result


    _extra_check_func = {
        'POST': has_edit_permission,
        'PUT' : has_edit_permission,
        'DELETE': has_delete_permission,
    }

    def has_permission(self, request, view):
        result = super().has_permission(request=request, view=view)
        account = request.user
        # a user at any group can edit their own profile, except applied groups and roles
        # which can only be edited by anyone granted `profile manager role`
        # at ancestor group.
        if result:
            if not account.is_superuser:
                fn = self._extra_check_func.get(request.method)
                if fn:
                    result = fn(self=self, request=request, view=view,)
        else:
            # if a user is not assigned with `profile manager role`, then this
            # user is only allowed to view/edit/delete their own profile.
            account_prof_id = str(account.genericuserauthrelation.profile.pk)
            log_args = []
            if request.method == 'PUT' and len(request.data) == 1:
                data = request.data[0]
                req_prof_id = str(data.get('id', ''))
                if req_prof_id == account_prof_id:
                    result = True
                    view._edit_personal_profile = True
                log_args.extend(['req_prof_id', req_prof_id])
            elif request.method == 'GET':
                req_prof_id = view.kwargs.get('pk', None)
                if req_prof_id is None or str(req_prof_id) == account_prof_id:
                    result = True
                    view._edit_personal_profile = True
                log_args.extend(['req_prof_id', req_prof_id])
            elif request.method == 'DELETE':
                IDs = self._get_delete_ids(request=request)
                log_args.extend(['IDs', IDs])
                # TODO, how to recover if a logged-in user deleted its own account ? recovered by superuser ?
                if len(IDs) == 1 and str(IDs[0]) == account_prof_id:
                    result = True
                    view._edit_personal_profile = True
            log_args.extend(['result', result, 'account_prof_id', account_prof_id])
            if hasattr(view, '_edit_personal_profile'):
                log_args.extend(['_edit_personal_profile', view._edit_personal_profile])
            loglevel = logging.DEBUG if result else logging.WARNING
            _logger.log(loglevel, None, *log_args, request=request)
        return result


    def has_object_permission(self, request, view, obj):
        result = False
        account = request.user
        if account.is_superuser:
            result = True
        else:
            if getattr(view, '_edit_personal_profile', False):
                result = account.genericuserauthrelation.profile.pk == obj.pk
            else:
                all_valid_profs = self._get_valid_profs(account=account, view=view)
                result = all_valid_profs.filter(pk=obj.pk).exists()
        return result


    def filter_queryset(self, request, queryset, view):
        account = request.user
        if not account.is_superuser:
            if getattr(view, '_edit_personal_profile', False):
                all_valid_profs = [account.genericuserauthrelation.profile.pk]
            else:
                all_valid_profs = self._get_valid_profs(account=account, view=view)
            queryset = queryset.filter(pk__in=all_valid_profs)
        return queryset

#### end of UserProfilesPermissions


class UserDeactivationPermission(DjangoModelPermissions):
    perms_map = {
        'GET': ['ALWAYS_INVALID'],
        'OPTIONS': ['ALWAYS_INVALID'],
        'HEAD'   : ['ALWAYS_INVALID'],
        'POST'   : [],
        'PUT'    : ['ALWAYS_INVALID'],
        'PATCH'  : ['ALWAYS_INVALID'],
        'DELETE' : ['ALWAYS_INVALID'],
    }

    pk_field_name = 'id'

    def has_permission(self, request, view):
        result = super().has_permission(request=request, view=view)
        account = request.user
        if result:
            if not account.is_superuser:
                log_args = ['perm_cls', type(self).__name__]
                err_msgs = {}
                valid_profs = self._get_valid_profs(account=account, view=view)
                pids = list(map(lambda d: d.get(self.pk_field_name, None), request.data))
                log_args.extend(['pids', pids])
                try:
                    num_valid_IDs = valid_profs.filter(pk__in=pids).count()
                    log_args.extend(['num_valid_IDs', num_valid_IDs])
                    if num_valid_IDs != len(pids):
                        err_msgs = {api_settings.NON_FIELD_ERRORS_KEY: 'The list %s contains invalid ID' % pids}
                        result = False
                except (ValueError, TypeError) as e:
                    err_msgs = {api_settings.NON_FIELD_ERRORS_KEY: 'unknown error from frontend input'}
                    result = False
                    log_args.extend(['excpt_msg', e])
                if not result:
                    self.message = err_msgs
                    log_args.extend(['err_msgs', err_msgs[api_settings.NON_FIELD_ERRORS_KEY]])
                log_args.extend(['result', result])
                loglevel = logging.DEBUG if result else logging.WARNING
                _logger.log(loglevel, None, *log_args, request=request)
        return result


class UserActivationPermission(UserDeactivationPermission):
    pk_field_name = 'profile'


