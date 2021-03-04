import json
import logging

from django.conf   import  settings as django_settings
from django.middleware  import csrf

from common.views.web  import  BaseAuthHTMLView, BaseHTMLView
from common.views.proxy.mixins import DjangoProxyRequestMixin, _render_url_path
from .common    import AuthHTMLView
from .constants import HTML_TEMPLATE_MAP, USERMGT_SERVICE_HOST

_logger = logging.getLogger(__name__)
_module_name = __name__.split('.')[-1]
template_map = HTML_TEMPLATE_MAP[_module_name]


class DashBoardView(AuthHTMLView):
    template_name = template_map[__qualname__]

class AuthRoleAddHTMLView(AuthHTMLView):
    template_name = template_map[__qualname__]

class AuthRoleUpdateHTMLView(AuthHTMLView, DjangoProxyRequestMixin):
    template_name = template_map[__qualname__]
    dst_host = USERMGT_SERVICE_HOST
    path_pattern = 'authroles'
    required_query_param_keys = ['ids']
    default_query_params = {'fields': 'id,name,permissions', 'skip_preserved_role': ''}

    def get(self, request, *args, **kwargs):
        pxy_req_kwargs = self.collect(request, **kwargs)
        pxy_resp = self.send(**pxy_req_kwargs)
        kwargs['response_kwargs'] = {'status': pxy_resp.status_code}
        if int(pxy_resp.status_code) < 400:
            edit_data = pxy_resp.text.encode('utf8')
            edit_data = json.loads(edit_data)
            kwargs['formparams'] = {'data': edit_data,}
        return  super().get(request, *args, **kwargs)

    def collect(self, request, **kwargs):
        out = super().collect(request, **kwargs)
        extra = {'method':'GET',}
        out.update(extra)
        return out


class QuotaUsageTypeAddHTMLView(AuthHTMLView, DjangoProxyRequestMixin):
    template_name = template_map[__qualname__]
    dst_host = USERMGT_SERVICE_HOST
    path_pattern = 'quota_material'

    def get(self, request, *args, **kwargs):
        pxy_req_kwargs = self.collect(request, **kwargs)
        pxy_req_kwargs['method'] = 'GET'
        pxy_req_kwargs['error_status_code'] = 503 # service unavailable
        pxy_resp = self.send(**pxy_req_kwargs)
        kwargs['response_kwargs'] = {'status': pxy_resp.status_code}
        if int(pxy_resp.status_code) < 400:
            material_type = pxy_resp.text.encode('utf8')
            material_type = json.loads(material_type)
            kwargs['formparams'] = {'material_type': material_type,}
        else:
            kwargs['formparams'] = {'reason': 'destination service not available',}
        return  super().get(request, *args, **kwargs)


class QuotaUsageTypeUpdateHTMLView(AuthHTMLView, DjangoProxyRequestMixin):
    template_name = template_map[__qualname__]
    dst_host = USERMGT_SERVICE_HOST
    path_pattern = 'quota'
    required_query_param_keys = ['ids']
    default_query_params = {'fields': 'id,label,material,appname',}

    def get(self, request, *args, **kwargs):
        editdata_req_kwargs = self.collect(request, **kwargs)
        extra = {'method':'GET', 'error_status_code': 503, }
        editdata_req_kwargs.update(extra)
        editdata_resp = self.send(**editdata_req_kwargs)

        material_req_kwargs = {
                'headers': editdata_req_kwargs['headers'],
                'cookies': editdata_req_kwargs['cookies'],
                'verify':  editdata_req_kwargs['verify'],
                'url':   '%s/quota_material' % self._get_dst_host(),
                'timeout': self.settings.TIMEOUT }
        material_req_kwargs.update(extra)
        material_resp = self.send(**material_req_kwargs)

        formparams = {}

        kwargs['response_kwargs'] = {'status': editdata_resp.status_code}
        resps = {'data': editdata_resp, 'material_type': material_resp}
        for key , resp in resps.items():
            if int(resp.status_code) < 400:
                _data = resp.text.encode('utf8')
                _data = json.loads(_data)
                formparams[key] = _data
        kwargs['formparams'] = formparams
        return  super().get(request, *args, **kwargs)



class UserGroupsAddHTMLView(AuthHTMLView):
    template_name = template_map[__qualname__]

class UserGroupsUpdateHTMLView(AuthHTMLView, DjangoProxyRequestMixin):
    template_name = template_map[__qualname__]
    dst_host = USERMGT_SERVICE_HOST
    path_pattern = 'usrgrps'
    required_query_param_keys = ['ids']
    default_query_params = {'parent_only': 'yes', 'exc_rd_fields':['roles__name',
        'quota__usage_type__label', 'ancestors__ancestor__name', 'ancestors__id'],
        'fields': 'id,name,ancestors,depth,ancestor,roles,quota,maxnum,usage_type,label',}

    def get(self, request, *args, **kwargs):
        pxy_req_kwargs = self.collect(request, **kwargs)
        pxy_req_kwargs['method'] = 'GET'
        pxy_resp = self.send(**pxy_req_kwargs)
        kwargs['response_kwargs'] = {'status': pxy_resp.status_code}
        if int(pxy_resp.status_code) < 400:
            edit_data = pxy_resp.text.encode('utf8')
            edit_data = json.loads(edit_data)
            kwargs['formparams'] = {'data': edit_data,}
        return  super().get(request, *args, **kwargs)


class UserProfileAddHTMLView(AuthHTMLView):
    template_name = template_map[__qualname__]

class UserProfileUpdateHTMLView(AuthHTMLView, DjangoProxyRequestMixin):
    template_name = template_map[__qualname__]
    dst_host = USERMGT_SERVICE_HOST
    path_pattern = 'usrprofs'
    required_query_param_keys = ['ids']
    _fetch_fields = ['id','first_name','last_name','groups','roles','quota','maxnum','usage_type',
            'label', 'phones', 'phone', 'country_code', 'line_number','emails', 'email', 'addr',
            'locations','address', 'country', 'province', 'locality', 'street', 'detail',
            'floor', 'description',]
    default_query_params = {'exc_rd_fields':['roles__name'],  'fields': ','.join(_fetch_fields),}

    def get(self, request, *args, **kwargs):
        pxy_req_kwargs = self.collect(request, **kwargs)
        pxy_req_kwargs['method'] = 'GET'
        pxy_resp = self.send(**pxy_req_kwargs)
        kwargs['response_kwargs'] = {'status': pxy_resp.status_code}
        if int(pxy_resp.status_code) < 400:
            edit_data = pxy_resp.text.encode('utf8')
            edit_data = json.loads(edit_data)
            kwargs['formparams'] = {'data': edit_data,}
        return  super().get(request, *args, **kwargs)


class AbstractAuthTokenHTMLView(BaseHTMLView, DjangoProxyRequestMixin):
    dst_host = USERMGT_SERVICE_HOST
    path_pattern = 'authtoken/{token}'
    path_handler = _render_url_path
    path_var_keys = ['token']

    def get(self, request, *args, **kwargs):
        pxy_req_kwargs = self.collect(request, **kwargs)
        pxy_req_kwargs['method'] = 'GET'
        pxy_resp = self.send(**pxy_req_kwargs)
        if int(pxy_resp.status_code) < 300:
            token = kwargs['token']
            chosen_template_idx = 0
            kwargs['formparams'] = {'activate_token': token}
            # the function below has side effect that also sets CSRF cookie, once the cookie is set
            # , the client frontend would send another request with the same CSRF cookie to my API server,
            # then the API server receives the CSRF cookie and start CSRF validation.
            csrf.get_token(request=request)
        else:
            chosen_template_idx = 1
        kwargs['response_kwargs'] = {'status': pxy_resp.status_code, 'chosen_template_idx': chosen_template_idx}
        return  super().get(request, *args, **kwargs)


class AccountCreateHTMLView(AbstractAuthTokenHTMLView):
    template_name = template_map[__qualname__] # supposed to be a list of templates


def _renew_csrf_cookies(request):
    # the function below refreshes and sets CSRF cookie, with custom expiry
    if "CSRF_COOKIE" not in request.META:
        csrf.rotate_token(request=request)
        request.csrf_cookie_age = 3600

class UsernameRecoveryRequestHTMLView(BaseHTMLView):
    template_name = template_map[__qualname__]
    def get(self, request, *args, **kwargs):
        _renew_csrf_cookies(request)
        return  super().get(request, *args, **kwargs)

class UnauthPasswdRstReqHTMLView(BaseHTMLView):
    template_name = template_map[__qualname__]
    def get(self, request, *args, **kwargs):
        _renew_csrf_cookies(request)
        return  super().get(request, *args, **kwargs)

class UnauthPasswdRstHTMLView(AbstractAuthTokenHTMLView):
    template_name = template_map[__qualname__]


