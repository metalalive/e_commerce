
WEB_HOST = 'http://localhost:8006'
API_GATEWAY_HOST = 'http://localhost:8007'
USERMGT_SERVICE_HOST = 'http://localhost:8008'

LOGIN_URL =  "/login"

MAX_NUM_FORM = 7

HTML_TEMPLATE_MAP = {
    'common': {
        'LoginView'             : 'user_management/Login.html',
        'Http400BadRequestView' : '400.html',
    },
    'usermgt': {
        'DashBoardView'          : 'user_management/DashBoard.html'     ,
        'AuthRoleAddHTMLView'    : 'user_management/AuthRoleCreate.html',
        'AuthRoleUpdateHTMLView' : 'user_management/AuthRoleEdit.html',
        'QuotaUsageTypeAddHTMLView'    : 'user_management/QuotaUsageTypeCreate.html',
        'QuotaUsageTypeUpdateHTMLView' : 'user_management/QuotaUsageTypeEdit.html',
        'UserGroupsAddHTMLView'     : 'user_management/UserGroupsCreate.html' ,
        'UserGroupsUpdateHTMLView'  : 'user_management/UserGroupsEdit.html'   ,
        'UserProfileAddHTMLView'    : 'user_management/UsersCreate.html'      ,
        'UserProfileUpdateHTMLView' : 'user_management/UsersEdit.html'        ,

        'AccountCreateHTMLView' : [
            'user_management/LoginAccountCreate.html',
            'user_management/AuthUserRequestTokenError.html'
            ],
        'UsernameRecoveryRequestHTMLView' : 'user_management/UsernameRecoveryRequest.html',
        'UnauthPasswdRstReqHTMLView'      : 'user_management/UnauthPasswordResetRequest.html',
        'UnauthPasswdRstHTMLView' : [
            'user_management/UnauthPasswordReset.html',
            'user_management/AuthUserRequestTokenError.html'
            ],
    },
    'productmgt': {
        'DashBoardView'  : 'product/DashBoard.html',
        'ProjectDevelopmentView': 'product/trello.html',
    },
} # end of HTML_TEMPLATE_MAP

