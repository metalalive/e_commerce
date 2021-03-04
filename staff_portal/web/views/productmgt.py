import logging

from .common    import AuthHTMLView
from .constants import HTML_TEMPLATE_MAP

_logger = logging.getLogger(__name__)
_module_name = __name__.split('.')[-1]
template_map = HTML_TEMPLATE_MAP[_module_name]


class DashBoardView(AuthHTMLView):
    template_name = template_map[__qualname__]




