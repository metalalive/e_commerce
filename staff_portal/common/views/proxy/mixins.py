import base64
import requests
from requests.exceptions import ConnectionError, SSLError, Timeout

from common.util.python import get_request_meta_key
from common.util.python.messaging.rpc       import RPCproxy
from common.util.python.messaging.constants import AMQP_EXCHANGE_NAME_CONFIG_KEY, AMQP_EXCHANGE_TYPE_CONFIG_KEY
from .settings import api_proxy_settings

class DjangoProxyRequestMixin:
    """
    a mixin class to collect data to be sent within requests.request()
    this mixin is tied to `Django` and `requests` python packages
    """
    settings = api_proxy_settings
    path_pattern = None
    path_handler = None
    path_var_keys = []
    required_query_param_keys = []
    default_query_params = {}
    dst_host = None
    verify_ssl = None

    def _get_dst_host(self):
        return self.dst_host or self.settings.HOST

    def _get_req_params(self, request):
        if request.GET:
            params_client = request.GET.copy()
        else:
            params_client = {}
        for key in self.required_query_param_keys:
            if params_client.get(key, None) is None:
                from django.core.exceptions import SuspiciousOperation
                # then return 400 bad request
                raise SuspiciousOperation('query parameter %s is required in the URL' % key)
        if any(self.default_query_params):
            params = self.default_query_params.copy()
            params.update(params_client)
        else:
            params = params_client
        return params

    def _get_default_headers(self, request):
        out = {}
        for http_header_name, default_value in self.settings.HEADER.items():
            meta_key = get_request_meta_key(http_header_name)
            value = request.META.get(meta_key, default_value)
            if not value:
                value = default_value
            out[http_header_name] = value
        return out

    def _get_auth_headers(self, request, headers):
        # append authorization header if required
        username = self.settings.AUTH.get('username')
        password = self.settings.AUTH.get('password')
        auth_token = self.settings.AUTH.get('token')
        if username and password:
            credential = '%s:%s' % (username, password)
            encoded = base64.b64encode(credential.encode('utf-8')).decode()
            headers['authorization'] = 'basic %s' % encoded
        elif auth_token:
            headers['authorization'] = 'token %s' % auth_token
        # forward remote user information (the authenticated client), by adding account username to header section
        # so downstream app servers have to check whether the remote user already exists in the database, and
        # whether the forwarding request comes from trusted proxy server (perhaps by examining the domain name)
        # TODO: figure out how downstream app servers handle the authorization from proxy server
        if request.user.is_authenticated:
            from common.auth.backends import FORWARDED_HEADER
            uname = request.user.get_username()
            headers[FORWARDED_HEADER] = 'by=%s;for=%s;host=%s;proto=%s' % \
                    ('proxy_api_gateway', uname, request.get_host(), request.scheme)

    def _get_headers(self, request):
        headers = self._get_default_headers(request)
        self._get_auth_headers(request=request, headers=headers)
        return headers

    def _get_verify_ssl(self):
        return self.verify_ssl or self.settings.VERIFY_SSL

    def get_cookies(self, request):
        """ subclass this proxy view and override this function """
        pass

    def _get_req_body(self, request):
        """ get raw string of request body """
        return  request.body

    def _get_req_files(self, request):
        return None # TODO, finish implementation

    def _get_req_path(self, request, **kwargs):
        """
        subclasses can overwrite this method for more complicated URI naming scheme in
        downstream application servers
        """
        out = None
        if self.path_pattern:
            _handler = self.path_handler
            if _handler and callable(_handler): # require further process on the path pattern
                fn = lambda x: (x, kwargs[x]) if kwargs.get(x, None) else None
                filtered_keys = list(filter(fn, self.path_var_keys))
                key_vars = dict(map(fn, filtered_keys))
                out = _handler(request=request, key_vars=key_vars) # implicitly pass proxyview=self argument
            else:
                out = self.path_pattern
        else: # pass the path of incoming request
            out = request.get_full_path()
        return out


    def _get_req_url(self, request, **kwargs):
        host = self._get_dst_host()
        path = self._get_req_path(request, **kwargs)
        if path:
            url = '/'.join([host, path])
        else:
            url = host
        return url

    def collect(self, request, **kwargs):
        """
        collect everything that will be sent within a requests.request()
        """
        params = self._get_req_params(request)
        headers = self._get_headers(request)
        verify_ssl = self._get_verify_ssl()
        cookies = self.get_cookies(request)
        body   = self._get_req_body(request)
        files  = self._get_req_files(request)
        url = self._get_req_url(request, **kwargs)
        return { 'params':params, 'headers':headers, 'cookies':cookies, 'verify':verify_ssl,
                 'files':files, 'data':body, 'url':url, 'timeout': self.settings.TIMEOUT }


    def send(self, **pxy_req_kwargs):
        error_status_code = pxy_req_kwargs.pop('error_status_code', None)
        send_fn = pxy_req_kwargs.pop('send_fn', requests.request)
        try:
            if pxy_req_kwargs.get('files', None):
                pass # TODO, finish implementation
            else:
                # TODO, may consider streaming request/response in the future ?
                response = send_fn( **pxy_req_kwargs )
                ##print('check headers after receiving from app server ? %s' % response.headers)
        except (ConnectionError, SSLError, Timeout) as e:
            print('proxy goes wrong, exception = %s , response = %s' % (e, e.response))
            response = e.response
            if response is None:
                response = requests.Response()
                if error_status_code:
                    response.status_code = error_status_code
                elif isinstance(e, Timeout):
                    response.status_code = requests.codes['gateway_timeout']
                else:
                    response.status_code = requests.codes['bad_gateway']
        return response


# helper functions for proxy view class
# it can be pointed by DjangoProxyRequestMixin.path_handler 
def _render_url_path(proxyview, request, key_vars):
    if any(key_vars):
        out = proxyview.path_pattern.format(**key_vars)
    else:
        out = proxyview.path_pattern
    return out


class BaseGetProfileIDMixin:
    UNKNOWN_ID = -1

    def get_account(self, request):
        raise NotImplementedError

    def get_account_id(self, account):
        raise NotImplementedError

    def get_profile_id(self, request, **kwargs):
        raise NotImplementedError

    def get_profile(self, account):
        raise NotImplementedError


class RemoteGetProfileIDMixin(BaseGetProfileIDMixin):
    _usermgt_rpc = RPCproxy(app_name='user_management')
    # currently get_account() and get_account_id() works with Django,
    # both functions should be abstracted for other web frameworks
    def get_account(self, request):
        return request.user

    def get_account_id(self, account):
        return account.pk

    def _ensure_get_profile_attr(self, request, field_name, default_value, **kwargs):
        reply = self.get_profile(account=self.get_account(request))
        if not reply.finished:
            num_of_msgs_fetch = kwargs.pop('num_of_msgs_fetch',None)
            reply.refresh(retry=reply.timeout, num_of_msgs_fetch=num_of_msgs_fetch)
        result = reply.result
        if result and isinstance(result, dict):
            usr_prof = result.get('result', None)
            if usr_prof and isinstance(usr_prof, dict):
                out = usr_prof.get(field_name, default_value)
            else:
                out = default_value
        else:
            out = default_value
        return out

    def get_profile_id(self, request, **kwargs):
        _id = self._ensure_get_profile_attr(request=request, field_name='id',
                default_value=self.UNKNOWN_ID, **kwargs)
        return str(_id)

    def get_profile_roles(self, request, **kwargs):
        roles = self._ensure_get_profile_attr(request=request, field_name='roles',
                default_value=[], **kwargs)
        return roles

    def get_profile(self, account):
        # make RPC call as internal communication to user-management service
        if not hasattr(self, '_user_profile_reply'):
            acc_id = self.get_account_id(account=account)
            field_names = ['id', 'first_name', 'last_name','roles','quota']
            self._user_profile_reply = self._usermgt_rpc.get_profile(account_id=acc_id,
                    field_names=field_names)
        # it is actually reply object, to retrieve return value of RPC
        # application has to invoke reply.result()
        return self._user_profile_reply


# from common.util.python.messaging.rpc       import RPCproxy
# myrpc = RPCproxy(app_name='user_management')
# evt = myrpc.get_profile(account_id=25, field_names=['id', 'first_name', 'last_name'])
# evt2 = myrpc.xxXx(yui='249fye')

