
from django.contrib.auth.models import Permission as AuthPermission, User as AuthUser, Group as AuthRole, LoginAccountRoleRelation

from user_management.models import GenericUserProfile, GenericUserAuthRelation

_fixtures = {
    'AuthUser': [
        {'id':14, 'is_superuser':False, 'is_staff':True,  'is_active':True,  'username': 'AltinGun','password': '93rutGrPt'} ,
        {'id':19, 'is_superuser':False, 'is_staff':False, 'is_active':True,  'username': 'KingGizz','password': '39rjfR@et'} ,
        {'id':10, 'is_superuser':False, 'is_staff':True,  'is_active':False, 'username': 'Imarhan', 'password': 'if74w#gfy'} ,
        {'id':7,  'is_superuser':True,  'is_staff':False, 'is_active':True,  'username': 'yuk0p1ano', 'password': 'anti@s0cia1'} ,
        {'id':8,  'is_superuser':True,  'is_staff':False, 'is_active':False, 'username': 'remoteCtrl','password': '9rJ3yf740fM'} ,
    ],
    'GenericUserProfile': [
        {'id':2, 'first_name':'Jon', 'last_name':'Snow'},
        {'id':3, 'first_name':'Shelton', 'last_name':'Cooper'},
        {'id':4, 'first_name':'Kenny',  'last_name':'McCormick'},
    ],
}

class AuthCheckMixin:
    def test_invalid_account_without_profile(self, testcase, path, methods):
        _account_info = _fixtures['AuthUser'][0]
        account = AuthUser(**_account_info)
        account.set_password(_account_info['password'])
        account.save()
        http_forwarded = testcase._forwarded_pattern % _account_info['username']
        headers = {'HTTP_FORWARDED': http_forwarded,}
        for method in methods:
            response = testcase._send_request_to_backend(path=path, method=method, headers=headers)
            testcase.assertEqual(int(response.status_code), 403)

    def test_inactive_staff(self, testcase, path, methods):
        _account_info = _fixtures['AuthUser'][0].copy()
        _account_info.update({'is_active':False, 'is_superuser':False, 'is_staff':True})
        account = AuthUser.objects.create_user(**_account_info)
        profile = GenericUserProfile(**_fixtures['GenericUserProfile'][0])
        profile.save()
        auth_rel = GenericUserAuthRelation(profile=profile, login=account)
        auth_rel.save()
        http_forwarded = testcase._forwarded_pattern % _account_info['username']
        headers = {'HTTP_FORWARDED': http_forwarded,}
        for method in methods:
            response = testcase._send_request_to_backend(path=path, method=method, headers=headers)
            testcase.assertEqual(int(response.status_code), 403)

    def test_unauthorized_user(self, testcase, path, methods):
        _account_info = _fixtures['AuthUser'][0].copy()
        _account_info.update({'is_active':True, 'is_superuser':False, 'is_staff':False})
        account = AuthUser.objects.create_user(**_account_info)
        profile = GenericUserProfile.objects.create(**_fixtures['GenericUserProfile'][0])
        auth_rel = GenericUserAuthRelation.objects.create(profile=profile, login=account)
        http_forwarded = testcase._forwarded_pattern % _account_info['username']
        headers = {'HTTP_FORWARDED': http_forwarded,}
        for method in methods:
            response = testcase._send_request_to_backend(path=path, method=method, headers=headers)
            testcase.assertEqual(int(response.status_code), 403)

    def test_unauthorized_staff(self, testcase, path, methods):
        _account_info = _fixtures['AuthUser'][0].copy()
        _account_info.update({'is_active':True, 'is_superuser':False, 'is_staff':True})
        account = AuthUser.objects.create_user(**_account_info)
        profile = GenericUserProfile.objects.create(**_fixtures['GenericUserProfile'][0])
        auth_rel = GenericUserAuthRelation.objects.create(profile=profile, login=account)
        http_forwarded = testcase._forwarded_pattern % _account_info['username']
        headers = {'HTTP_FORWARDED': http_forwarded,}
        for method in methods:
            response = testcase._send_request_to_backend(path=path, method=method, headers=headers)
            testcase.assertEqual(int(response.status_code), 403)



