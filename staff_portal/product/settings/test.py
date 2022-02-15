import json
from .common import *

proj_path = BASE_DIR
secrets_path = proj_path.joinpath('common/data/secrets.json')
secrets = None

AUTH_KEYSTORE['persist_secret_handler_test'] = {
    'module_path': 'common.auth.keystore.JWKSFilePersistHandler',
    'init_kwargs': {
        'filepath': './tmp/cache/test/jwks/privkey/current.json',
        'name':'secret', 'expired_after_days': 7, 'flush_threshold':4,
    },
}

AUTH_KEYSTORE['persist_pubkey_handler_test'] = {
    'module_path': 'common.auth.keystore.JWKSFilePersistHandler',
    'init_kwargs': {
        'filepath': './tmp/cache/test/jwks/pubkey/current.json',
        'name':'pubkey', 'expired_after_days': 9, 'flush_threshold':4,
    },
}

with open(secrets_path, 'r') as f:
    secrets = json.load(f)
    secrets = secrets['backend_apps']['databases']['test_site_dba']

# Django test only uses `default` alias , which does NOT allow users to switch
# between different database credentials
DATABASES['default'].update(secrets)
DATABASES['default']['NAME'] = DATABASES['default']['TEST']['NAME']

DATABASE_ROUTERS.clear()

