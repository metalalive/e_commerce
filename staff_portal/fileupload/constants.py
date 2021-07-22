from common.util.python.fastapi.settings import settings as fa_settings

ACCEPT_DUPLICATE = True
ANONYMOUS_USER = '-1' # TODO, will be configurable parameter
app_cfg = fa_settings.apps['fileupload']

