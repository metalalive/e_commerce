from functools import partial

from django.db import models, IntegrityError, transaction
from django.contrib.contenttypes.fields  import GenericRelation

from softdelete.models import ChangeSet, SoftDeleteRecord, SoftDeleteQuerySet,  SoftDeleteManager,  SoftDeleteObjectMixin
from common.util.python.django.setup import test_enable as django_test_enable


DB_ALIAS_APPLIED = 'default' if django_test_enable else 'product_dev_service'
_atomicity_fn = partial(transaction.atomic, using=DB_ALIAS_APPLIED)

class ProductmgtChangeSet(ChangeSet):
    class Meta:
        db_table = 'productmgt_soft_delete_changeset'

class ProductmgtSoftDeleteRecord(SoftDeleteRecord):
    class Meta:
        db_table = 'productmgt_soft_delete_record'
    changeset = ProductmgtChangeSet.foreignkey_fieldtype()


class _BaseIngredientQuerySet(SoftDeleteQuerySet):
    @_atomicity_fn()
    def delete(self, *args, **kwargs):
        deleted = super().delete(*args, **kwargs)
        return deleted

    @_atomicity_fn()
    def undelete(self, *args, **kwargs):
        result = super().undelete(*args, **kwargs)
        return result


class _BaseIngredientManager(SoftDeleteManager):
    # subclasses of soft-delete manager can override its queryset class
    default_qset_cls = _BaseIngredientQuerySet


class BaseProductIngredient(SoftDeleteObjectMixin):
    """
    subclasses can extend from this class for saleable product/package item
    , or non-saleable ingredient for product development
    """
    SOFTDELETE_CHANGESET_MODEL = ProductmgtChangeSet
    SOFTDELETE_RECORD_MODEL = ProductmgtSoftDeleteRecord
    objects = _BaseIngredientManager()

    class Meta:
        abstract = True
    name   = models.CharField(max_length=128, unique=False, null=False)
    # active item that can be viewed / edited (only) at staff site
    ##active   = models.BooleanField(default=False) # TODO, remove the field
    # relation fields to attribute types and values of different data types
    attr_val_str     = GenericRelation('ProductAttributeValueStr',    object_id_field='ingredient_id', content_type_field='ingredient_type')
    attr_val_pos_int = GenericRelation('ProductAttributeValuePosInt', object_id_field='ingredient_id', content_type_field='ingredient_type')
    attr_val_int     = GenericRelation('ProductAttributeValueInt',   object_id_field='ingredient_id', content_type_field='ingredient_type')
    attr_val_float   = GenericRelation('ProductAttributeValueFloat', object_id_field='ingredient_id', content_type_field='ingredient_type')

    @_atomicity_fn()
    def delete(self, *args, **kwargs):
        new_changeset = False
        hard_delete = kwargs.get('hard', False)
        if not hard_delete:# let nested fields add in the same soft-deleted changeset
            if kwargs.get('changeset', None) is None:
                profile_id = kwargs['profile_id'] # kwargs.get('profile_id')
                kwargs['changeset'] = self.determine_change_set(profile_id=profile_id)
                new_changeset = True
        deleted = super().delete(*args, **kwargs)
        if not hard_delete:
            self.attr_val_str.all().delete(*args, **kwargs)
            self.attr_val_pos_int.all().delete(*args, **kwargs)
            self.attr_val_int.all().delete(*args, **kwargs)
            self.attr_val_float.all().delete(*args, **kwargs)
            ##attr_del_fn = lambda dtype_item: getattr(self, dtype_item[0][1]).all().delete(*args, **kwargs)
            ##list(map(attr_del_fn, _ProductAttrValueDataType))
            if new_changeset:
                kwargs.pop('changeset', None)
        return deleted

    @_atomicity_fn()
    def undelete(self, *args, **kwargs):
        result = super().undelete(*args, **kwargs)
        return result
#### end of class BaseProductIngredient


class _UserProfileMixin(models.Model):
    class Meta:
        abstract = True
    # profile is linked to profile ID of each active user in user management service
    usrprof = models.PositiveIntegerField(unique=False, db_column='usrprof',)



