from django.apps import AppConfig
from common.util.python import BaseUriLookup, BaseTemplateLookup

class WebAPIurl(BaseUriLookup):
    # URLs of web APIs , accessable to end client users
    _urls = {
            'LoginView' :'login',
            'LogoutView':'logout',
            'RefreshAccessTokenView': 'refresh_access_token',

            'PermissionView': 'permissions',
            'RoleAPIView'        : ['roles', 'role/<slug:pk>'],
            'UserGroupsAPIView'  : ["groups", "group/<slug:pk>"],
            'UserProfileAPIView' : ["profiles", "profile/<slug:pk>"],
            'AccountActivationView'   : 'account/activate',
            'AccountDeactivationView' : 'account/deactivate',
            'LoginAccountCreateView'  : 'account/create/<slug:token>',

            #### '^usergrps/edit/(?P<IDs>[\d/]+)$',

            'AuthTokenReadAPIView'    : 'authtoken/<slug:token>',
            'UsernameRecoveryRequestView'     : 'username/recovery',
            'UnauthPasswordResetRequestView'  : 'password/reset',
            'UnauthPasswordResetView'         : 'password/reset/<slug:token>',

            'AuthUsernameEditAPIView': 'username/edit',
            'AuthPasswdEditAPIView':   'password/edit',
    } # end of _urls
#### end of WebAPIurlMeta


class UserManagementConfig(AppConfig):
    name = 'user_management'
    app_url   = 'usermgt'
    api_url   = WebAPIurl()

    def ready(self):
        from common.util.python.celery import app as celery_app
        from . import celeryconfig
        if celery_app.configured is False: # avoid re-configuration
            celery_app.config_from_object(celeryconfig)
        celeryconfig.init_rpc(app=celery_app)

        from common.util.python.messaging.monkeypatch import patch_kombu_pool
        patch_kombu_pool()
        from common.models.db import monkeypatch_django_db
        monkeypatch_django_db()
        # add --noreload to avoid django runserver from loading twice initially


