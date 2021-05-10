
from django.db import models
from django.utils import timezone
from django.contrib.contenttypes.models  import ContentType
from django.contrib.contenttypes.fields  import GenericForeignKey, GenericRelation

from softdelete.models import SoftDeleteObjectMixin
from common.models.fields  import CompoundPrimaryKeyField
from common.models.mixins  import MinimumInfoMixin
from common.util.python.django.storage import ExtendedFileSysStorage

from .common import ProductmgtChangeSet, ProductmgtSoftDeleteRecord, _UserProfileMixin, BaseProductIngredient

_fs_board_img   = ExtendedFileSysStorage(
        location='filespace/product/development/board/{id}/img',
        extra_id_required=['id'],
        )
_fs_card_img    = ExtendedFileSysStorage(
        location='filespace/product/development/board/{list__board__id}/card/{id}/img',
        extra_id_required=['list__board__id', 'id'],
        )
_fs_card_attach = ExtendedFileSysStorage(
        location='filespace/product/development/board/{card__list__board__id}/card/{card__id}/other', \
        extra_id_required=['card__list__board__id', 'card__id'],
        )


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
    class Meta:
        db_table = 'product_dev_ingredient'
    min_info_field_names = ['id','name']
    # it can be saleable (finished goods) or not (e.g. raw material, consumable)
    category = models.PositiveSmallIntegerField(choices=ProductDevIngredientType.choices)

    def _delete_relations(self, related_fields, *args, **kwargs):
        kwargs['skip_model_types'] = [self.saleitems_applied.model,]
        super()._delete_relations(related_fields, *args, **kwargs)

# The classes below are used for kanban-style project management

class ProjectOwnerMixin(_UserProfileMixin):
    class Meta:
        abstract = True
    @property
    def owner(self):
        return self.usrprof

    @owner.setter
    def owner(self, newval):
        self.usrprof = newval

class _CreateTimeFieldMixin(models.Model):
    class Meta:
        abstract = True
    created_at = models.DateTimeField(auto_now_add=True)


class ProductDevProject(SoftDeleteObjectMixin, _CreateTimeFieldMixin):
    SOFTDELETE_CHANGESET_MODEL = ProductmgtChangeSet
    SOFTDELETE_RECORD_MODEL = ProductmgtSoftDeleteRecord
    class Meta:
        db_table = 'product_dev_project'
        constraints = [models.UniqueConstraint(fields=['saleable_type','saleable_id'],
            name="unique_proj_saleable",)]

    allowed_models = models.Q(app_label='product', model='ProductSaleableItem') | \
                     models.Q(app_label='product', model='ProductSaleablePackage')
    saleable_type = models.ForeignKey(to=ContentType, on_delete=models.CASCADE, null=False,
                           db_column='saleable_type',  limit_choices_to=allowed_models)
    saleable_id   = models.PositiveIntegerField(db_column='saleable_id')
    saleable_ref  = GenericForeignKey(ct_field='saleable_type', fk_field='saleable_id')
    # TODO, apply composite primary key and handle referential key issue


class ProductDevProjMembership(SoftDeleteObjectMixin, _UserProfileMixin, _CreateTimeFieldMixin):
    SOFTDELETE_CHANGESET_MODEL = ProductmgtChangeSet
    SOFTDELETE_RECORD_MODEL = ProductmgtSoftDeleteRecord
    class Meta:
        #managed = False
        db_table = 'product_dev_proj_membership'

    class Access(models.IntegerChoices):
        MEMBER = 1   # Can view and create and move only own items
        ADMIN = 2    # Can remove members and modify project settings.

    id = CompoundPrimaryKeyField(inc_fields=['project','usrprof'])
    project = models.ForeignKey(to=ProductDevProject, null=False, db_column='project',
            on_delete=models.CASCADE, related_name='membership')
    access_level = models.IntegerField(choices=Access.choices, default=Access.MEMBER)




class ProductDevKanbanBoard(SoftDeleteObjectMixin, ProjectOwnerMixin, _CreateTimeFieldMixin):
    SOFTDELETE_CHANGESET_MODEL = ProductmgtChangeSet
    SOFTDELETE_RECORD_MODEL = ProductmgtSoftDeleteRecord
    class Meta:
        db_table = 'product_dev_kanban_board'

    project = models.ForeignKey(to=ProductDevProject, null=False, db_column='project',
            on_delete=models.CASCADE, related_name='boards')
    title = models.CharField(max_length=127, blank=False, null=False)
    description = models.TextField(blank=True, null=False)

    # Only one of the below will be used from the frontend
    image = models.ImageField(blank=True, storage=_fs_board_img)
    # images stored in external server
    image_url = models.URLField(blank=True, null=False)
    color = models.CharField(blank=True, null=False, max_length=6)  # Hex Code


class ProductDevBoardLabel(SoftDeleteObjectMixin):
    SOFTDELETE_CHANGESET_MODEL = ProductmgtChangeSet
    SOFTDELETE_RECORD_MODEL = ProductmgtSoftDeleteRecord
    class Meta:
        db_table = 'product_dev_board_label'
    board = models.ForeignKey(ProductDevKanbanBoard, db_column='board',
            on_delete=models.CASCADE, related_name='labels')
    title = models.CharField(max_length=127, blank=True, null=False)
    color = models.CharField(max_length=6, blank=False, null=False)


class ProductDevKanbanList(SoftDeleteObjectMixin, _CreateTimeFieldMixin):
    SOFTDELETE_CHANGESET_MODEL = ProductmgtChangeSet
    SOFTDELETE_RECORD_MODEL = ProductmgtSoftDeleteRecord
    class Meta:
        db_table = 'product_dev_kanban_list'

    board = models.ForeignKey(ProductDevKanbanBoard, db_column='board',
            on_delete=models.CASCADE, related_name="lists")
    title = models.CharField(max_length=127, blank=False, null=False)
    order = models.DecimalField(max_digits=30, decimal_places=15 , blank=True, null=True)

    def save(self, *args, **kwargs):
        filtered_objects = self.board.lists.all()
        if not self.order:
            if not filtered_objects.exists():
                self.order = 2 << 16 - 1
            else:
                max_order = filtered_objects.aggregate(models.Max('order'))['order__max']
                self.order = max_order + 2 << 16 - 1
        return super().save(*args, **kwargs)


class ProductDevKanbanCard(SoftDeleteObjectMixin, _CreateTimeFieldMixin):
    SOFTDELETE_CHANGESET_MODEL = ProductmgtChangeSet
    SOFTDELETE_RECORD_MODEL = ProductmgtSoftDeleteRecord
    class Meta:
        db_table = 'product_dev_kanban_card'

    list = models.ForeignKey(ProductDevKanbanList, db_column='list',
            on_delete=models.CASCADE, related_name='cards')
    title = models.CharField(max_length=127, blank=False, null=False)
    description = models.TextField(blank=True, null=False)

    # Only one of the below will be used from the frontend
    image = models.ImageField(blank=True, storage=_fs_card_img)
    image_url = models.URLField(blank=True, null=False)
    color = models.CharField(blank=True, null=False, max_length=6)  # Hex Code

    order = models.DecimalField(max_digits=30,decimal_places=15, blank=True, null=True)
    labels = models.ManyToManyField(ProductDevBoardLabel, blank=True, related_name='tagged_labels')
    due_date = models.DateTimeField(blank=True, null=True)

    def save(self, *args, **kwargs):
        filtered_objects = self.list.cards.all()
        if not self.order:
            if not filtered_objects.exists():
                self.order = 2 << 16 - 1
            else:
                max_order = filtered_objects.aggregate(models.Max('order'))['order__max']
                self.order = max_order + 2 << 16 - 1
        return super().save(*args, **kwargs)


class ProductDevKanbanCardAssignment(SoftDeleteObjectMixin, _UserProfileMixin, _CreateTimeFieldMixin):
    SOFTDELETE_CHANGESET_MODEL = ProductmgtChangeSet
    SOFTDELETE_RECORD_MODEL = ProductmgtSoftDeleteRecord
    class Meta:
        db_table = 'product_dev_kanban_card_assignment'
    card = models.ForeignKey(ProductDevKanbanCard, db_column='card',
            on_delete=models.CASCADE, related_name='assignments')
    id = CompoundPrimaryKeyField(inc_fields=['card','usrprof'])


class ProductDevKanbanCardComment(SoftDeleteObjectMixin, _UserProfileMixin, _CreateTimeFieldMixin):
    SOFTDELETE_CHANGESET_MODEL = ProductmgtChangeSet
    SOFTDELETE_RECORD_MODEL = ProductmgtSoftDeleteRecord
    class Meta:
        db_table = 'product_dev_kanban_card_comment'
    card = models.ForeignKey(ProductDevKanbanCard, db_column='card',
            on_delete=models.CASCADE, related_name='comments')
    body = models.TextField(blank=False, null=False)


class ProductDevKanbanCardAttachment(SoftDeleteObjectMixin, _UserProfileMixin, _CreateTimeFieldMixin):
    SOFTDELETE_CHANGESET_MODEL = ProductmgtChangeSet
    SOFTDELETE_RECORD_MODEL = ProductmgtSoftDeleteRecord
    class Meta:
        db_table = 'product_dev_kanban_card_attachment'
    card = models.ForeignKey(ProductDevKanbanCard, db_column='card',
            on_delete=models.CASCADE, related_name='attachments')
    upload = models.FileField(storage=_fs_card_attach)




# out = 'CREATE TABLE `product_dev_proj_membership` (`id` integer AUTO_INCREMENT NOT NULL PRIMARY KEY, `time_deleted` datetime(6) NULL, `usrprof` integer UNSIGNED NOT NULL CHECK (`usrprof` >= 0), `access_level` integer NOT NULL, `created_at` datetime(6) NOT NULL, `project` integer NOT NULL);'


#### # in case users' business would like to customize products for their clients
#### class ProductCustomOption(models.Model):
####     class Meta:
####         db_table = 'product_custom_option'
####     """
####     users can define hierarchy of each custom option
####     e.g.
####       your product is burger, one of the custom options is "meat type",
####       "meat type" can be lamb, beef, and chicken.
####       the option "beef" can be further "6 oz beef", "8 oz beef" ... etc.
####       The hierarcgy of the custom option will be like:
####           meat_type/beef/6oz
####     """
####     name   = models.CharField(max_length=100, unique=False)
####     active = models.BooleanField(default=False)
####     # path of metadata file that describes essential attributes of every custom option
####     meta_path = models.CharField(max_length=200, unique=False)
####     parent = models.ForeignKey('self', db_column='parent', on_delete=models.CASCADE)
#### 
#### 
#### # some custom options may consist of a set of product items, which are used as materials,
#### # these items can be saleable or not.
#### class ProductCustomOptionComposition(models.Model):
####     class Meta:
####         db_table = 'product_custom_option_composition'
####     custom_option = models.ForeignKey(ProductCustomOption, db_column='custom_option', on_delete=models.CASCADE)
####     # the "item" at here can be
####     #     (1) raw material
####     #     (2) work in progress
####     #     (3) consumables
####     #     (4) finished goods that can also be part of a defined product custom option.
####     ingredient = models.ForeignKey(ProductItem, db_column='ingredient',  on_delete=models.CASCADE)
####     qty_required = models.PositiveIntegerField()
#### 
#### 
#### class ProductItemCustomOption(models.Model):
####     class Meta:
####         db_table = 'product_item_custom_option'
####     """
####     This model shows the limit about which custome option can be applied to which product item.
####     it's like restricted many-to-many relation,
####     One product item can include certain (or none of) defined custom options,
####     one custom option could also be applied to several different product items
####     """
####     # the product item at here must be saleable
####     saleable_item = models.ForeignKey(ProductItem, db_column='saleable_item', on_delete=models.CASCADE, unique=False,
####                             limit_choices_to=models.Q(category=ProductItemCategory.FINISHED_GOODS))
####     custom_option = models.ForeignKey(ProductCustomOption, db_column='custom_option',  on_delete=models.CASCADE, unique=False)
#### 
#### 
#### 
#### class PackageSaleableItem(models.Model):
####     class Meta:
####         db_table = 'package_saleable_item'
####     """ users may require certain number of identical saleable items included in a package,
####         therefore many-to-many field cannot be used at here to model the relation between
####         Package and ProductItem.  """
####     pkg = models.ForeignKey(Package , db_column='pkg',  on_delete=models.CASCADE, unique=False)
####     # each product included in a package must be saleable
####     item = models.ForeignKey(ProductItem, db_column='item', on_delete=models.CASCADE,
####                limit_choices_to=models.Q(category=ProductItemCategory.FINISHED_GOODS))
####     qty_required = models.PositiveIntegerField()


