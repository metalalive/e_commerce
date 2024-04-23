import os
from pathlib import Path
from ecommerce_common.util import load_config_to_module

ALLOWED_ORIGIN = None

ALLOWED_METHODS = None

ALLOWED_HEADERS = None

ALLOW_CREDENTIALS = False

PREFLIGHT_MAX_AGE = 60


basepath = Path(os.environ["SERVICE_BASE_PATH"]).resolve(strict=True)
fullpath = os.path.join(basepath, "common/data/cors.json")
load_config_to_module(cfg_path=fullpath, module_path=__name__)
