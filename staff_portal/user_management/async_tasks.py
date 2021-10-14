import logging
from datetime  import datetime, timedelta, date

from django.utils.module_loading import import_string
from celery.backends.rpc import RPCBackend as CeleryRpcBackend

from common.auth.keystore import create_keystore_helper
from common.util.python.messaging.constants import  RPC_EXCHANGE_DEFAULT_NAME
from common.util.python.celery import app as celery_app
from common.logging.util  import log_fn_wrapper

from .models.base import GenericUserGroup
from .models.auth import AccountResetRequest
from django.contrib import auth

_logger = logging.getLogger(__name__)


@celery_app.task(bind=True, queue='usermgt_default')
def update_accounts_privilege(self, affected_groups, deleted=False):
    # TODO, may repeat the task after certain time interval if it failed in the middle
    # (until it's successfully completed)
    return GenericUserGroup.update_accounts_privilege(grp_ids=affected_groups, deleted=deleted)


@celery_app.task
@log_fn_wrapper(logger=_logger, loglevel=logging.INFO)
def clean_expired_auth_token(days):
    td = timedelta(days=days)
    t0 = datetime.now()
    t0 = t0 - td
    expired = AccountResetRequest.objects.filter(time_created__lt=t0)
    result = expired.values('id', 'profile__pk', 'email__email__addr', 'time_created')
    result = list(result)
    expired.delete()
    return result


def _rotate_keystores_setup(module_setup):
    keystore = create_keystore_helper(cfg=module_setup, import_fn=import_string)
    key_size_in_bits = module_setup['key_size_in_bits']
    num_keys = module_setup.get('num_keys', keystore.DEFAULT_NUM_KEYS)
    date_limit = None
    if module_setup.get('date_limit', None):
        date_limit = date.fromisoformat(module_setup['date_limit'])
    keygen_handler_module = import_string(module_setup['keygen_handler']['module_path'])
    keygen_handler_kwargs = module_setup['keygen_handler'].get('init_kwargs', {})
    keygen_handler = keygen_handler_module(**keygen_handler_kwargs)
    return keystore.rotate(keygen_handler=keygen_handler, key_size_in_bits=key_size_in_bits,
            num_keys=num_keys, date_limit=date_limit)


@celery_app.task(queue='usermgt_default')
@log_fn_wrapper(logger=_logger, loglevel=logging.INFO)
def rotate_keystores(modules_setup):
    """ cron job to update given list of key stores periodically """
    # TODO, clean up old files
    results = map(_rotate_keystores_setup , modules_setup)
    return list(results)


@celery_app.task(backend=CeleryRpcBackend(app=celery_app), queue='rpc_usermgt_get_profile', exchange=RPC_EXCHANGE_DEFAULT_NAME, \
        routing_key='rpc.user_management.get_profile')
@log_fn_wrapper(logger=_logger, loglevel=logging.WARNING, log_if_succeed=False)
def get_profile(account_id, field_names, services_label=None):
    account_id = int(account_id)
    account = auth.get_user_model().objects.get(pk=account_id)
    profile = account.profile
    data = profile.serializable(present=field_names, services_label=services_label)
    return data


