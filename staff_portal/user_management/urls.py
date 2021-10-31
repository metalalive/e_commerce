from django.urls import path

from .views.base import RoleAPIView, UserGroupsAPIView, UserProfileAPIView, AccountActivationView, AccountDeactivationView

from .views.auth  import LoginAccountCreateView, RefreshAccessTokenView, UsernameRecoveryRequestView, UnauthPasswordResetRequestView, UnauthPasswordResetView, AuthPasswdEditAPIView, AuthUsernameEditAPIView, LoginView, LogoutView, PermissionView, JWKSPublicKeyView

from .apps import UserManagementConfig as UserMgtCfg


urlpatterns = [
    path(UserMgtCfg.api_url[LoginView.__name__],  LoginView.as_view()),
    path(UserMgtCfg.api_url[LogoutView.__name__], LogoutView.as_view()),
    path(UserMgtCfg.api_url[RefreshAccessTokenView.__name__],  RefreshAccessTokenView.as_view()  ),

    path(UserMgtCfg.api_url[PermissionView.__name__],  PermissionView.as_view()),
    path(UserMgtCfg.api_url[RoleAPIView.__name__][0],  RoleAPIView.as_view() ),
    path(UserMgtCfg.api_url[RoleAPIView.__name__][1],  RoleAPIView.as_view() ),

    path(UserMgtCfg.api_url[UserGroupsAPIView.__name__][0], UserGroupsAPIView.as_view()),
    path(UserMgtCfg.api_url[UserGroupsAPIView.__name__][1], UserGroupsAPIView.as_view()),
    path(UserMgtCfg.api_url[UserProfileAPIView.__name__][0], UserProfileAPIView.as_view()  ),
    path(UserMgtCfg.api_url[UserProfileAPIView.__name__][1], UserProfileAPIView.as_view()  ),

    path(UserMgtCfg.api_url[AccountActivationView.__name__],  AccountActivationView.as_view()),
    path(UserMgtCfg.api_url[AccountDeactivationView.__name__],  AccountDeactivationView.as_view()),

    path(UserMgtCfg.api_url[LoginAccountCreateView.__name__],  LoginAccountCreateView.as_view()  ),
    path(UserMgtCfg.api_url[UsernameRecoveryRequestView.__name__],  UsernameRecoveryRequestView.as_view()  ),
    path(UserMgtCfg.api_url[UnauthPasswordResetRequestView.__name__],  UnauthPasswordResetRequestView.as_view()  ),
    path(UserMgtCfg.api_url[UnauthPasswordResetView.__name__],  UnauthPasswordResetView.as_view()  ),

    path(UserMgtCfg.api_url[AuthPasswdEditAPIView.__name__],  AuthPasswdEditAPIView.as_view()  ),
    path(UserMgtCfg.api_url[AuthUsernameEditAPIView.__name__],  AuthUsernameEditAPIView.as_view()  ),
    path(UserMgtCfg.api_url[JWKSPublicKeyView.__name__],  JWKSPublicKeyView.as_view()  ),
] # end of urlpatterns


