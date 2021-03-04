from common.views.web  import  BaseAuthHTMLView, BaseLoginHTMLView
from .constants import WEB_HOST, API_GATEWAY_HOST, LOGIN_URL, HTML_TEMPLATE_MAP

_module_name = __name__.split('.')[-1]
template_map = HTML_TEMPLATE_MAP[_module_name]

class AuthHTMLView(BaseAuthHTMLView):
    login_url = '%s%s' % (WEB_HOST, LOGIN_URL)

class LoginView(BaseLoginHTMLView):
    template_name = template_map[__qualname__]
    submit_url    = '%s%s' % (API_GATEWAY_HOST, LOGIN_URL)
    default_url_redirect = '/default_page'


