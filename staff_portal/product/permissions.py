
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
        'GET': ['product.view_producttag'],
        'OPTIONS': [],
        'HEAD': [],
        'POST':   ['product.add_producttag'],
        'PUT':    ['product.change_producttag'],
        'PATCH':  ['product.change_producttag'],
        'DELETE': ['product.delete_producttag'],
    }




