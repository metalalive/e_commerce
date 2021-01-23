from common.util.python   import get_fixture_pks

from ..apps   import UserManagementConfig as UserMgtCfg

_PRESERVED_ROLE_IDS = get_fixture_pks(filepath='user_management.json', pkg_hierarchy='auth.group')
##print('_PRESERVED_ROLE_IDS : %s' % _PRESERVED_ROLE_IDS)

LOGIN_URL = "/{}/{}".format(UserMgtCfg.app_url, UserMgtCfg.api_url['LoginView'])

MAX_NUM_FORM = 7

