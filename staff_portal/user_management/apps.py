import os

from django.apps import AppConfig


class WebAPIurlMeta(type):
    # URLs of web APIs , accessable to end client users
    _api_urls = {
            'AuthPermissionView'  :'api/permissions/',
            'DashBoardView'       : 'dashboard',

            'AuthRoleAddHTMLView'    : 'roles/add',
            'AuthRoleUpdateHTMLView' : 'roles/update',
            'AuthRoleAPIView'        : ['api/roles/', 'api/role/<slug:pk>/'],
            'AppliedRoleReadAPIView' : 'api/applied_role/<slug:pk>/',
            'AppliedGroupReadAPIView': 'api/applied_group/<slug:pk>/',

            'QuotaUsageTypeAddHTMLView'     : 'quota/add',
            'QuotaUsageTypeUpdateHTMLView'  : 'quota/update',
            'QuotaUsageTypeAPIView'  : "api/quota/",

            'UserGroupsAddHTMLView'  : 'usergrps/add',
            'UserGroupsUpdateHTMLView'  : 'usergrps/update',
            'UserGroupsAPIView'      : ["api/usrgrps/", "api/usrgrps/<slug:pk>/"],

            #### '^usergrps/edit/(?P<IDs>[\d/]+)$',
            'UserProfileAddHTMLView'      : 'usrprofs/add',
            'UserProfileUpdateHTMLView'   : 'usrprofs/update',
            'UserProfileAPIView'     : ["api/usrprofs/", "api/usrprofs/<slug:pk>/"],

            'UserActivationView'     : 'users/activate',
            'UserDeactivationView'   : 'users/deactivate',

            'LoginAccountCreateView'  : 'account/activate/<slug:token>',
            'UsernameRecoveryRequestView'     : 'username/recovery',
            'UnauthPasswordResetRequestView'  : 'password/reset',
            'UnauthPasswordResetView'         : 'password/reset/<slug:token>',

            'AuthUsernameEditAPIView': 'username/edit',
            'AuthPasswdEditAPIView':   'password/edit',
            'UserActionHistoryAPIReadView': 'activity_log',
            'DynamicLoglevelAPIView': 'log_level',

            'LoginView'              : 'login',
            'LogoutView'             : 'logout',
    } # end of _api_urls

    @classmethod
    def __getitem__(cls, key):
        out = ''
        try:
            out = cls._api_urls[key]
        except KeyError as err:
            err_msg = ['[', cls.__name__ ,']' ,', KeyError when searching for template file, no such key :', str(key)]
            #print("".join(err_msg))
        return out

    def __iter__(cls):
        cls._iter_api_url = iter(cls._api_urls)
        return cls

    def __next__(cls):
        key = next(cls._iter_api_url)
        return (key, cls._api_urls[key])

#### end of WebAPIurlMeta


class WebAPIurl(metaclass=WebAPIurlMeta):
    pass


class TemplateNameMeta(type):
    template_path  = 'user_management'

    _template_names = {
        'DashBoardView'       : 'DashBoard.html'        ,

        'AuthRoleAddHTMLView'    : 'AuthRoleCreate.html',
        'AuthRoleUpdateHTMLView' : 'AuthRoleEdit.html',

        'QuotaUsageTypeAddHTMLView'    : 'QuotaUsageTypeCreate.html',
        'QuotaUsageTypeUpdateHTMLView' : 'QuotaUsageTypeEdit.html',

        'UserGroupsAddHTMLView' : 'UserGroupsCreate.html' ,
        'UserGroupsUpdateHTMLView' : 'UserGroupsEdit.html'   ,

        'UserProfileAddHTMLView'    : 'UsersCreate.html'      ,
        'UserProfileUpdateHTMLView' : 'UsersEdit.html'        ,

        'LoginAccountCreateView' : ['LoginAccountCreate.html', 'AuthUserRequestTokenError.html'],

        'UsernameRecoveryRequestView'     : 'UsernameRecoveryRequest.html',
        'UnauthPasswordResetRequestView'  : 'UnauthPasswordResetRequest.html',
        'UnauthPasswordResetView'         : ['UnauthPasswordReset.html',   'AuthUserRequestTokenError.html'],

        'LoginView'             : 'Login.html',
        'Http400BadRequestView' : '400.html',
    } # end of _template_names

    @classmethod
    def __getitem__(cls, key):
        out = None
        try:
            item = cls._template_names[key]
            if isinstance(item, str):
                out = os.path.join(cls.template_path, item)
            elif isinstance(item, list):
                out = [os.path.join(cls.template_path, x) for x in item]
        except KeyError:
            err_msg = ['[', cls.__name__ ,']' ,', KeyError when searching for template file, no such key :', str(key)]
            print("".join(err_msg))
        return out


class TemplateName(metaclass=TemplateNameMeta):
    pass


class UserManagementConfig(AppConfig):
    name = 'user_management'
    app_url   = 'usermgt'
    api_url   = WebAPIurl
    template_name = TemplateName


