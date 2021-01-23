import logging
from datetime  import datetime, timedelta

from django.db     import  IntegrityError, transaction

from common.util.python.celery import app as celery_app
from common.util.python import log_wrapper

from .models import GenericUserGroup, GenericUserGroupClosure, GenericUserProfile
from .models import GenericUserAppliedRole, GenericUserGroupRelation, AuthUserResetRequest

_logger = logging.getLogger(__name__)


@celery_app.task(bind=True, queue='usermgt_default')
def update_roles_on_accounts(self, affected_groups, deleted=False):
    done = False
    affected_groups_origin = affected_groups
    if deleted:
        qset = GenericUserGroupClosure.objects.get_deleted_set()
    else:
        qset = GenericUserGroupClosure.objects.all()
    qset = qset.filter(ancestor__pk__in=affected_groups)
    affected_groups = qset.values_list('descendant__pk', flat=True)
    # always update roles in soft-deleted / deactivated user accounts
    kwargs_prof = {'group__pk__in': affected_groups, 'with_deleted':True}
    qset = GenericUserGroupRelation.objects.filter(**kwargs_prof)
    profile_ids = qset.values_list('profile__pk', flat=True)
    log_args = ['affected_groups', affected_groups, 'profile_ids', profile_ids,
            'affected_groups_origin', affected_groups_origin]
    _logger.info(None, *log_args)
    # load use profiles who have account (regardless of activation status)
    profiles = GenericUserProfile.objects.filter(pk__in=profile_ids, auth__isnull=False)
    with transaction.atomic():
        # TODO, may repeat the task after certain time interval if it failed in the middle
        # (until it's successfully completed)
        for prof in profiles:
            GenericUserProfile.update_account_privilege(profile=prof, account=prof.account)
        done = True
    return done


@celery_app.task
@log_wrapper(logger=_logger, loglevel=logging.INFO)
def clean_expired_auth_token(days):
    td = timedelta(days=days)
    t0 = datetime.now()
    t0 = t0 - td
    expired = AuthUserResetRequest.objects.filter(time_created__lt=t0)
    result = expired.values('id', 'profile__pk', 'email__email__addr', 'time_created')
    result = list(result)
    expired.delete()
    return result



