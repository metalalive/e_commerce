import os
import pathlib
import shutil
import json
import secrets
from datetime import datetime

curr_path = os.path.dirname(os.path.realpath(__file__))
curr_path = pathlib.Path(curr_path)

def _check_dst_path(apps):
    for app in apps:
        app_path = os.path.dirname(app.__file__)
        migration_path = '/'.join([app_path, 'migrations'])
        exists = os.path.exists(migration_path)
        if not exists:
            os.mkdir(migration_path)
        file_names = os.listdir(migration_path)
        if any(file_names):
            err_msg = 'There must not be any migration file existing in %s' % migration_path
            raise FileExistsError(err_msg)

def auto_deploy(apps):
    _check_dst_path(apps=apps)
    for app in apps:
        app_path = os.path.dirname(app.__file__)
        migration_path = '/'.join([app_path, 'migrations'])
        app_name_folder = app_path.split('/')[-1]
        mi_file_src = tuple(filter(lambda f:f.is_dir() and f.name == app_name_folder, curr_path.iterdir()))
        src_path = mi_file_src[0]
        dst_path = migration_path
        for file_ in src_path.iterdir():
            if not file_.is_file():
                continue
            if not file_.suffix in ('.py',):
                continue
            shutil.copy(str(file_), dst_path)


def render_fixture(src_filepath, detail_fn):
    src_filepath = src_filepath.split('/')
    # update content type ID of GenericUserProfile to fixture file
    src_path = curr_path
    for part in src_filepath:
        filtered = tuple(filter(lambda f: f.name == part, src_path.iterdir()))
        src_path = filtered[0]
    assert src_path.is_file(), 'src_path is NOT a file, at %s' % str(src_path)
    dst_path = str(src_path).split('/')
    dst_path[-1] = 'renderred_%s' % dst_path[-1]
    dst_path = '/'.join(dst_path)
    with open(src_path, 'r') as f:
        src = json.load(f)
        dst = open(dst_path,'w')
        try:
            detail_fn(src=src)
            src = json.dumps(src)
            dst.write(src)
        finally:
            dst.close()
    return dst_path


def _render_usermgt_fixture(src):
    from django.contrib.contenttypes.models import ContentType
    from django.contrib.auth.hashers import make_password
    from user_management.models import GenericUserProfile, GenericUserAppliedRole, LoginAccount
    profile_ct_id = ContentType.objects.get_for_model(GenericUserProfile).pk
    now_time = '%sZ' % datetime.utcnow().isoformat()
    for item in src:
        if item['fields'].get('user_type'):
            item['fields']['user_type'] = profile_ct_id
        parts = item['model'].split('.')
        model_name = parts[-1]
        fields = item['fields']
        pk = item['pk']
        if model_name == GenericUserProfile.__name__.lower():
            fields['time_created'] = now_time
            fields['last_updated'] = now_time
        elif model_name == GenericUserAppliedRole.__name__.lower():
            fields['last_updated'] = now_time
            pk['user_type'] = item['fields']['user_type']
        elif model_name == LoginAccount.__name__.lower():
            fields['last_login'] = now_time
            fields['date_joined'] = now_time
            fields['password_last_updated'] = now_time
            raw_passwd = secrets.token_urlsafe(16)
            fields['password'] = make_password(raw_passwd)
            info_msg = '[INFO] Default user account %s with random-generated password %s'
            info_msg = info_msg % (fields['username'], raw_passwd)
            print(info_msg)

