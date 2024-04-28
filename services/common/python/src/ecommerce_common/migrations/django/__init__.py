import os
import pathlib
import shutil

curr_path = os.path.dirname(os.path.realpath(__file__))
curr_path = pathlib.Path(curr_path)


def _ensure_dst_path(dst_path:str) -> str:
    migration_path = "/".join([dst_path, "migrations"])
    if not os.path.exists(migration_path):
        os.mkdir(migration_path)
    return migration_path
    ##file_names = os.listdir(migration_path)
    ##if any(file_names):
    ##    err_msg = 'There must not be any migration file existing in %s' % migration_path
    ##    raise FileExistsError(err_msg)


def auto_deploy(label:str, dst_path:str):
    dst_path = _ensure_dst_path(dst_path)
    src_path = curr_path.joinpath(label)
    for file_ in src_path.iterdir():
        if not file_.is_file():
            continue
        if not file_.suffix in (".py",):
            continue
        shutil.copy(str(file_), dst_path)
