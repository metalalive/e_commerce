import sys
import os
import json
import shutil
import secrets
from pathlib import Path
from datetime import datetime

from django import setup
from django.core.management import call_command

from ecommerce_common.migrations.django import auto_deploy


def _render_usermgt_fixture(src):
    from django.contrib.contenttypes.models import ContentType
    from django.contrib.auth.hashers import make_password
    from user_management.models import (
        GenericUserProfile,
        GenericUserAppliedRole,
        LoginAccount,
    )

    profile_ct_id = ContentType.objects.get_for_model(GenericUserProfile).pk
    now_time = "%sZ" % datetime.utcnow().isoformat()
    for item in src:
        if item["fields"].get("user_type"):
            item["fields"]["user_type"] = profile_ct_id
        parts = item["model"].split(".")
        model_name = parts[-1]
        fields = item["fields"]
        pk = item["pk"]
        if model_name == GenericUserProfile.__name__.lower():
            fields["time_created"] = now_time
            fields["last_updated"] = now_time
        elif model_name == GenericUserAppliedRole.__name__.lower():
            fields["expiry"] = None  # never expired, now_time
            pk["user_type"] = item["fields"]["user_type"]
        elif model_name == LoginAccount.__name__.lower():
            fields["last_login"] = now_time
            fields["date_joined"] = now_time
            fields["password_last_updated"] = now_time
            raw_passwd = secrets.token_urlsafe(16)
            fields["password"] = make_password(raw_passwd)
            info_msg = (
                "[INFO] Default user account %s with random-generated password %s"
            )
            info_msg = info_msg % (fields["username"], raw_passwd)
            print(info_msg)


def render_fixture(src_path, detail_fn):
    dst_filename = "renderred_%s" % src_path.name
    dst_path = src_path.parent.joinpath(dst_filename)
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


def init_migration():
    os.environ.setdefault("DJANGO_SETTINGS_MODULE", "settings.migration")
    setup()
    call_command("makemigrations", "contenttypes")
    call_command("makemigrations", "auth")
    call_command("makemigrations", "user_management")
    # --- schema migration ---
    import user_management

    dst_path = os.path.dirname(user_management.__file__)
    auto_deploy(label=user_management.apps.UserManagementConfig.name, dst_path=dst_path)
    options = {
        "database": "site_dba",
    }
    call_command("migrate", "contenttypes", **options)
    call_command("migrate", "auth", **options)
    call_command("migrate", "user_management", "0002", **options)
    # --- data migration ---
    renderred_fixture_path = render_fixture(
        src_path=Path(dst_path).parent.joinpath("data/app_init_fixtures.json"),
        detail_fn=_render_usermgt_fixture,
    )
    options = {
        "database": "usermgt_service",
    }
    call_command("loaddata", renderred_fixture_path, **options)
    os.remove(
        renderred_fixture_path
    )  # MUST NOT keep the fixture data which contains password


def deinit_migration():
    os.environ.setdefault("DJANGO_SETTINGS_MODULE", "settings.migration")
    setup()
    from django.contrib import auth
    import user_management

    options = {
        "database": "site_dba",
    }
    call_command("migrate", "user_management", "zero", **options)
    call_command("migrate", "auth", "zero", **options)
    call_command("migrate", "contenttypes", "zero", **options)
    apps = (
        user_management,
        auth,
    )
    for app in apps:
        app_path = Path(app.__file__).resolve(strict=True)
        migration_path = app_path.parent.joinpath("migrations")
        if migration_path.exists():
            shutil.rmtree(migration_path)


if __name__ == "__main__":
    if sys.argv[-1] == "reverse":
        deinit_migration()
    else:
        init_migration()
