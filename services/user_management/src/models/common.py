from functools import partial
from django.db import models, transaction

from ecommerce_common.models.enums.django import AppCodeOptions
from ecommerce_common.util.django.setup import test_enable as django_test_enable
from softdelete.models import ChangeSet, SoftDeleteRecord

DB_ALIAS_APPLIED = "default" if django_test_enable else "usermgt_service"
# note that atomicity fails siliently with incorrect database credential
# that is why I use partial() to tie `using` argument with transaction.atomic(**kwargs)
_atomicity_fn = partial(transaction.atomic, using=DB_ALIAS_APPLIED)


class UsermgtChangeSet(ChangeSet):
    class Meta:
        db_table = "usermgt_soft_delete_changeset"


class UsermgtSoftDeleteRecord(SoftDeleteRecord):
    class Meta:
        db_table = "usermgt_soft_delete_record"

    changeset = UsermgtChangeSet.foreignkey_fieldtype()
