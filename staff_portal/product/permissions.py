import copy

from rest_framework.permissions import BasePermission as DRFBasePermission
from rest_framework.filters import BaseFilterBackend as DRFBaseFilterBackend

from common.models.constants import ROLE_ID_SUPERUSER

class AppBasePermission(DRFBasePermission, DRFBaseFilterBackend):
    perms_map = {
        'GET': [],
        'OPTIONS': [],
        'HEAD': [],
        'POST':   [],
        'PUT':    [],
        'PATCH':  [],
        'DELETE': [],
    }
    superuser_id = ROLE_ID_SUPERUSER

    def has_permission(self, request, view):
        result = False
        perms_required = self.perms_map.get(request.method.upper(), None)
        if perms_required is None:
            pass
        elif any(perms_required):
            perms_required = set(perms_required)
            roles = view.get_profile_roles(request=request, num_of_msgs_fetch=2)
            for role in roles:
                if role['id'] == self.superuser_id:
                    result = True
                    break
                perms_required = perms_required - set(role['perm_code'])
        if not any(perms_required):
            result = True
        #import pdb
        #pdb.set_trace()
        return result

    def filter_queryset(self, request, queryset, view):
        return queryset


class TagsPermissions(AppBasePermission):
    perms_map = {
        'GET': ['product.view_producttag', 'product.view_producttagclosure'],
        'OPTIONS': [],
        'HEAD': [],
        'POST':   ['product.add_producttag', 'product.add_producttagclosure'],
        'PUT':    ['product.change_producttag', 'product.change_producttagclosure'],
        'PATCH':  ['product.change_producttag', 'product.change_producttagclosure'],
        'DELETE': ['product.delete_producttag', 'product.delete_producttagclosure'],
    }

class AttributeTypePermissions(AppBasePermission):
    perms_map = {
        'GET': ['product.view_productattributetype'],
        'OPTIONS': [],
        'HEAD': [],
        'POST':   ['product.add_productattributetype'],
        'PUT':    ['product.change_productattributetype'],
        'PATCH':  ['product.change_productattributetype'],
        'DELETE': ['product.delete_productattributetype'],
    }

class BaseIngredientPermissions(AppBasePermission):
    perms_map = {
        'GET': [
            'product.view_productattributetype',
            'product.view_productattributevaluestr',
            'product.view_productattributevalueposint',
            'product.view_productattributevalueint',
            'product.view_productattributevaluefloat',
        ],
        'OPTIONS': [],
        'HEAD': [],
        'POST':   [
            'product.view_productattributetype',
            'product.add_productattributevaluestr',   'product.view_productattributevaluestr',
            'product.add_productattributevalueposint','product.view_productattributevalueposint',
            'product.add_productattributevalueint',   'product.view_productattributevalueint',
            'product.add_productattributevaluefloat', 'product.view_productattributevaluefloat',
        ],
        'PUT':    [
            'product.view_productattributetype',
            'product.add_productattributevaluestr',   'product.change_productattributevaluestr',
            'product.delete_productattributevaluestr','product.view_productattributevaluestr',
            'product.add_productattributevalueposint','product.change_productattributevalueposint',
            'product.delete_productattributevalueposint','product.view_productattributevalueposint',
            'product.add_productattributevalueint',    'product.change_productattributevalueint',
            'product.delete_productattributevalueint', 'product.view_productattributevalueint',
            'product.add_productattributevaluefloat',  'product.change_productattributevaluefloat',
            'product.delete_productattributevaluefloat', 'product.view_productattributevaluefloat',
        ],
        'PATCH':  [
            'product.change_productattributevaluestr',
            'product.change_productattributevalueposint',
            'product.change_productattributevalueint',
            'product.change_productattributevaluefloat',
        ],
        'DELETE': [
            'product.delete_productattributevaluestr',
            'product.delete_productattributevalueposint',
            'product.delete_productattributevalueint',
            'product.delete_productattributevaluefloat',
        ],
    }

class FabricationIngredientPermissions(BaseIngredientPermissions):
    perms_map = {
        'GET': copy.copy(BaseIngredientPermissions.perms_map['GET']) + ['product.view_productdevingredient',],
        'OPTIONS': [],
        'HEAD': [],
        'POST':   copy.copy(BaseIngredientPermissions.perms_map['GET']) + ['product.add_productdevingredient',   ],
        'PUT':    copy.copy(BaseIngredientPermissions.perms_map['PUT']) + ['product.change_productdevingredient',],
        'PATCH':  copy.copy(BaseIngredientPermissions.perms_map['PATCH'])  + ['product.change_productdevingredient',],
        'DELETE': copy.copy(BaseIngredientPermissions.perms_map['DELETE']) + ['product.delete_productdevingredient',],
    }



