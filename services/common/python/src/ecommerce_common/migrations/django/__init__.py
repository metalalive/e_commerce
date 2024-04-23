import os
import pathlib
import shutil
import json

curr_path = os.path.dirname(os.path.realpath(__file__))
curr_path = pathlib.Path(curr_path)


def _check_dst_path(apps):
    for app in apps:
        app_path = os.path.dirname(app.__file__)
        migration_path = "/".join([app_path, "migrations"])
        exists = os.path.exists(migration_path)
        if not exists:
            os.mkdir(migration_path)
        ##file_names = os.listdir(migration_path)
        ##if any(file_names):
        ##    err_msg = 'There must not be any migration file existing in %s' % migration_path
        ##    raise FileExistsError(err_msg)


def auto_deploy(apps):
    _check_dst_path(apps=apps)
    for app in apps:
        app_path = os.path.dirname(app.__file__)
        migration_path = "/".join([app_path, "migrations"])
        app_name_folder = app_path.split("/")[-1]
        mi_file_src = tuple(
            filter(
                lambda f: f.is_dir() and f.name == app_name_folder, curr_path.iterdir()
            )
        )
        src_path = mi_file_src[0]
        dst_path = migration_path
        for file_ in src_path.iterdir():
            if not file_.is_file():
                continue
            if not file_.suffix in (".py",):
                continue
            shutil.copy(str(file_), dst_path)


def render_fixture(src_filepath, detail_fn):
    src_filepath = src_filepath.split("/")
    # update content type ID of GenericUserProfile to fixture file
    src_path = curr_path
    for part in src_filepath:
        filtered = tuple(filter(lambda f: f.name == part, src_path.iterdir()))
        src_path = filtered[0]
    assert src_path.is_file(), "src_path is NOT a file, at %s" % str(src_path)
    dst_path = str(src_path).split("/")
    dst_path[-1] = "renderred_%s" % dst_path[-1]
    dst_path = "/".join(dst_path)
    with open(src_path, "r") as f:
        src = json.load(f)
        dst = open(dst_path, "w")
        try:
            detail_fn(src=src)
            src = json.dumps(src)
            dst.write(src)
        finally:
            dst.close()
    return dst_path
