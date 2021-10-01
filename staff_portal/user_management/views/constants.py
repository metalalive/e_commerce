from common.util.python   import get_fixture_pks

_PRESERVED_ROLE_IDS = get_fixture_pks(filepath='fixtures.json', pkg_hierarchy='user_management.role')
##print('_PRESERVED_ROLE_IDS : %s' % _PRESERVED_ROLE_IDS)

# TODO, parameterize
WEB_HOST = 'http://localhost:8006'
LOGIN_WEB_URL = WEB_HOST+'/login'

MAX_NUM_FORM = 7

