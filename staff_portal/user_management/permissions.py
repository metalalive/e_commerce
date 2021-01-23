import logging

from django.core.validators    import EMPTY_VALUES
from django.contrib.contenttypes.models  import ContentType

from rest_framework.settings   import api_settings
from rest_framework.exceptions import PermissionDenied
from rest_framework.permissions import DjangoModelPermissions, DjangoObjectPermissions
from rest_framework.filters import BaseFilterBackend

from .models import GenericUserGroup,  GenericUserGroupClosure, GenericUserProfile, GenericUserGroupRelation

_logger = logging.getLogger(__name__)
"""
permissions for views in staff-only backend site
"""

class BaseValidObjectsMixin:

    def _get_valid_roles(self, account, view):
        if not hasattr(view, '_valid_roles_pk'):
            view._valid_roles_pk = account.roles_applied.values_list('role__pk', flat=True)
        return view._valid_roles_pk

    def _get_valid_groups(self, account, view):
        if not hasattr(view, '_valid_groups_pk'):
            profile = account.genericuserauthrelation.profile
            view._valid_groups_pk = {}
            grp_set = profile.groups.values_list('group', flat=True)
            closure = GenericUserGroupClosure.objects.filter(ancestor__in=grp_set)
            view._valid_groups_pk['all'] = closure.values_list('descendant__pk', flat=True)
            closure = closure.filter(depth__gt=0)
            view._valid_groups_pk['descendant'] = closure.values_list('descendant__pk', flat=True)
        return view._valid_groups_pk

    def _get_valid_profs(self, account, view):
        if not hasattr(view, '_valid_profs_pk'):
            valid_grps = self._get_valid_groups(account=account, view=view)
            applied_grp_set = GenericUserGroupRelation.objects.filter(group__pk__in=valid_grps['all'])
            valid_prof_set = GenericUserProfile.objects.filter(pk__in=applied_grp_set.values_list('profile__pk', flat=True))
            view._valid_profs_pk = valid_prof_set.values_list('pk', flat=True)
        return view._valid_profs_pk


class BaseRolePermission(DjangoModelPermissions, BaseFilterBackend, BaseValidObjectsMixin):
    message = {api_settings.NON_FIELD_ERRORS_KEY: ['you do not have permission to perform the operation']}


class AuthRolePermissions(BaseRolePermission):
    perms_map = {
        'GET': ['auth.view_permission', '%(app_label)s.view_%(model_name)s'],
        'OPTIONS': [],
        'HEAD': [],
        'POST':   ['auth.view_permission', '%(app_label)s.view_%(model_name)s', '%(app_label)s.add_%(model_name)s'],
        'PUT':    ['auth.view_permission', '%(app_label)s.view_%(model_name)s', '%(app_label)s.change_%(model_name)s'],
        'PATCH':  ['auth.view_permission', '%(app_label)s.view_%(model_name)s', '%(app_label)s.change_%(model_name)s'],
        'DELETE': ['auth.view_permission', '%(app_label)s.view_%(model_name)s', '%(app_label)s.delete_%(model_name)s'],
    }

    def filter_queryset(self, request, queryset, view):
        """
        only retrieve all roles granted to the current authenticated user,
        except it's superuser
        """
        account = request.user
        full_access = getattr(view, '_perms_full_access', False)
        if not account.is_superuser and not full_access:
            valid_roles = self._get_valid_roles(account=account, view=view)
            queryset = queryset.filter(pk__in=valid_roles)
        return queryset

    def has_permission(self, request, view):
        result = super().has_permission(request=request, view=view)
        view._perms_full_access = result
        # still return true for safe method like GET, because unauthorized users still
        # can only view the roles granted to themselves, But NOT allowed to modify
        if result is False and request.method == 'GET':
            result = True
        return result

    def has_object_permission(self, request, view, obj):
        account = request.user
        full_access = getattr(view, '_perms_full_access', False)
        if account.is_superuser or full_access:
            result = True
        else:
            valid_roles = self._get_valid_roles(account=account, view=view)
            result = valid_roles.filter(role__pk=obj.pk).exists()
        return result


class AppliedRolePermissions(BaseRolePermission):
    def _get_all_valid_groups(self, account, view):
        out = self._get_valid_groups(account=account, view=view)
        return out['all']

    def filter_queryset(self, request, queryset, view):
        _valid_seek_types = {
            'groups':   {'model_cls':GenericUserGroup,   'fn':self._get_all_valid_groups },
            'profiles': {'model_cls':GenericUserProfile, 'fn':self._get_valid_profs },
        }
        seek_type = request.query_params.get('type', None)
        if seek_type in _valid_seek_types.keys() :
            account = request.user
            model_cls = _valid_seek_types[seek_type]['model_cls']
            ct_cls = ContentType.objects.get_for_model(model_cls)
            filter_kwargs = {'user_type':ct_cls,}
            log_args = ['account_id', account.pk, 'seek_type', seek_type]
            if not account.is_superuser:
                # TODO, what if the user does not have permissions to view other profiles ?
                fn = _valid_seek_types[seek_type]['fn']
                valid_ids = fn(account=account, view=view)
                filter_kwargs['user_id__in'] = valid_ids
            queryset = queryset.filter(**filter_kwargs)
            log_args.extend(['filter_kwargs', filter_kwargs])
            _logger.debug(None, *log_args, request=request)
        else:
            queryset = queryset.none()
        return queryset

    def has_permission(self, request, view):
        if request.method == 'GET':
            account = request.user
            if account.is_superuser:
                result = True
            else:
                role_pk = view.kwargs.get('pk', 0)
                log_args = ['role_pk', role_pk, 'account_id', account.pk]
                if role_pk.isdigit():
                    valid_roles = self._get_valid_roles(account=account, view=view)
                    result = valid_roles.filter(role__pk=role_pk).exists()
                    log_args.extend(['valid_roles', valid_roles])
                else:
                    result = False
                log_args.extend(['result', result])
                loglevel = logging.DEBUG if result else logging.WARNING
                _logger.log(loglevel, None, *log_args, request=request)
        else:
            result = False
        return result


class AppliedGroupPermissions(BaseRolePermission):
    def filter_queryset(self, request, queryset, view):
        account = request.user
        ct_cls = ContentType.objects.get_for_model(GenericUserProfile)
        if not account.is_superuser:
            valid_ids = self._get_valid_profs(account=account, view=view)
            filter_kwargs = {'profile__pk__in':valid_ids}
            queryset = queryset.filter(**filter_kwargs)
            log_args = ['filter_kwargs', filter_kwargs, 'account_id', account.pk]
            _logger.debug(None, *log_args, request=request)
        return queryset

    def has_permission(self, request, view):
        if request.method == 'GET':
            account = request.user
            if account.is_superuser:
                result = True
            else:
                grp_id = view.kwargs.get('pk', 0)
                log_args = ['grp_id', grp_id, 'account_id', account.pk]
                if grp_id.isdigit():
                    valid_grps = self._get_valid_groups(account=account, view=view)
                    result = valid_grps['all'].filter(descendant__pk=grp_id).exists()
                    log_args.extend(['valid_grps_all', valid_grps['all']])
                else:
                    result = False
                log_args.extend(['result', result])
                loglevel = logging.DEBUG if result else logging.WARNING
                _logger.log(loglevel, None, *log_args, request=request)
        else:
            result = False
        return result



class CommonUserPermissions(DjangoObjectPermissions, BaseValidObjectsMixin, BaseFilterBackend):

    message = {api_settings.NON_FIELD_ERRORS_KEY: ['not allowed to perform this action on the profile(s) or group(s)']}

    # In Django default implementation, APIView.check_permissions() is automatically called
    # prior to method handling function (e.g. GET, POST ... etc) ,
    # while APIView.check_object_permissions() is called only when invoking View.get_object()
    # , for performance reason, generic view will NOT automatically call check_object_permissions()
    # to check permission on each object in a queryset, instead one could filter the queryset
    # appropriately before checking permission

    def has_object_permission(self, request, view, obj):
        raise NotImplementedError



class UserGroupsPermissions(CommonUserPermissions):
    perms_map = {
        'GET': [
            #### '%(app_label)s.view_%(model_name)s',
            #### '%(app_label)s.view_genericusergroupclosure',
            #### '%(app_label)s.view_userquotarelation',
            #### '%(app_label)s.view_quotausagetype',
            #### 'auth.view_group',
            ],
        'OPTIONS': [],
        'HEAD': [],
        'POST':   [
            '%(app_label)s.add_%(model_name)s',
            '%(app_label)s.add_genericusergroupclosure',
            '%(app_label)s.add_userquotarelation',
            '%(app_label)s.view_quotausagetype',
            'auth.view_group',
            ],
        'PUT': [
            '%(app_label)s.change_%(model_name)s',
            '%(app_label)s.add_genericusergroupclosure',
            '%(app_label)s.change_genericusergroupclosure',
            '%(app_label)s.delete_genericusergroupclosure',
            '%(app_label)s.add_userquotarelation',
            '%(app_label)s.change_userquotarelation',
            '%(app_label)s.delete_userquotarelation',
            '%(app_label)s.view_quotausagetype',
            'auth.view_group',
            ],
        'PATCH':  [ # used as undelete / recovery API
            '%(app_label)s.change_%(model_name)s',
            '%(app_label)s.change_genericusergroupclosure',
            '%(app_label)s.delete_genericusergroupclosure',
            'auth.view_group',
            ],
        'DELETE': [
            '%(app_label)s.change_%(model_name)s', # consider soft-delete cases
            '%(app_label)s.delete_%(model_name)s',
            '%(app_label)s.change_genericusergroupclosure',
            '%(app_label)s.delete_genericusergroupclosure',
            '%(app_label)s.delete_userquotarelation',
            ],
    }

    def has_edit_permission(self, request, view):
        result = True
        account = request.user
        method = request.method
        req_payld = request.data
        valid_roles = self._get_valid_roles(account=account, view=view)
        valid_grps  = self._get_valid_groups(account=account, view=view)
        err_msgs = []
        log_args = ['request_method', method, 'valid_roles', valid_roles, 'valid_grps', valid_grps]

        try:
            for data in req_payld:
                err_msg = {}
                # Each applied group itself has to be read-only, while all its descendants
                # can be  modified , TODO, re-factor
                gid = data.get('id')
                exist_parent = data.get('exist_parent')
                new_parent = data.get('new_parent', None)
                roles = set(data.get('roles', []))
                log_args.extend(['gid', gid, 'exist_parent', exist_parent])
                if gid and not valid_grps['descendant'].filter(descendant__pk=int(gid)).exists():
                    err_msg[api_settings.NON_FIELD_ERRORS_KEY] = ["not allowed to edit group {}".format(gid)]
                elif not exist_parent:
                    parent_not_included = False
                    if method == 'POST' and not new_parent:
                        parent_not_included = True
                    elif method == 'PUT':
                        parent_not_included = True
                    if parent_not_included:
                        err_msg['exist_parent'] = ["you must select parent for the group you currently edit"]
                elif exist_parent and not valid_grps['all'].filter(descendant__pk=int(exist_parent)).exists():
                    err_msg['exist_parent'] = ["not allowed to select the parent group ID {}".format(exist_parent)]
                num_valid_roles = valid_roles.filter(role__pk__in=roles).count()
                if len(roles) != num_valid_roles:
                    # check whether the logged-in user has permission to grant all
                    # the roles to the edit subgroup
                    err_msg['roles'] = ["list of roles contains invalid ID {}".format(str(roles)) ]
                if any(err_msg):
                    result = False
                err_msgs.append(err_msg)
        except (ValueError, TypeError) as e:
            # still return 403 to frontend, but log this frontend input error (TODO)
            err_msgs = {api_settings.NON_FIELD_ERRORS_KEY: ["unknown error from frontend input"]}
            result = False
        if not result:
            self.message = err_msgs
            log_args.extend(['err_msgs', err_msgs])
        log_args.extend(['result', result])
        loglevel = logging.DEBUG if result else logging.WARNING
        _logger.log(loglevel, None, *log_args, request=request)
        return result


    def has_delete_permission(self, request, view):
        result = True
        err_msgs = {}
        log_args = []
        try:  # supposed to get list of IDs from URL
            IDs = request.query_params.get('ids', '')
            IDs = IDs.split(',')
            IDs = [int(i) for i in IDs if not i in EMPTY_VALUES]
            valid_grps  = self._get_valid_groups(account=request.user, view=view)
            num_valid_IDs = valid_grps['descendant'].filter(descendant__pk__in=IDs).count()
            log_args.extend(['frontend_IDs', IDs, 'valid_grps_descendant', valid_grps['descendant'],
                'num_valid_IDs', num_valid_IDs])
            if len(IDs) != num_valid_IDs:
                err_msgs = {api_settings.NON_FIELD_ERRORS_KEY: "The list %s contains invalid ID" % IDs}
                result = False
        except (ValueError, TypeError) as e:
            err_msgs = {api_settings.NON_FIELD_ERRORS_KEY: "unknown error from frontend input"}
            result = False
        if not result:
            self.message = err_msgs
            log_args.extend(['err_msgs', err_msgs])
        log_args.extend(['result', result])
        loglevel = logging.DEBUG if result else logging.WARNING
        _logger.log(loglevel, None, *log_args, request=request)
        return result


    # is it good practice to skip check on recovery permission ?
    _unsafe_methods = ['POST', 'PUT', 'DELETE'] # 'PATCH', 

    _extra_check_func = {
        'POST': has_edit_permission,
        'PUT': has_edit_permission,
        'DELETE': has_delete_permission,
    }

    def has_permission(self, request, view):
        result = super().has_permission(request=request, view=view)
        if result is True:
            account = request.user
            if not account.is_superuser:
                fn = self._extra_check_func.get(request.method)
                if fn:
                    result = fn(self=self, request=request, view=view,)
        return result

    def has_object_permission(self, request, view, obj):
        result = False
        account = request.user
        if account.is_superuser:
            result = True
        else:
            # conventionally this function is called for reading one specific object, not for updating
            # , so it is allowed to view all groups 
            valid_grps = self._get_valid_groups(account=account, view=view)
            result = valid_grps['all'].filter(descendant__pk=obj.pk).exists()
        return result


    # only for handling queryset permissions
    def filter_queryset(self, request, queryset, view):
        account = request.user
        if not account.is_superuser:
            valid_grps = self._get_valid_groups(account=account, view=view)
            if request.method in self._unsafe_methods:
                all_valid_grps = valid_grps['descendant']
            else:
                all_valid_grps = valid_grps['all']
            queryset = queryset.filter(pk__in=all_valid_grps)
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
        valid_roles = self._get_valid_roles(account=account, view=view)
        valid_grps  = self._get_valid_groups(account=account, view=view)
        valid_profs = self._get_valid_profs(account=account, view=view)
        err_msgs = []
        try:
            for data in req_payld:
                err_msg = {}
                pid   = data.get('id')
                grps  = data.get('groups', [])
                roles = data.get('roles', [])
                num_valid_grps  = valid_grps['all'].filter(descendant__pk__in=grps).count()
                num_valid_roles = valid_roles.filter(role__pk__in=roles).count()
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


    _unsafe_methods = ['POST', 'PUT', 'DELETE']

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


class UserDeactivationPermission(DjangoModelPermissions, BaseValidObjectsMixin):
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


