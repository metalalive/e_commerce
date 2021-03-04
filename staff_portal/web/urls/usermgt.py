from django.urls import include, path, re_path

from ..views.usermgt import DashBoardView, AuthRoleAddHTMLView, AuthRoleUpdateHTMLView, QuotaUsageTypeAddHTMLView, QuotaUsageTypeUpdateHTMLView
from ..views.usermgt import UserGroupsAddHTMLView, UserGroupsUpdateHTMLView, UserProfileAddHTMLView, UserProfileUpdateHTMLView
from ..views.usermgt import AccountCreateHTMLView, UsernameRecoveryRequestHTMLView, UnauthPasswdRstReqHTMLView, UnauthPasswdRstHTMLView


app_name = 'usermgt'

urlpatterns = [
    path('dashboard',  DashBoardView.as_view()),

    path('roles/add',    AuthRoleAddHTMLView.as_view() ),
    path('roles/update', AuthRoleUpdateHTMLView.as_view() ),
    path('quota/add',    QuotaUsageTypeAddHTMLView.as_view()  ),
    path('quota/update', QuotaUsageTypeUpdateHTMLView.as_view()  ),
    path('usergrps/add',     UserGroupsAddHTMLView.as_view() ),
    path('usergrps/update',  UserGroupsUpdateHTMLView.as_view() ),
    path('usrprofs/add',     UserProfileAddHTMLView.as_view()  ),
    path('usrprofs/update',  UserProfileUpdateHTMLView.as_view()  ),

    path('account/create/<slug:token>',  AccountCreateHTMLView.as_view()  ),
    path('username/recovery',  UsernameRecoveryRequestHTMLView.as_view()  ),
    path('password/reset',    UnauthPasswdRstReqHTMLView.as_view()  ),
    path('password/reset/<slug:token>',  UnauthPasswdRstHTMLView.as_view()  ),
] # end of urlpatterns

