
from django.db import models

from softdelete.models import SoftDeleteObjectMixin
from common.models.fields  import CompoundPrimaryKeyField
from common.models.mixins  import MinimumInfoMixin

from .common import ProductmgtChangeSet, ProductmgtSoftDeleteRecord, _UserProfileMixin, BaseProductIngredient, _atomicity_fn, _MatCodeOptions


class ProductDevIngredientType(models.IntegerChoices):
    RAW_MATERIAL  = 1,
    WORK_IN_PROGRESS = 2,
    FINISHED_GOODS = 3,
    CONSUMABLES    = 4, # e.g. fuel, gas for restaurant
    EQUIPMENTS     = 5, # e.g. pot, stove, oven for restaurant


class ProductDevIngredient(BaseProductIngredient, MinimumInfoMixin):
    """
    ingredients used fpr product development/manufacture, not directly saleable,
    and only visible at staff site
    """
    quota_material = _MatCodeOptions.MAX_NUM_INGREDIENTS
    class Meta:
        db_table = 'product_dev_ingredient'
    min_info_field_names = ['id','name']
    # it can be saleable (finished goods) or not (e.g. raw material, consumable)
    category = models.PositiveSmallIntegerField(choices=ProductDevIngredientType.choices)

    def _delete_relations(self, related_fields, *args, **kwargs):
        kwargs['skip_model_types'] = [self.saleitems_applied.model,]
        super()._delete_relations(related_fields, *args, **kwargs)

    @_atomicity_fn()
    def delete(self, *args, **kwargs):
        hard_delete = kwargs.get('hard', False)
        if not hard_delete:# let nested fields add in the same soft-deleted changeset
            if kwargs.get('changeset', None) is None:
                profile_id = kwargs['profile_id'] # kwargs.get('profile_id')
                kwargs['changeset'] = self.determine_change_set(profile_id=profile_id)
        deleted = super().delete(*args, **kwargs)
        if not hard_delete:
            self.saleitems_applied.all().delete(*args, **kwargs)
            kwargs.pop('changeset', None)
        return deleted


