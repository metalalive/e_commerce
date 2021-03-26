from django.db import models
from django.db.models.fields.related_descriptors import ForwardManyToOneDescriptor, ManyToManyDescriptor
from django.contrib.contenttypes.models  import ContentType
from django.contrib.contenttypes.fields  import GenericForeignKey, GenericRelation

from common.models.enums   import UnitOfMeasurement
from common.models.mixins  import MinimumInfoMixin
from common.models.fields  import CompoundPrimaryKeyField
from common.models.closure_table import ClosureTableModelMixin, get_paths_through_processing_node, filter_closure_nodes_recovery
from common.util.python.django.storage import ExtendedFileSysStorage
from softdelete.models import  SoftDeleteObjectMixin

from .common import ProductmgtChangeSet, ProductmgtSoftDeleteRecord, BaseProductIngredient, _UserProfileMixin
# The term "product" here means :
# * items for sale (saleable)
# * items bought from suppliers, and then used as material of your product (non-saleable)
#
_fs_item = ExtendedFileSysStorage(
        location='filespace/product/saleable/item/{sale_item__id}',
        extra_id_required=['sale_item__id']
        )
_fs_pkg  = ExtendedFileSysStorage(
        location='filespace/product/saleable/pkg/{sale_pkg__id}',
        extra_id_required=['sale_pkg__id']
        )


class UniqueIdentifierMixin(models.Model):
    """
    the mixin provides 4-byte integer as primary key, key generating function,
    collision handling function on insertion/update to guarantee uniqueness.
    * The id field in this mixin does NOT use auto-increment feature supported by
      supported low-level databases.
    * Also the id does NOT use any UUID-like generator because the probability of
      collision is not exactly zero even you use something like UUID4,
      and you still need to handle collision when the database table grows
      to very large extent. (not to mention UUID is 16 bytes requires 4x space
      than a typical 4-byte integer-based B-tree index)
    """
    class Meta:
        abstract = True
    id = models.PositiveIntegerField(primary_key=True, unique=True, db_index=True, db_column='id',)


class ProductTag(_UserProfileMixin, MinimumInfoMixin):
    """
    hierarchical tags for product categories
    """
    class Meta:
        db_table = 'product_tag'
    name   = models.CharField(max_length=64, unique=False)


class ProductTagClosure(ClosureTableModelMixin):
    """ closure table to represent tree structure of tag hierarchy """
    class Meta(ClosureTableModelMixin.Meta):
        db_table = 'product_tag_closure'
    ancestor   = ClosureTableModelMixin.asc_field(ref_cls=ProductTag)
    descendant = ClosureTableModelMixin.desc_field(ref_cls=ProductTag)


class _RelatedFieldMixin:
    """
    allow to change related_name for related field (e.g. foreign key, m2m key)
    at runtime
    """
    @classmethod
    def set_related_name(cls, field_name, value):
        valid_list = (ForwardManyToOneDescriptor, ManyToManyDescriptor,)
        related_descriptor = getattr(cls, field_name, None)
        assert related_descriptor and isinstance(related_descriptor, valid_list), \
                'related_descriptor inproper type : %s' % related_descriptor
        related_descriptor.field.remote_field.related_name = value


class AbstractProduct(BaseProductIngredient, UniqueIdentifierMixin, _UserProfileMixin, _RelatedFieldMixin):
    """
    Define abstract product model, it records generic product information that is required.
    this abstract class (model) can be inherited if user wants to :
    * sell product item individually
    * or define packages that sell several product items togather at once (Package)
    """
    class Meta:
        abstract = True
    # one product item could have many tags, a tag includes several product items
    tags = models.ManyToManyField(ProductTag, db_column='tags', related_name='tagged_abs_products')
    # global visible flag at front store (will be used at PoS service)
    visible  = models.BooleanField(default=False)
    # current price of this product or package item. this field value also means the base price
    # without discount and extra charge due to customization
    price = models.FloatField(default=0.00)
    # TODO, does this project require secondary index on `profile` column ?


class ProductSaleableItem(AbstractProduct):
    class Meta(AbstractProduct.Meta):
        db_table = 'product_saleable_item'

class ProductSaleablePackage(AbstractProduct):
    class Meta(AbstractProduct.Meta):
        db_table = 'product_saleable_package'

ProductSaleableItem.set_related_name(field_name='tags', value='tagged_products')
ProductSaleablePackage.set_related_name(field_name='tags', value='tagged_packages')

class ProductSaleableItemComposite(SoftDeleteObjectMixin):
    """
    This model is used in case user's business details development or manufacturing
    of all the saleable items with all required ingredients.
    Each instance of this model indicates one ingredient type applied to developing
    a saleable item with number of (or amount of, if uncountable) the ingredients to use.
    """
    # TODO, add another model for recording instructions of saleable item manufacturing
    SOFTDELETE_CHANGESET_MODEL = ProductmgtChangeSet
    SOFTDELETE_RECORD_MODEL = ProductmgtSoftDeleteRecord
    class Meta:
        db_table = 'product_saleable_item_composite'
    quantity = models.FloatField(blank=False, null=False)
    unit = models.SmallIntegerField(blank=False, null=False, choices=UnitOfMeasurement.choices)
    ingredient = models.ForeignKey('ProductDevIngredient', on_delete=models.CASCADE,
                  db_column='ingredient', related_name='saleitems_applied')
    sale_item = models.ForeignKey('ProductSaleableItem', on_delete=models.CASCADE,
                  db_column='sale_item', related_name='ingredients_applied')
    id = CompoundPrimaryKeyField(inc_fields=['sale_item','ingredient'])


class ProductSaleablePackageComposite(SoftDeleteObjectMixin):
    SOFTDELETE_CHANGESET_MODEL = ProductmgtChangeSet
    SOFTDELETE_RECORD_MODEL = ProductmgtSoftDeleteRecord
    class Meta:
        db_table = 'product_saleable_package_composite'
    quantity = models.FloatField(blank=False, null=False)
    unit = models.SmallIntegerField(blank=False, null=False, choices=UnitOfMeasurement.choices)
    sale_item = models.ForeignKey('ProductSaleableItem', on_delete=models.CASCADE,
                  db_column='sale_item', related_name='pkgs_applied')
    package = models.ForeignKey('ProductSaleablePackage', on_delete=models.CASCADE,
                  db_column='package', related_name='saleitems_applied')
    id = CompoundPrimaryKeyField(inc_fields=['sale_item','package'])


class ProductSaleableItemMedia(SoftDeleteObjectMixin):
    """
    media file paths e.g. image / audio / video for saleable item(s)
    """
    SOFTDELETE_CHANGESET_MODEL = ProductmgtChangeSet
    SOFTDELETE_RECORD_MODEL = ProductmgtSoftDeleteRecord
    class Meta:
        db_table = 'product_saleable_item_media'
    sale_item = models.ForeignKey('ProductSaleableItem', on_delete=models.CASCADE,
                  db_column='sale_item', related_name='media_set')
    # the media could be image / audio / video file, or any other to represent
    # your saleable item, application developers can restrict number of media files
    # uploaded for each saleable item, by limiting number of instances of this
    # model (number of records at DB table level)
    media = models.FileField(storage=_fs_item)


class ProductSaleablePackageMedia(SoftDeleteObjectMixin):
    SOFTDELETE_CHANGESET_MODEL = ProductmgtChangeSet
    SOFTDELETE_RECORD_MODEL = ProductmgtSoftDeleteRecord
    class Meta:
        db_table = 'product_saleable_package_media'
    sale_pkg = models.ForeignKey('ProductSaleablePackage', on_delete=models.CASCADE,
                  db_column='sale_pkg', related_name='media_set')
    media = models.FileField(storage=_fs_pkg)


class _ProductAttrValueDataType(models.TextChoices):
    """
    data type options for textual/numeric attributes of saleable item(s)
    """
    STRING  = 'attr_val_str',
    INTEGER = 'attr_val_int',
    POSITIVE_INTEGER = 'attr_val_pos_int',
    FLOAT = 'attr_val_float',


class ProductAttributeType(SoftDeleteObjectMixin):
    """
    this is EAV pattern, a schemaless design,
    in order to avoid mistakes that might be made by users :
    * this attribute table (and the value table below) should be edited carefully
      only by authorized staff (better not letting customers add whatever attribute
      they want), in order to avoid duplicate attribute types with the same semantic.
    * each attribute may require different data type as the attribute value, so
      I create abstract model class `BaseProductAttributeValue` for all attribute
      values tied to existing attribute type, and existing product item, then subclass it
      for different data types.
    * each attribute type added in this table are optional to all product items, product
      owners or managers are free to determine what attribute types they'll apply to
      each of their product.
    * To mitigate performance degradation when performing query with complex condition
      in such schemaless design, application frontend can preload all required attribute
      types in advance, let end users at frontend select attribute types, then use id
      of each attribute type on sending search request (instead of using name of each
      attribute type). By doing so the database (at backend) does not have to join this
      table when searching for attributes and their values
    """
    SOFTDELETE_CHANGESET_MODEL = ProductmgtChangeSet
    SOFTDELETE_RECORD_MODEL = ProductmgtSoftDeleteRecord

    class Meta:
        db_table = 'product_attribute_type'

    # help text to describe how this attribute is used to a product
    name = models.CharField(max_length=64, unique=False, blank=False)
    # the value can also be the name of the reverse field in this model
    value_dtype = models.CharField(max_length=20, choices=_ProductAttrValueDataType.choices)


class BaseProductAttributeValue(SoftDeleteObjectMixin, _RelatedFieldMixin):
    """
    user-defined metadata for storing textual/numeric attributes applied to saleable items.

    Here are examples of how attribiute key/value pairs may look like :
    * paths of (different qualities of) image / audio / video sample file, for showcasing this product
    * weight, physical size e.g. width x depth x height per unit
    * there should be unit of measure for both countable items (e.g. number of cans, bottles)
      and uncountable items (e.g. litre, gallon, square-meter)
    * the metadata format can be XML / JSON / CSV, or user-defined data structure
    """
    SOFTDELETE_CHANGESET_MODEL = ProductmgtChangeSet
    SOFTDELETE_RECORD_MODEL = ProductmgtSoftDeleteRecord
    class Meta:
        abstract = True
    allowed_models = models.Q(app_label='product', model='ProductSaleableItem') | \
                     models.Q(app_label='product', model='ProductSaleablePackage') | \
                     models.Q(app_label='product', model='ProductDevIngredient')
    ingredient_type = models.ForeignKey(to=ContentType, on_delete=models.CASCADE, null=False,
                                  db_column='ingredient_type',  limit_choices_to=allowed_models)
    ingredient_id   = models.PositiveIntegerField(db_column='ingredient_id')
    ingredient_ref  = GenericForeignKey(ct_field='ingredient_type', fk_field='ingredient_id')
    attr_type = models.ForeignKey(ProductAttributeType, db_column='attr_type', related_name='attr_val',
                on_delete=models.CASCADE)


class ProductAttributeValueStr(BaseProductAttributeValue):
    class Meta:
        db_table = 'product_attribute_value_str'
    value  = models.CharField(max_length=64, unique=False)

class ProductAttributeValuePosInt(BaseProductAttributeValue):
    class Meta:
        db_table = 'product_attribute_value_pos_int'
    value  = models.PositiveIntegerField()

class ProductAttributeValueInt(BaseProductAttributeValue):
    class Meta:
        db_table = 'product_attribute_value_int'
    value  = models.IntegerField()

class ProductAttributeValueFloat(BaseProductAttributeValue):
    class Meta:
        db_table = 'product_attribute_value_float'
    value  = models.FloatField()


ProductAttributeValueStr.set_related_name(field_name='attr_type', value=_ProductAttrValueDataType.STRING )
ProductAttributeValueInt.set_related_name(field_name='attr_type', value=_ProductAttrValueDataType.INTEGER)
ProductAttributeValuePosInt.set_related_name(field_name='attr_type', value=_ProductAttrValueDataType.POSITIVE_INTEGER)
ProductAttributeValueFloat.set_related_name(field_name='attr_type', value=_ProductAttrValueDataType.FLOAT)


class ProductAppliedAttributePrice(SoftDeleteObjectMixin):
    """
    describe pricing method, extra amount of money will be charged if customer
    orders a product customized with certain attribute values in the table above
    """
    SOFTDELETE_CHANGESET_MODEL = ProductmgtChangeSet
    SOFTDELETE_RECORD_MODEL = ProductmgtSoftDeleteRecord
    class Meta:
        db_table = 'product_applied_attribute_price'

    allowed_models = models.Q(app_label='product', model='ProductAttributeValueStr') | \
                     models.Q(app_label='product', model='ProductAttributeValuePosInt') | \
                     models.Q(app_label='product', model='ProductAttributeValueInt')
    attrval_type = models.ForeignKey(to=ContentType, on_delete=models.CASCADE, null=False,
                                  db_column='attrval_type',  limit_choices_to=allowed_models)
    attrval_id   = models.PositiveIntegerField(db_column='attrval_id')
    attrval_ref  = GenericForeignKey(ct_field='attrval_type', fk_field='attrval_id')
    amount = models.FloatField(default=0.00)




# class ProductPriceHistory(models.Model):
#     class Meta:
#         db_table = 'product_price_history'
#     # the product item at here must be saleable
#     saleable_item  = models.ForeignKey(ProductItem, db_column='saleable_item', on_delete=models.CASCADE,
#                             limit_choices_to=models.Q(category=ProductItemCategory.FINISHED_GOODS))
#     price = models.FloatField(default=0.00)
#     applied_until = models.DateTimeField()

