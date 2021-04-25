from django.http.response          import HttpResponse

from common.views.proxy.mixins import _render_url_path
from .constants import SERVICE_HOSTS
from .common    import BaseRevProxyView, _get_path_list_or_item_api


class AppBaseProxyView(BaseRevProxyView):
    dst_host = SERVICE_HOSTS['productmgt'][0]
    authenticate_required = {
        'OPTIONS': True, 'GET': True, 'POST': True,
        'PUT': True,  'PATCH': True, 'DELETE': True,
    }

class TrelloMemberProxyView(BaseRevProxyView):
    dst_host = SERVICE_HOSTS['usermgt'][0]
    authenticate_required = {'OPTIONS': True, 'GET': True,}
    # if prof_id is 'me', then return profile data of current login user
    path_pattern =  'usrprof/{prof_id}'
    path_handler =  _render_url_path
    path_var_keys = ['prof_id']
    _fetch_fields = ['id','first_name','last_name','groups', 'phones', 'phone',
            'country_code', 'line_number','emails', 'email', 'addr',   'country', 'locality',]
    default_query_params = {'fields': ','.join(_fetch_fields),}


class TrelloNotificationProxyView(AppBaseProxyView):
    # TODO: will be replaced with centralized logging / notification service
    authenticate_required = {'OPTIONS': True, 'GET': True, 'PATCH': True,}
    path_pattern =  'trello/notifications'

    def dispatch(self, request, *args, **kwargs):
        response = self._authenticate(request)
        if response:
            return response
        response = HttpResponse(content='[]', status=None, content_type='application/json')
        return response


class ProductTagProxyView(AppBaseProxyView):
    authenticate_required = {
        'OPTIONS': True, 'GET': True, 'POST': True,
        'PUT': True,  'DELETE': True,
    }
    path_pattern = ['tags', 'tag/{tag_id}', 'tag/{tag_id}/ancestors', 'tag/{tag_id}/descendants']
    path_handler = _get_path_list_or_item_api
    path_var_keys = ['tag_id']



