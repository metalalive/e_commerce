from django.apps import AppConfig
from ecommerce_common.util import BaseUriLookup


class WebAPIurl(BaseUriLookup):
    # URLs of web APIs , accessable to end client users
    _urls = {
        "LoginView": "login",
        "LogoutView": "logout",
        "RefreshAccessTokenView": "refresh_access_token",
        "PermissionView": "permissions",
        "RoleAPIView": ["roles", "role/<slug:pk>"],
        "UserGroupsAPIView": ["groups", "group/<slug:pk>"],
        "UserProfileAPIView": ["profiles", "profile/<slug:pk>"],
        "AccountActivationView": "account/activate",
        "AccountDeactivationView": "account/deactivate",
        "LoginAccountCreateView": "account/create/<slug:token>",
        "UsernameRecoveryRequestView": "account/username/recovery",
        "UnauthPasswordResetRequestView": "account/password/reset",
        "UnauthPasswordResetView": "account/password/reset/<slug:token>",
        #### '^usergrps/edit/(?P<IDs>[\d/]+)$',
        "AuthUsernameEditAPIView": "account/username",
        "AuthPasswdEditAPIView": "account/password",
        "JWKSPublicKeyView": "jwks",
    }  # end of _urls


#### end of WebAPIurlMeta


class UserManagementConfig(AppConfig):
    name = "user_management"
    app_url = "usermgt"
    api_url = WebAPIurl()

    def ready(self):
        from ecommerce_common.util.celery import app as celery_app
        from . import celeryconfig

        if celery_app.configured is False:  # avoid re-configuration
            celery_app.config_from_object(celeryconfig)
        celeryconfig.init_rpc(app=celery_app)

        from ecommerce_common.util.messaging.monkeypatch import patch_kombu_pool

        patch_kombu_pool()
        # add --noreload to avoid django runserver from loading twice initially
