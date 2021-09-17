import copy

from django.db.models.constants import LOOKUP_SEP
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
        return result

    def filter_queryset(self, request, queryset, view):
        return queryset
## end of class AppBasePermission


class InputDataOwnerPermissionMixin:
    # currently the function only supports single-column key ID
    def _http_put_delete_permission(self, request, view, id_label='id'):
        result = False
        fn = lambda x: x.get(id_label, None)
        edit_ids = filter(fn, request.data)
        edit_ids = set(map(fn, edit_ids))
        if any(edit_ids):
            profile_id = view.get_profile_id(request=request)
            try:
                lookup_kwargs = {LOOKUP_SEP.join([id_label,'in']):edit_ids, 'usrprof':profile_id}
                qset = view.queryset.filter(**lookup_kwargs)
                if qset.count() == len(edit_ids):
                    result = True
            except ValueError as e: # TODO, log the error 
                # comes from invalid data type of ID, skip it for now,
                # later the serializer will check the ID again then
                # raise validation error and respond with 400 bad request
                pass
        return result

    def _input_data_owner_check(self, request, view):
        # TODO, consider the data which can be read only by small portion of clients
        if request.method.lower() in ('get', 'post', 'patch',):
            result = True
        elif request.method.lower() in ('put', 'delete',):
            result = self._http_put_delete_permission(request, view)
        else:
            result = False
        return result


class TagsPermissions(AppBasePermission):
    perms_map = {
        'GET': ['product.view_producttag', 'product.view_producttagclosure'],
        'OPTIONS': [],
        'HEAD': [],
        'POST':   ['product.add_producttag', 'product.add_producttagclosure'],
        'PUT':    [
            'product.add_producttag', 'product.change_producttag','product.delete_producttag',
            'product.add_producttagclosure', 'product.change_producttagclosure',
            'product.delete_producttagclosure'
            ],
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
            'product.change_productattributevaluestr',
            'product.change_productattributevalueposint',
            'product.change_productattributevalueint',
            'product.change_productattributevaluefloat',
        ], # users should have access to change delete status, since its soft-delete
    }

class FabricationIngredientPermissions(BaseIngredientPermissions):
    perms_map = {
        'GET': copy.copy(BaseIngredientPermissions.perms_map['GET']) + ['product.view_productdevingredient',],
        'OPTIONS': [],
        'HEAD': [],
        'POST':   copy.copy(BaseIngredientPermissions.perms_map['POST']) + ['product.add_productdevingredient',   ],
        'PUT':    copy.copy(BaseIngredientPermissions.perms_map['PUT'])  + ['product.change_productdevingredient',],
        'PATCH':  copy.copy(BaseIngredientPermissions.perms_map['PATCH'])  + ['product.change_productdevingredient',],
        'DELETE': copy.copy(BaseIngredientPermissions.perms_map['DELETE']) + ['product.change_productdevingredient',],
    }

class SaleableItemPermissions(BaseIngredientPermissions, InputDataOwnerPermissionMixin):
    perms_map = {
        'GET': copy.copy(BaseIngredientPermissions.perms_map['GET']) + [
            'product.view_productdevingredient',
            'product.view_productsaleableitem',
            'product.view_productsaleableitemcomposite',
            'product.view_productsaleableitemmedia',
            'product.view_productappliedattributeprice',
        ],
        'OPTIONS': [],
        'HEAD': [],
        'POST':   copy.copy(BaseIngredientPermissions.perms_map['POST']) + [
            'product.view_productdevingredient',
            'product.add_productsaleableitem',
            'product.add_productsaleableitemcomposite', 'product.view_productsaleableitemcomposite',
            'product.add_productsaleableitemmedia',     'product.view_productsaleableitemmedia',
            'product.add_productappliedattributeprice', 'product.view_productappliedattributeprice',
        ],
        'PUT':    copy.copy(BaseIngredientPermissions.perms_map['PUT'])  + [
            'product.view_productdevingredient',
            'product.change_productsaleableitem',
            'product.add_productsaleableitemcomposite',    'product.view_productsaleableitemcomposite',
            'product.change_productsaleableitemcomposite', 'product.delete_productsaleableitemcomposite',
            'product.add_productsaleableitemmedia',      'product.view_productsaleableitemmedia',
            'product.change_productsaleableitemmedia',   'product.delete_productsaleableitemmedia',
            'product.add_productappliedattributeprice',    'product.view_productappliedattributeprice',
            'product.change_productappliedattributeprice', 'product.delete_productappliedattributeprice',
        ],
        'PATCH':  copy.copy(BaseIngredientPermissions.perms_map['PATCH'])  + [
            'product.view_productdevingredient',
            'product.change_productsaleableitem',
            'product.change_productsaleableitemcomposite',
            'product.change_productsaleableitemmedia',
            'product.change_productappliedattributeprice',
        ],
        'DELETE': copy.copy(BaseIngredientPermissions.perms_map['DELETE']) + [
            'product.change_productsaleableitem',
            'product.change_productsaleableitemcomposite',
            'product.change_productsaleableitemmedia',
            'product.change_productappliedattributeprice',
        ], # users should have access to change delete status, since its soft-delete
    }

    def has_permission(self, request, view):
        result = super().has_permission(request=request, view=view)
        if result is True:
            result = self._input_data_owner_check(request, view)
        return result
## end of class SaleableItemPermissions



class SaleablePackagePermissions(BaseIngredientPermissions, InputDataOwnerPermissionMixin):
    perms_map = {
        'GET': copy.copy(BaseIngredientPermissions.perms_map['GET']) + [
            'product.view_productsaleableitem',
            'product.view_productsaleablepackage',
            'product.view_productsaleablepackagecomposite',
            'product.view_productsaleablepackagemedia',
            'product.view_productappliedattributeprice',
        ],
        'OPTIONS': [],
        'HEAD': [],
        'POST':   copy.copy(BaseIngredientPermissions.perms_map['POST']) + [
            'product.view_productsaleableitem',
            'product.add_productsaleablepackage',
            'product.add_productsaleablepackagecomposite', 'product.view_productsaleablepackagecomposite',
            'product.add_productsaleablepackagemedia',     'product.view_productsaleablepackagemedia',
            'product.add_productappliedattributeprice', 'product.view_productappliedattributeprice',
        ],
        'PUT':    copy.copy(BaseIngredientPermissions.perms_map['PUT'])  + [
            'product.view_productsaleableitem',
            'product.change_productsaleablepackage',
            'product.add_productsaleablepackagecomposite',    'product.view_productsaleablepackagecomposite',
            'product.change_productsaleablepackagecomposite', 'product.delete_productsaleablepackagecomposite',
            'product.add_productsaleablepackagemedia',    'product.view_productsaleablepackagemedia',
            'product.change_productsaleablepackagemedia', 'product.delete_productsaleablepackagemedia',
            'product.add_productappliedattributeprice',    'product.view_productappliedattributeprice',
            'product.change_productappliedattributeprice', 'product.delete_productappliedattributeprice',
        ],
        'PATCH':  copy.copy(BaseIngredientPermissions.perms_map['PATCH'])  + [
            'product.view_productsaleableitem',
            'product.change_productsaleablepackage',
            'product.change_productsaleablepackagecomposite',
            'product.change_productsaleablepackagemedia',
            'product.change_productappliedattributeprice',
        ],
        'DELETE': copy.copy(BaseIngredientPermissions.perms_map['DELETE']) + [
            'product.change_productsaleablepackage',
            'product.change_productsaleablepackagecomposite',
            'product.change_productsaleablepackagemedia',
            'product.change_productappliedattributeprice',
        ], # users should have access to change delete status, since its soft-delete
    }

    def has_permission(self, request, view):
        result = super().has_permission(request=request, view=view)
        if result is True:
            result = self._input_data_owner_check(request, view)
        return result
## end of class SaleablePackagePermissions

