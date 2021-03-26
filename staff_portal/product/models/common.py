
from django.db import models
from softdelete.models import ChangeSet, SoftDeleteRecord, SoftDeleteObjectMixin

class ProductmgtChangeSet(ChangeSet):
    class Meta:
        db_table = 'productmgt_soft_delete_changeset'

class ProductmgtSoftDeleteRecord(SoftDeleteRecord):
    class Meta:
        db_table = 'productmgt_soft_delete_record'
    changeset = ProductmgtChangeSet.foreignkey_fieldtype()


class BaseProductIngredient(SoftDeleteObjectMixin):
    """
    subclasses can extend from this class for saleable product/package item
    , or non-saleable ingredient for product development
    """
    SOFTDELETE_CHANGESET_MODEL = ProductmgtChangeSet
    SOFTDELETE_RECORD_MODEL = ProductmgtSoftDeleteRecord

    class Meta:
        abstract = True
    name   = models.CharField(max_length=128, unique=False)
    # active item that can be viewed / edited (only) at staff site
    active   = models.BooleanField(default=False)


class _UserProfileMixin(models.Model):
    class Meta:
        abstract = True
    # profile is linked to profile ID of each active user in user management service
    usrprof = models.PositiveIntegerField(unique=False, db_column='usrprof',)


