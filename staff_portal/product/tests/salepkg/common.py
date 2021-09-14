
from product.models.base import _ProductAttrValueDataType
from product.tests.common import _fixtures


_attr_vals_fixture_map = {
    _ProductAttrValueDataType.STRING.value[0][0]: _fixtures['ProductAttributeValueStr'],
    _ProductAttrValueDataType.POSITIVE_INTEGER.value[0][0]: _fixtures['ProductAttributeValuePosInt'],
    _ProductAttrValueDataType.INTEGER.value[0][0]: _fixtures['ProductAttributeValueInt'],
    _ProductAttrValueDataType.FLOAT.value[0][0]: _fixtures['ProductAttributeValueFloat'],
}

