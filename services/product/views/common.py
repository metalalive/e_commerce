
from django.db.models.constants import LOOKUP_SEP
from common.views.filters import  AdvancedSearchFilter
from ..models.base        import _ProductAttrValueDataType
from ..serializers.common import attribute_field_label

class BaseIngredientSearchFilter(AdvancedSearchFilter):
    def _create_leaf_node(self, _operator, operands, metadata=None):
        """
        overwrite this function for extra search field mapping:
            * attribute type
            * attribute value
        This function expects to check data type of the attribute type
        by looking for `dtype` field of the given `metadata` .
        For example , a client sends advanced search condition within the request:
            {
                "operator": "and",
                "operands":[
                    {
                        "operator":"==",
                        "operands":["attributes__type", 7],
                        "metadata": {"dtype": 1}
                    } ,
                    {
                        "operator":"contains",
                        "operands":["attributes__value", "wh"],
                        "metadata": {"dtype": 1}
                    }
                ]
            }

        `dtype` is equal to 1, which means the attribute value is string value,
        then this filter converts certain field names to :
            {
                "operator": "and",
                "operands":[
                    {
                        "operator":"==",
                        "operands":["attr_val_str__attr_type", 7],
                        "metadata": {"dtype": 1}
                    } ,
                    {
                        "operator":"contains",
                        "operands":["attr_val_str__value", "wh"],
                        "metadata": {"dtype": 1}
                    }
                ]
            }

        """
        if operands and operands[0] in self._map_attr_label.keys():
            err_msg = "unknown data type of the attribute type with operands %s"
            err_msg = err_msg % (operands)
            assert metadata and metadata.get('dtype') , err_msg
            err_msg = "incorrect data type `%s` of the attribute type with operands %s"
            err_msg = err_msg % (metadata['dtype'], operands)
            filtered_dtype = tuple(filter(lambda x: x[0] == metadata['dtype'], self._valid_attr_dtypes))
            assert len(filtered_dtype) == 1, err_msg
            full_related_name = [filtered_dtype[0][1], self._map_attr_label[operands[0]]]
            operands[0] = LOOKUP_SEP.join(full_related_name)
        return super()._create_leaf_node(_operator=_operator, operands=operands, metadata=metadata)

    def filter_queryset(self, request, queryset, view):
        frontend_attr_type_label = LOOKUP_SEP.join([attribute_field_label['_list'] , attribute_field_label['type']])
        frontend_attr_val_label  = LOOKUP_SEP.join([attribute_field_label['_list'] , attribute_field_label['value']])
        self._map_attr_label = {
            frontend_attr_type_label : 'attr_type',
            frontend_attr_val_label  : 'value',
        }
        self._valid_attr_dtypes   = tuple(map(lambda x: getattr(_ProductAttrValueDataType, x)[0] , _ProductAttrValueDataType.names))
        #self._resource_model = view.serializer_class.Meta.model
        return super().filter_queryset(request=request, queryset=queryset, view=view)


# {"operator": "and", "operands":[{"operator":"==", "operands":["attributes__type", 7], "metadata": {"dtype": 1}}, {"operator":"contains", "operands":["attributes__value", "w"], "metadata": {"dtype": 1} }]}


