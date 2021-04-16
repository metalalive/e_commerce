from django.apps import AppConfig

class APIgatewayConfig(AppConfig):
    name = 'api'

    def ready(self):
        from common.util.python.messaging.monkeypatch import patch_kombu_pool
        patch_kombu_pool()
        from common.views.error import monkeypatch_django_error_view_production
        monkeypatch_django_error_view_production()
        # add --noreload to avoid django runserver from loading twice initially


