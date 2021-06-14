
from django.urls import  path
from ..views.usermgt import UserGroupsProxyView, UserProfilesProxyView, AppliedRoleProxyView, AppliedGroupProxyView
from ..views.usermgt import AuthRoleProxyView, UserQuotaProxyView, UserLowLvlPermProxyView
from ..views.usermgt import AccountActivationProxyView, AccountDeactivationProxyView, AccountCreateProxyView
from ..views.usermgt import UsernameRecoveryReqProxyView, UnauthPasswdRstReqProxyView, UnauthPasswdRstProxyView
from ..views.usermgt import AuthUsernameEditProxyView, AuthPasswdEditProxyView, RemoteAuthProxyView

app_name = 'usermgt'

urlpatterns = [
    path('low_lvl_perms',    UserLowLvlPermProxyView.as_view()  ),
    path('quota',            UserQuotaProxyView.as_view()  ),
    path('roles',            AuthRoleProxyView.as_view()  ),
    path('role/<slug:rid>',  AuthRoleProxyView.as_view()  ),
    path('usrgrps',                UserGroupsProxyView.as_view()  ),
    path('usrgrp/<slug:grp_id>',   UserGroupsProxyView.as_view()  ),
    path('usrprofs',               UserProfilesProxyView.as_view() ),
    path('usrprof/<slug:prof_id>', UserProfilesProxyView.as_view() ),
    path('role_applied/<slug:role_id>', AppliedRoleProxyView.as_view() ),
    path('grps_applied/<slug:grp_id>', AppliedGroupProxyView.as_view() ),
    path('account/activate'  ,   AccountActivationProxyView.as_view() ),
    path('account/deactivate',   AccountDeactivationProxyView.as_view() ),
    path('account/create/<slug:token>',  AccountCreateProxyView.as_view()  ),
    path('username/recovery',  UsernameRecoveryReqProxyView.as_view()  ),
    path('password/reset',    UnauthPasswdRstReqProxyView.as_view()  ),
    path('password/reset/<slug:token>',  UnauthPasswdRstProxyView.as_view()  ),
    path('username/edit', AuthUsernameEditProxyView.as_view()),
    path('password/edit', AuthPasswdEditProxyView.as_view()),
    path('remote_auth',   RemoteAuthProxyView.as_view()  ),
]

