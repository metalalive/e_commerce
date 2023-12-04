from common.util.python import load_config_to_module

ALLOWED_ORIGIN  = None

ALLOWED_METHODS = None

ALLOWED_HEADERS = None

ALLOW_CREDENTIALS = False

PREFLIGHT_MAX_AGE = 60


load_config_to_module(cfg_path='common/data/cors.json', module_path=__name__)

