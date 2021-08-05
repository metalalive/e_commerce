import json
import sys
import os

def _refresh_test_enable():
    cmd_entry = 'manage.py'
    cmd_type  = 'test'
    cmd_entry_detected = False
    cmd_type_detected  = False
    #print('do we have test command ? %s' % sys.argv)
    for arg in sys.argv:
        if arg.endswith(cmd_entry):
            cmd_entry_detected = True
        elif arg == cmd_type and cmd_entry_detected:
            cmd_type_detected = True
    return  cmd_entry_detected and cmd_type_detected


def setup_secrets(secrets_path, module_path, portal_type, interface_type):
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
        key = secrets['secret_key'][portal_type][interface_type]
        setattr(_module, 'SECRET_KEY', key)
    # part of SMTP server setup requires to read from secrets file
    if getattr(_module, 'EMAIL_HOST_PASSWORD', None) is None:
        if secrets.get('smtp', None):
            setattr(_module, 'EMAIL_HOST', secrets['smtp']['host'])
            setattr(_module, 'EMAIL_PORT', secrets['smtp']['port'])
            setattr(_module, 'EMAIL_HOST_USER', secrets['smtp']['username'])
            setattr(_module, 'EMAIL_HOST_PASSWORD', secrets['smtp']['password'])
    # database
    for key, setup in _module.DATABASES.items():
        if setup.get('PASSWORD', None):
            continue
        if test_enable :
            if key == 'default':
                key = 'test_site_dba'
            else:
                continue
        secret = secrets['databases'][key]
        setup.update(secret)
    if test_enable:
        # in this project, I simply need different user credentials that access
        # to the same database, however Django doesn't seem to recognize this
        # when running test cases so it will attempt to create duplicate test
        # databases using the same name but different user accounts from different
        # aliases in settings.DATABASE dictionary.
        # which is the reason I need to evict all other non-default database
        # aliases when running Django tests .
        _module.DATABASE_ROUTERS.clear()
        db_keys = set(_module.DATABASES.keys()) - set(['default'])
        list(map(lambda k: _module.DATABASES.pop(k), db_keys))
    #import pdb
    #pdb.set_trace()

    base_dir = _module.BASE_DIR
    dev_status = 'test' if test_enable else 'production'
    # cache 
    mod_caches = getattr(_module, 'CACHES', {})
    for key,setup in mod_caches.items():
        secret = secrets['caches'][dev_status][key]
        location = secret.get('LOCATION', None)
        if location:
            secret['LOCATION'] = os.path.join(base_dir, location)
        setup.update(secret)
    # session
    sess_engine = getattr(_module, 'SESSION_ENGINE', None)
    if sess_engine:
        if sess_engine == 'common.sessions.backends.file':
            filepath = secrets['sessions'][dev_status]['filepath']
            filepath = os.path.join(base_dir , filepath)
            setattr(_module, 'SESSION_FILE_PATH', filepath)

## end of setup_secrets


test_enable = _refresh_test_enable()

