from django.urls import include, path, re_path

from .views.api  import AuthPermissionView, AuthRoleAPIView, AppliedRoleReadAPIView, AppliedGroupReadAPIView
from .views.api  import QuotaUsageTypeAPIView, UserGroupsAPIView, UserProfileAPIView
from .views.html import DashBoardView, AuthRoleAddHTMLView, AuthRoleUpdateHTMLView, QuotaUsageTypeAddHTMLView, QuotaUsageTypeUpdateHTMLView
from .views.html import UserGroupsAddHTMLView, UserGroupsUpdateHTMLView, UserProfileAddHTMLView, UserProfileUpdateHTMLView

from .views.html import LoginAccountCreateView, UsernameRecoveryRequestView, UnauthPasswordResetRequestView, UnauthPasswordResetView, LoginView
from .views.api  import UserActivationView, UserDeactivationView, AuthPasswdEditAPIView, AuthUsernameEditAPIView
from .views.api  import LogoutView, UserActionHistoryAPIReadView, DynamicLoglevelAPIView

from .apps import UserManagementConfig as UserMgtCfg

app_name = UserMgtCfg.app_url

urlpatterns = [
    #### path('api-auth/', include('rest_framework.urls', namespace='rest_framework')),
    path(UserMgtCfg.api_url[AuthPermissionView.__name__],  AuthPermissionView.as_view()),

    path(UserMgtCfg.api_url[DashBoardView.__name__] ,       DashBoardView.as_view()      ),

    path(UserMgtCfg.api_url[AuthRoleAddHTMLView.__name__],     AuthRoleAddHTMLView.as_view() ),
    path(UserMgtCfg.api_url[AuthRoleUpdateHTMLView.__name__],  AuthRoleUpdateHTMLView.as_view() ),
    path(UserMgtCfg.api_url[AuthRoleAPIView.__name__][0],     AuthRoleAPIView.as_view() ),
    path(UserMgtCfg.api_url[AuthRoleAPIView.__name__][1],     AuthRoleAPIView.as_view() ),
    path(UserMgtCfg.api_url[AppliedRoleReadAPIView.__name__], AppliedRoleReadAPIView.as_view() ),
    path(UserMgtCfg.api_url[AppliedGroupReadAPIView.__name__], AppliedGroupReadAPIView.as_view() ),

    path(UserMgtCfg.api_url[QuotaUsageTypeAddHTMLView.__name__],    QuotaUsageTypeAddHTMLView.as_view()  ),
    path(UserMgtCfg.api_url[QuotaUsageTypeUpdateHTMLView.__name__], QuotaUsageTypeUpdateHTMLView.as_view()  ),
    path(UserMgtCfg.api_url[QuotaUsageTypeAPIView.__name__],  QuotaUsageTypeAPIView.as_view()),

    path(UserMgtCfg.api_url[UserGroupsAddHTMLView.__name__],  UserGroupsAddHTMLView.as_view() ),
    path(UserMgtCfg.api_url[UserGroupsUpdateHTMLView.__name__],  UserGroupsUpdateHTMLView.as_view() ),
    path(UserMgtCfg.api_url[UserGroupsAPIView.__name__][0], UserGroupsAPIView.as_view()),
    path(UserMgtCfg.api_url[UserGroupsAPIView.__name__][1], UserGroupsAPIView.as_view()),

    path(UserMgtCfg.api_url[UserProfileAddHTMLView.__name__],    UserProfileAddHTMLView.as_view()  ),
    path(UserMgtCfg.api_url[UserProfileUpdateHTMLView.__name__],    UserProfileUpdateHTMLView.as_view()  ),
    path(UserMgtCfg.api_url[UserProfileAPIView.__name__][0], UserProfileAPIView.as_view()  ),
    path(UserMgtCfg.api_url[UserProfileAPIView.__name__][1], UserProfileAPIView.as_view()  ),

    path(UserMgtCfg.api_url[UserActivationView.__name__],    UserActivationView.as_view()  ),
    path(UserMgtCfg.api_url[UserDeactivationView.__name__],  UserDeactivationView.as_view()),

    path(UserMgtCfg.api_url[LoginAccountCreateView.__name__],  LoginAccountCreateView.as_view()  ),

    path(UserMgtCfg.api_url[UsernameRecoveryRequestView.__name__],  UsernameRecoveryRequestView.as_view()  ),
    path(UserMgtCfg.api_url[UnauthPasswordResetRequestView.__name__],  UnauthPasswordResetRequestView.as_view()  ),
    path(UserMgtCfg.api_url[UnauthPasswordResetView.__name__],  UnauthPasswordResetView.as_view()  ),
    path(UserMgtCfg.api_url[AuthPasswdEditAPIView.__name__],  AuthPasswdEditAPIView.as_view()  ),
    path(UserMgtCfg.api_url[AuthUsernameEditAPIView.__name__],  AuthUsernameEditAPIView.as_view()  ),

    path(UserMgtCfg.api_url[UserActionHistoryAPIReadView.__name__],  UserActionHistoryAPIReadView.as_view()  ),
    path(UserMgtCfg.api_url[DynamicLoglevelAPIView.__name__],  DynamicLoglevelAPIView.as_view()  ),

    path(UserMgtCfg.api_url[LoginView.__name__],  LoginView.as_view()  ),
    path(UserMgtCfg.api_url[LogoutView.__name__], LogoutView.as_view() ),
] # end of urlpatterns


