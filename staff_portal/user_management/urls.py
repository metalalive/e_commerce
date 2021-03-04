from django.urls import path

from .views.api  import AuthPermissionView, AuthRoleAPIView, AppliedRoleReadAPIView, AppliedGroupReadAPIView
from .views.api  import QuotaUsageTypeAPIView, QuotaMaterialReadAPIView, UserGroupsAPIView, UserProfileAPIView

from .views.api  import UserActivationView, UserDeactivationView, AuthPasswdEditAPIView, AuthUsernameEditAPIView
from .views.api  import AuthTokenReadAPIView, LoginAccountCreateView
from .views.api  import UsernameRecoveryRequestView, UnauthPasswordResetRequestView, UnauthPasswordResetView

from .apps import UserManagementConfig as UserMgtCfg


urlpatterns = [
    path(UserMgtCfg.api_url[AuthPermissionView.__name__],  AuthPermissionView.as_view()),

    path(UserMgtCfg.api_url[AuthRoleAPIView.__name__][0],     AuthRoleAPIView.as_view() ),
    path(UserMgtCfg.api_url[AuthRoleAPIView.__name__][1],     AuthRoleAPIView.as_view() ),
    path(UserMgtCfg.api_url[AppliedRoleReadAPIView.__name__], AppliedRoleReadAPIView.as_view() ),
    path(UserMgtCfg.api_url[AppliedGroupReadAPIView.__name__], AppliedGroupReadAPIView.as_view() ),

    path(UserMgtCfg.api_url[QuotaUsageTypeAPIView.__name__],  QuotaUsageTypeAPIView.as_view()),
    path(UserMgtCfg.api_url[QuotaMaterialReadAPIView.__name__],  QuotaMaterialReadAPIView.as_view()),

    path(UserMgtCfg.api_url[UserGroupsAPIView.__name__][0], UserGroupsAPIView.as_view()),
    path(UserMgtCfg.api_url[UserGroupsAPIView.__name__][1], UserGroupsAPIView.as_view()),
    path(UserMgtCfg.api_url[UserProfileAPIView.__name__][0], UserProfileAPIView.as_view()  ),
    path(UserMgtCfg.api_url[UserProfileAPIView.__name__][1], UserProfileAPIView.as_view()  ),

    path(UserMgtCfg.api_url[UserActivationView.__name__],    UserActivationView.as_view()  ),
    path(UserMgtCfg.api_url[UserDeactivationView.__name__],  UserDeactivationView.as_view()),

    path(UserMgtCfg.api_url[AuthTokenReadAPIView.__name__],  AuthTokenReadAPIView.as_view()  ),
    path(UserMgtCfg.api_url[LoginAccountCreateView.__name__],  LoginAccountCreateView.as_view()  ),
    path(UserMgtCfg.api_url[UsernameRecoveryRequestView.__name__],  UsernameRecoveryRequestView.as_view()  ),
    path(UserMgtCfg.api_url[UnauthPasswordResetRequestView.__name__],  UnauthPasswordResetRequestView.as_view()  ),
    path(UserMgtCfg.api_url[UnauthPasswordResetView.__name__],  UnauthPasswordResetView.as_view()  ),

    path(UserMgtCfg.api_url[AuthPasswdEditAPIView.__name__],  AuthPasswdEditAPIView.as_view()  ),
    path(UserMgtCfg.api_url[AuthUsernameEditAPIView.__name__],  AuthUsernameEditAPIView.as_view()  ),
] # end of urlpatterns


