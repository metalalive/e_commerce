import json
import sys


def setup_secrets(secrets_path, module_path):
    _module = sys.modules[module_path]
    secrets = None
    with open(secrets_path, 'r') as f:
        secrets = json.load(f)
        secrets = secrets['backend_apps']
    assert secrets, "failed to load secrets from file"
    # Quick-start development settings - unsuitable for production
    # See https://docs.djangoproject.com/en/dev/howto/deployment/checklist/
    # SECURITY WARNING: keep the secret key used in production secret!
    if getattr(_module, 'SECRET_KEY', None) is None:
        setattr(_module, 'SECRET_KEY', secrets['secret_key']['staff_portal'])
    # part of SMTP server setup requires to read from secrets file
    if getattr(_module, 'EMAIL_HOST_PASSWORD', None) is None:
        if secrets.get('smtp', None):
            setattr(_module, 'EMAIL_HOST', secrets['smtp']['host'])
            setattr(_module, 'EMAIL_PORT', secrets['smtp']['port'])
            setattr(_module, 'EMAIL_HOST_USER', secrets['smtp']['username'])
            setattr(_module, 'EMAIL_HOST_PASSWORD', secrets['smtp']['password'])
    for key, setup in _module.DATABASES.items():
        if setup.get('PASSWORD', None):
            continue
        secret = secrets['databases'][key]
        setup.update(secret)


