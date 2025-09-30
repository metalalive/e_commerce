import os
import logging
from datetime import timedelta, date
from typing import List
from pathlib import Path

from django.utils import timezone as django_timezone
from django.utils.module_loading import import_string
from celery.backends.rpc import RPCBackend as CeleryRpcBackend

from ecommerce_common.auth.keystore import create_keystore_helper
from ecommerce_common.util.messaging.constants import RPC_EXCHANGE_DEFAULT_NAME
from ecommerce_common.util.celery import app as celery_app
from ecommerce_common.logging.util import log_fn_wrapper

from .models.base import GenericUserGroup, GenericUserProfile
from .models.auth import UnauthResetAccountRequest

_logger = logging.getLogger(__name__)

srv_basepath = Path(os.environ["SYS_BASE_PATH"]).resolve(strict=True)


@celery_app.task(bind=True, queue="usermgt_default")
def update_accounts_privilege(self, affected_groups, deleted=False):
    # TODO, may repeat the task after certain time interval if it failed in the middle
    # (until it's successfully completed)
    profiles = GenericUserGroup.get_profiles_under_groups(grp_ids=affected_groups, deleted=deleted)
    GenericUserProfile.update_accounts_privilege(profiles)
    return True


@celery_app.task
@log_fn_wrapper(logger=_logger, loglevel=logging.INFO)
def clean_expired_reset_requests(days, hours=0, minutes=0):
    td = timedelta(days=days, hours=hours, minutes=minutes)
    t0 = django_timezone.now()
    t0 = t0 - td
    expired = UnauthResetAccountRequest.objects.filter(time_created__lt=t0)
    result = expired.values("email__user_id", "email__user_type", "email__addr", "time_created")
    result = list(result)
    expired.delete()
    return result


def _rotate_keystores_setup(module_setup):
    hdlr_args = module_setup["persist_secret_handler"]["init_kwargs"]
    if hdlr_args.get("filepath"):  # TODO, better design approach
        hdlr_args["filepath"] = os.path.join(srv_basepath, hdlr_args["filepath"])
    hdlr_args = module_setup["persist_pubkey_handler"]["init_kwargs"]
    if hdlr_args.get("filepath"):
        hdlr_args["filepath"] = os.path.join(srv_basepath, hdlr_args["filepath"])
    keystore = create_keystore_helper(cfg=module_setup, import_fn=import_string)
    key_size_in_bits = module_setup["key_size_in_bits"]
    num_keys = module_setup.get("num_keys", keystore.DEFAULT_NUM_KEYS)
    date_limit = None
    if module_setup.get("date_limit", None):
        date_limit = date.fromisoformat(module_setup["date_limit"])
    keygen_handler_module = import_string(module_setup["keygen_handler"]["module_path"])
    keygen_handler_kwargs = module_setup["keygen_handler"].get("init_kwargs", {})
    keygen_handler = keygen_handler_module(**keygen_handler_kwargs)
    return keystore.rotate(
        keygen_handler=keygen_handler,
        key_size_in_bits=key_size_in_bits,
        num_keys=num_keys,
        date_limit=date_limit,
    )


@celery_app.task(queue="usermgt_default")
@log_fn_wrapper(logger=_logger, loglevel=logging.INFO)
def rotate_keystores(modules_setup):
    """cron job to update given list of key stores periodically"""
    # TODO, clean up old files
    results = map(_rotate_keystores_setup, modules_setup)
    return list(results)


@celery_app.task(
    backend=CeleryRpcBackend(app=celery_app),
    queue="rpc_usermgt_get_profile",
    bind=True,
    exchange=RPC_EXCHANGE_DEFAULT_NAME,
    routing_key="rpc.user_management.get_profile",
)
@log_fn_wrapper(logger=_logger, loglevel=logging.WARNING, log_if_succeed=False)
def get_profile(self, ids: List[int], fields: List[str]):
    from .serializers import GenericUserProfileSerializer
    from .serializers.common import (
        serialize_profile_quota,
        serialize_profile_permissions,
    )

    present_quota = "quota" in fields
    present_roles = "roles" in fields
    if present_quota:
        fields.remove("quota")
    if present_roles:
        fields.remove("roles")
    if "id" not in fields:
        fields.append("id")
    qset = GenericUserProfile.objects.filter(id__in=ids)
    req = self.request  # retrieve extra headers from celery task context
    src_app_label = req.headers.get("src_app") if req.headers else None
    if not src_app_label and (present_quota or present_roles):
        raise ValueError("src_app_label is required for fetching quota/roles of user profile")

    class fake_request:
        query_params = {"fields": ",".join(fields)}

    extra_context = {"request": fake_request}
    serializer = GenericUserProfileSerializer(many=True, instance=qset, context=extra_context)
    data = serializer.data
    for d in data:
        profile = qset.get(id=d["id"])
        if present_quota:
            d["quota"] = serialize_profile_quota(profile, app_labels=[src_app_label])
        if present_roles:
            d["perms"] = serialize_profile_permissions(profile, app_labels=[src_app_label])
    return data


@celery_app.task(
    backend=CeleryRpcBackend(app=celery_app),
    queue="rpc_usermgt_profile_descendant_validity",
    exchange=RPC_EXCHANGE_DEFAULT_NAME,
    routing_key="rpc.user_management.profile_descendant_validity",
)
@log_fn_wrapper(logger=_logger, loglevel=logging.WARNING, log_if_succeed=False)
def profile_descendant_validity(asc: int, descs: List[int]) -> List[int]:
    asc_prof = GenericUserProfile.objects.filter(id=asc).first()
    assert asc_prof, "invalid profile ID for ancestor"
    grps_applied = asc_prof.groups.values_list("group__id", flat=True)
    valid_desc_profs = GenericUserGroup.get_profiles_under_groups(
        grp_ids=grps_applied, deleted=False
    )
    valid_desc_ids = valid_desc_profs.filter(id__in=descs).values_list("id", flat=True)
    return list(valid_desc_ids)
