from django.apps import AppConfig
from common.util.python import BaseUriLookup, BaseTemplateLookup


class WebAPIurl(BaseUriLookup):
    # URLs of web APIs , accessable to end client users
    _urls = {
        'DashBoardView'       : 'dashboard',
    } # end of _urls


class TemplateName(BaseTemplateLookup):
    template_path  = 'product'

    _template_names = {
        'DashBoardView'       : 'DashBoard.html'        ,
    }


class ProductConfig(AppConfig):
    name = 'product'
    app_url   = 'productmgt'
    api_url   = WebAPIurl()
    template_name = TemplateName()

    def ready(self):
        from common.models.db import monkeypatch_django_db
        monkeypatch_django_db()
        from common.models.migrations import monkeypatch_django_migration
        monkeypatch_django_migration()

