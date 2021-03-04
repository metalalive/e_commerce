import json
import sys


ALLOWED_ORIGIN  = None

ALLOWED_METHODS = None

ALLOWED_HEADERS = None

ALLOW_CREDENTIALS = 'false'

PREFLIGHT_MAX_AGE = 60


def _load_config(cfg_path, module_path):
    _module = sys.modules[module_path]
    data = None
    with open(cfg_path, 'r') as f:
        data = json.load(f)
    assert data, "failed to load configuration from file %s" % cfg_path
    for key in _module.__dict__.keys():
        if data.get(key, None) is None:
            continue
        setattr(_module, key, data[key])


_load_config(cfg_path='common/data/cors.json', module_path=__name__)

