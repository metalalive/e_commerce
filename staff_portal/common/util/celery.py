import os

from celery import Celery
from celery.result import AsyncResult

from . import celeryconfig

# set default Django settings module for Celery application
os.environ.setdefault('DJANGO_SETTINGS_MODULE', 'restaurant.settings')

# REST framework has to be imported after django settings
from rest_framework.generics    import GenericAPIView
from rest_framework.renderers   import JSONRenderer
from rest_framework.response    import Response as RestResponse

class AsyncTaskResultView(GenericAPIView):
    renderer_classes = [JSONRenderer]

    def get(self, request, *args, **kwargs):
        # print("kwargs  : "+ str(kwargs))
        r = AsyncResult(kwargs.pop('id', None))
        status = None
        headers = {}
        s_data = {'status': r.status, 'result': r.result or '' }
        return RestResponse(s_data, status=status, headers=headers)




app = Celery('app543')
# load centralized configuration module
app.config_from_object(celeryconfig)

if __name__ == '__main__':
    app.start()

