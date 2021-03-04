from .common import BaseRevProxyView
from common.views.proxy.mixins import _render_url_path


def _get_path_list_or_item_api(proxyview, request, key_vars):
    #full_path = request.get_full_path()
    #print('check the full path from client %s' % (full_path))
    if any(key_vars):
        out = proxyview.path_pattern[1].format(**key_vars)
    else:
        out = proxyview.path_pattern[0]
    return out



class AppBaseProxyView(BaseRevProxyView):
    dst_host = 'http://localhost:8008'
    authenticate_required = {
        'OPTIONS': True, 'GET': True, 'POST': True,
        'PUT': True,  'PATCH': True, 'DELETE': True,
    }


class UserGroupsProxyView(AppBaseProxyView):
    path_pattern = ['usrgrps', 'usrgrp/{grp_id}']
    path_handler = _get_path_list_or_item_api
    path_var_keys = ['grp_id']


class UserProfilesProxyView(AppBaseProxyView):
    path_pattern = ['usrprofs', 'usrprof/{prof_id}']
    path_handler = _get_path_list_or_item_api
    path_var_keys = ['prof_id']


class AppliedRoleProxyView(AppBaseProxyView):
    path_pattern = 'applied_role/{role_id}'
    path_handler = _render_url_path
    path_var_keys = ['role_id']


class AppliedGroupProxyView(AppBaseProxyView):
    path_pattern = 'applied_group/{grp_id}'
    path_handler = _render_url_path
    path_var_keys = ['grp_id']


class AuthRoleProxyView(AppBaseProxyView):
    path_pattern = ['authroles', 'authrole/{rid}']
    path_handler = _get_path_list_or_item_api
    path_var_keys = ['rid']


class UserQuotaProxyView(AppBaseProxyView):
    path_pattern = 'quota'

class UserLowLvlPermProxyView(AppBaseProxyView):
    path_pattern = 'permissions'
    path_handler = _render_url_path

class AccountActivationProxyView(AppBaseProxyView):
    path_pattern = 'account/activate'
    authenticate_required = {'OPTIONS': True, 'POST': True,}

class AccountDeactivationProxyView(AppBaseProxyView):
    path_pattern = 'account/deactivate'
    authenticate_required = {'OPTIONS': True, 'POST': True,}

class AccountCreateProxyView(AppBaseProxyView):
    authenticate_required = {'OPTIONS': False, 'POST': False,}
    path_pattern = 'account/create/{token}'
    path_handler = _render_url_path
    path_var_keys = ['token']

class UsernameRecoveryReqProxyView(AppBaseProxyView):
    authenticate_required = {'OPTIONS': False, 'POST': False,}
    path_pattern = 'username/recovery'

class UnauthPasswdRstReqProxyView(AppBaseProxyView):
    authenticate_required = {'OPTIONS': False, 'POST': False,}
    path_pattern = 'password/reset'

class UnauthPasswdRstProxyView(AppBaseProxyView):
    authenticate_required = {'OPTIONS': False, 'PATCH': False,}
    path_pattern = 'password/reset/{token}'
    path_handler = _render_url_path
    path_var_keys = ['token']

class AuthUsernameEditProxyView(AppBaseProxyView):
    path_pattern = 'username/edit'
    authenticate_required = {'OPTIONS': True, 'PATCH': True,}

class AuthPasswdEditProxyView(AppBaseProxyView):
    path_pattern = 'password/edit'
    authenticate_required = {'OPTIONS': True, 'PATCH': True,}



