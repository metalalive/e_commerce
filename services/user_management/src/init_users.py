import os
import json
import secrets
from pathlib import Path
from typing import Any, Callable, Dict, List
from datetime import datetime, UTC

from django import setup
from django.core.management import call_command


def _render_usermgt_fixture(src: List[Dict[str, Any]]) -> None:
    from django.contrib.contenttypes.models import ContentType
    from django.contrib.auth.hashers import make_password
    from user_management.models import (
        GenericUserProfile,
        GenericUserAppliedRole,
        LoginAccount,
    )

    profile_ct_id = ContentType.objects.get_for_model(GenericUserProfile).pk
    now_time = "%sZ" % datetime.now(UTC).strftime("%Y-%m-%d %H:%M:%S.%f")

    for item in src:
        if item["fields"].get("user_type"):
            item["fields"]["user_type"] = profile_ct_id
        parts = item["model"].split(".")
        model_name = parts[-1]
        fields = item["fields"]
        if model_name == GenericUserProfile.__name__.lower():
            fields["time_created"] = now_time
            fields["last_updated"] = now_time
        elif model_name == GenericUserAppliedRole.__name__.lower():
            fields["expiry"] = None  # never expired, now_time
            item["pk"] = GenericUserAppliedRole.format_pk(
                usr_type=fields["user_type"],
                usr_id=fields["user_id"],
                role_id=fields["role"],
            )
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


def render_fixture(
    src_path: Path, detail_fn: Callable[[List[Dict[str, Any]]], None]
) -> Path:
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


def data_migration() -> None:
    os.environ.setdefault("DJANGO_SETTINGS_MODULE", "settings.migration")
    setup()
    import user_management
    from user_management.models import LoginAccount

    if LoginAccount.objects.exists():
        print("[INFO] User accounts already exist, skipping initial data migration.")
        return

    dst_path = os.path.dirname(user_management.__file__)
    renderred_fixture_path = render_fixture(
        src_path=Path(dst_path).parent.joinpath("data/app_init_fixtures.json"),
        detail_fn=_render_usermgt_fixture,
    )
    options = {"database": "usermgt_service"}
    call_command("loaddata", renderred_fixture_path, **options)
    os.remove(renderred_fixture_path)
    # MUST NOT keep the fixture data which contains password


# TODO
# - mail nortification after default admin account is created
if __name__ == "__main__":
    data_migration()
