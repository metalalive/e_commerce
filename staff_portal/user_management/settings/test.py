import json
from .common import *

proj_path = BASE_DIR
secrets_path = proj_path.joinpath('common/data/secrets.json')
secrets = None

with open(secrets_path, 'r') as f:
    secrets = json.load(f)
    secrets = secrets['backend_apps']['databases']['test_site_dba']

# Django test only uses `default` alias , which does NOT allow users to switch
# between different database credentials
DATABASES['default'].update(secrets)
## does NOT work for testing
##DATABASES['usermgt_service'].update(secrets)
DATABASE_ROUTERS.clear()

